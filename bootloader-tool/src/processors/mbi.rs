use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, anyhow};

fn input_tmpfile(data: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::with_prefix_in("signtmp", ".").unwrap();
    file.write_all(data).unwrap();
    file
}

pub fn generate(
    nxpimage: impl AsRef<Path>,
    input_image: &[u8],
    base_addr: u32,
    output_path: impl AsRef<Path>,
    is_bootloader: bool,
) -> anyhow::Result<()> {
    let mut config: BTreeMap<String, String> = BTreeMap::default();

    config.insert(
        "outputImageExecutionAddress".to_owned(),
        format!("{base_addr:#x}"),
    );

    let input_image_file = input_tmpfile(input_image);
    config.insert(
        "inputImageFile".to_owned(),
        input_image_file
            .path()
            .to_str()
            .ok_or_else(|| anyhow!("Path not a string"))?
            .into(),
    );

    let output_path_abs = std::env::current_dir()?.join(output_path.as_ref());
    config.insert(
        "masterBootOutputFile".to_owned(),
        output_path_abs
            .to_str()
            .ok_or_else(|| anyhow!("Path not a string"))?
            .into(),
    );

    log::debug!("Config: {config:#?}");

    let mut command = Command::new(nxpimage.as_ref());

    let mbi_conf_path = if is_bootloader {
        "artifacts/mbi-bootloader.yaml"
    } else {
        "artifacts/mbi-application.yaml"
    };

    command.args(["mbi", "export", "-c", mbi_conf_path]);

    for (k, v) in config {
        command.args(["-oc", &format!("{k}={v}")]);
    }

    let output = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("Could not execute command {command:?}"))?;

    if !output.status.success() {
        return Err(
            anyhow::anyhow!(format!("Failed to build MBI image from {}", mbi_conf_path))
                .context(String::from_utf8(output.stdout)?),
        );
    }

    let output = std::fs::read(&output_path)?;
    let diff_len = output.len() - input_image.len();

    log::debug!("Output len: 0x{:x}", output.len());
    log::debug!("Added len: 0x{diff_len:x}");

    if diff_len > 0x744 {
        return Err(anyhow::anyhow!(
            "Added more than expected to output image when signing"
        ));
    }

    // Performing checks on output image
    let expected_image_type = if is_bootloader { 0x4001u32 } else { 0x4004u32 };

    let image_type = u32::from_le_bytes((&output[0x24..0x28]).try_into().unwrap());
    if image_type != expected_image_type {
        return Err(anyhow::anyhow!(
            "Failed to generate expected image type 0x{:x}, got 0x{:x}",
            expected_image_type,
            image_type
        ));
    }
    log::debug!("Got expected image type 0x{expected_image_type:x}");

    // TODO more checks

    Ok(())
}
