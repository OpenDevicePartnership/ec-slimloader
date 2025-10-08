use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;
use serde::Serialize;

use crate::GenerateCertificatesArguments;
use crate::config::{Certificate, CertificatePrototype, Config};
use crate::util::{bytes_to_u32_le, generate_hex, parse_hex};

#[derive(Serialize)]
struct BasicConstraints {
    ca: bool,
}

#[derive(Serialize)]
struct CertificateExtensions {
    #[serde(rename = "BASIC_CONSTRAINTS")]
    basic_constraints: BasicConstraints,
}

#[derive(Serialize)]
struct CertificateConfig {
    issuer_private_key: PathBuf,
    subject_public_key: PathBuf,
    extensions: CertificateExtensions,
}

fn generate_private_key(nxpcrypto: impl AsRef<Path>, prototype: &CertificatePrototype) -> anyhow::Result<()> {
    // Note: apparently the field name refers to a public key, but it is for the private key.
    let output_path = &prototype.key_path;
    if std::fs::exists(output_path)? {
        log::warn!("Private key {} already generated, skipping...", output_path.display());
        return Ok(());
    }

    log::info!("Generating private key {}", output_path.display());

    let mut command = Command::new(nxpcrypto.as_ref());

    command.args(["key", "generate", "-k", prototype.key_type.as_str(), "-e", "PEM", "-o"]);
    command.arg(output_path);

    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(failed_exec(nxpcrypto))?;

    if !output.status.success() {
        Err(
            anyhow::anyhow!(format!("Failed to generate private key {}", output_path.display()))
                .context(String::from_utf8(output.stdout)?),
        )
    } else {
        Ok(())
    }
}

fn generate_certificate(
    nxpcrypto: impl AsRef<Path>,
    certificate: &Certificate,
    parent_certificate: &Option<Certificate>,
    is_leaf: bool,
) -> anyhow::Result<()> {
    let Some(prototype) = &certificate.prototype else {
        return Err(anyhow::anyhow!(
            "Request generation of private key for {}, but no prototype has been defined",
            certificate.path.display()
        ));
    };

    let issuer_private_key = if let Some(parent_certificate) = &parent_certificate {
        let Some(parent_prototype) = &parent_certificate.prototype else {
            return Err(anyhow::anyhow!(
                "Request generation of private key for {}, but no prototype has been defined",
                certificate.path.display()
            ));
        };

        parent_prototype.key_path.clone()
    } else {
        // This certificate is the root certificate, and hence self-signed.
        prototype.key_path.clone()
    };

    let input = CertificateConfig {
        issuer_private_key,
        subject_public_key: prototype.key_path.clone(),
        extensions: CertificateExtensions {
            basic_constraints: BasicConstraints { ca: !is_leaf },
        },
    };

    let mut input_file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut input_file, &input)?;

    let output_path = &certificate.path;
    if std::fs::exists(output_path)? {
        log::warn!("Certificate {} already generated, skipping...", output_path.display());
        return Ok(());
    }

    log::info!("Generating certificate {}", output_path.display());

    let mut command = Command::new(nxpcrypto.as_ref());

    command.args(["cert", "generate", "-e", "PEM", "-c"]);
    command.arg(input_file.path());
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
            "Failed to build certificate from {:?} (parent: {:?})",
            certificate, parent_certificate
        ))
        .context(String::from_utf8(output.stdout)?))
    } else {
        Ok(())
    }
}

fn generate_single(
    nxpcrypto: impl AsRef<Path>,
    certificate: &Certificate,
    parent_certificate: &Option<Certificate>,
    is_leaf: bool,
) -> anyhow::Result<()> {
    let Some(prototype) = &certificate.prototype else {
        return Err(anyhow::anyhow!(
            "Request generation of private key for {}, but no prototype has been defined",
            certificate.path.display()
        ));
    };

    generate_private_key(&nxpcrypto, prototype)?;
    generate_certificate(&nxpcrypto, certificate, parent_certificate, is_leaf)?;

    Ok(())
}

#[derive(PartialEq, Clone, Debug)]
pub struct Rkth(pub [u8; 32]);

impl Rkth {
    pub fn as_hex(&self) -> String {
        generate_hex(&self.0)
    }

    pub fn from_hex(str: &str) -> anyhow::Result<Self> {
        Ok(Self(
            parse_hex(str)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("Input not appropriate size"))?,
        ))
    }

    pub fn as_u32_le(&self) -> Vec<u32> {
        bytes_to_u32_le(&self.0)
    }
}

pub fn generate(args: GenerateCertificatesArguments, config: &Config) -> anyhow::Result<()> {
    for chain in &config.certificates {
        let mut parent = None;
        let mut iter = chain.0.iter().peekable();
        while let Some(certificate) = iter.next() {
            let is_leaf = iter.peek().is_none();
            generate_single(&args.nxpcrypto_path, certificate, &parent, is_leaf)?;
            parent = Some(certificate.clone());
        }
    }

    Ok(())
}

fn failed_exec<'a>(tool: impl AsRef<Path> + 'a) -> impl Fn() -> String + 'a {
    move || format!("Could not execute `{}`, is it installed?", tool.as_ref().display())
}
