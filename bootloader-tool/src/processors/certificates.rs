use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::Context;
use serde::Deserialize;

use crate::{
    GenerateCertificatesArguments,
    config::Config,
    util::{bytes_to_u32_le, generate_hex, parse_hex},
};

#[derive(Deserialize, Debug)]
struct CertDescr {
    subject_public_key: PathBuf,
}

fn generate_private_key(
    cert_descr: &CertDescr,
    nxpcrypto: impl AsRef<Path>,
    config: &Config,
) -> anyhow::Result<()> {
    // nxpcrypto key generate -k rsa2048 -e PEM -o IMG1_1_sha256_2048_65537_v3_usr_key.pem

    // Note: apparently the field name refers to a public key, but it is for the private key.
    let output_path = &cert_descr.subject_public_key;
    let output_path_abs = config.artifacts_path.join(output_path);
    if std::fs::exists(&output_path_abs)? {
        log::warn!(
            "Private key {} already generated, skipping...",
            output_path_abs.display()
        );
        return Ok(());
    }

    log::info!("Generating private key {}", output_path.display());

    let mut command = Command::new(nxpcrypto.as_ref());
    command.current_dir(&config.artifacts_path);

    command.args([
        "key",
        "generate",
        "-k",
        config.key_type.as_str(),
        "-e",
        "PEM",
        "-o",
    ]);
    command.arg(output_path);

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(failed_exec(nxpcrypto))?;

    if !output.status.success() {
        Err(anyhow::anyhow!(format!(
            "Failed to generate private key {}",
            cert_descr.subject_public_key.display()
        ))
        .context(String::from_utf8(output.stdout)?))
    } else {
        Ok(())
    }
}

fn generate_certificate(
    input: impl AsRef<Path>,
    nxpcrypto: impl AsRef<Path>,
    config: &Config,
) -> anyhow::Result<()> {
    // nxpcrypto cert generate -c ROT1_2048_csr.yaml -e PEM -o ROT1_sha256_2048_65537_v3_ca_crt.PEM

    let mut output_path = input.as_ref().to_path_buf();
    output_path.set_extension("pem");

    let output_path_abs = config.artifacts_path.join(&output_path);
    if std::fs::exists(&output_path_abs)? {
        log::warn!(
            "Certificate {} already generated, skipping...",
            output_path_abs.display()
        );
        return Ok(());
    }

    log::info!("Generating certificate {}", output_path.display());

    let mut command = Command::new(nxpcrypto.as_ref());
    command.current_dir(&config.artifacts_path);

    command.args(["cert", "generate", "-e", "PEM", "-c"]);
    command.arg(input.as_ref());
    command.arg("-o");
    command.arg(output_path);

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(failed_exec(nxpcrypto))?;

    if !output.status.success() {
        Err(anyhow::anyhow!(format!(
            "Failed to build certificate from {}",
            input.as_ref().display()
        ))
        .context(String::from_utf8(output.stdout)?))
    } else {
        Ok(())
    }
}

fn generate_single(
    input: impl AsRef<Path>,
    nxpcrypto: impl AsRef<Path>,
    config: &Config,
) -> anyhow::Result<()> {
    let input_abs = config.artifacts_path.join(&input);

    let cert_descr: CertDescr = serde_yml::from_reader(std::fs::File::open(&input_abs)?)?;

    generate_private_key(&cert_descr, &nxpcrypto, config)?;
    generate_certificate(input, &nxpcrypto, config)?;

    Ok(())
}

#[derive(PartialEq, Clone, Debug)]
pub struct Rkth(pub [u8; 32]);

impl Rkth {
    pub fn as_hex(&self) -> String {
        generate_hex(&self.0)
    }

    pub fn from_hex(str: &str) -> anyhow::Result<Self> {
        Ok(Self(parse_hex(str)?.try_into().map_err(|_| {
            anyhow::anyhow!("Input not appropriate size")
        })?))
    }

    pub fn as_u32_le(&self) -> Vec<u32> {
        bytes_to_u32_le(&self.0)
    }
}

fn generate_cert_block_rkth(
    input: impl AsRef<Path>,
    nxpimage: impl AsRef<Path>,
    config: &Config,
) -> anyhow::Result<Rkth> {
    // nxpimage cert-block export -c ./cert-block.yaml

    if std::fs::exists(&config.rkth_path)? {
        log::warn!(
            "RKTH file {} already generated, skipping...",
            &config.rkth_path.display()
        );
        return get_rkth(config);
    }

    log::info!(
        "Generating RKTH file from certificate block {}",
        &config.rkth_path.display()
    );

    let mut command = Command::new(nxpimage.as_ref());
    command.current_dir(&config.artifacts_path);

    command.args(["cert-block", "export", "-c"]);
    command.arg(input.as_ref());

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(failed_exec(nxpimage))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(format!(
            "Failed to build certificate block from {}",
            input.as_ref().display()
        ))
        .context(String::from_utf8(output.stdout)?));
    }

    let rkth_str = String::from_utf8(output.stdout)?
        .lines()
        .find(|line| line.contains("RKTH"))
        .map(|line| line.trim().trim_start_matches("RKTH: "))
        .ok_or_else(|| anyhow::anyhow!("nxpimage output does not contain RKTH"))?
        .to_owned();
    let rkth = Rkth::from_hex(&rkth_str)?;
    let rkth_str = rkth.as_hex(); // Canonicalize

    log::info!("RKTH: {rkth_str}");
    std::fs::write(&config.rkth_path, rkth_str)?;

    Ok(rkth)
}

const ROOT_KEY_DESCR_PATH: &str = "./cert-rot1.yaml";
const IMG_KEY_DESCR_PATH: &str = "./cert-img1.yaml";
const CERT_BLOCK_DESCR_PATH: &str = "./cert-block.yaml";

pub fn generate(args: GenerateCertificatesArguments, config: &Config) -> anyhow::Result<()> {
    generate_single(ROOT_KEY_DESCR_PATH, &args.nxpcrypto_path, config)?;
    generate_single(IMG_KEY_DESCR_PATH, &args.nxpcrypto_path, config)?;
    generate_cert_block_rkth(CERT_BLOCK_DESCR_PATH, &args.nxpimage_path, config)?;

    Ok(())
}

pub fn get_rkth(config: &Config) -> anyhow::Result<Rkth> {
    let path = &config.rkth_path;
    let rkth_hex_str = std::fs::read_to_string(&config.rkth_path)
        .with_context(|| format!("Failed to open RKTH file {}", path.display()))?;

    Rkth::from_hex(&rkth_hex_str)
}

fn failed_exec<'a>(tool: impl AsRef<Path> + 'a) -> impl Fn() -> String + 'a {
    move || {
        format!(
            "Could not execute `{}`, is it installed?",
            tool.as_ref().display()
        )
    }
}
