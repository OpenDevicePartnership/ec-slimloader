use crate::SignCommands;
use crate::config::Config;
use crate::processors::certificates::get_rkth;
use crate::processors::otp::get_otp;
use crate::processors::{mbi, objcopy};
use anyhow::Context;
use object::read::elf::ElfFile32;
use std::path::PathBuf;

pub struct SignOutput {
    pub output_path: Option<PathBuf>,
}

pub async fn process(config: &Config, command: SignCommands) -> anyhow::Result<SignOutput> {
    let (is_bootloader, args) = match command {
        SignCommands::Bootloader(sign_arguments) => (true, sign_arguments),
        SignCommands::Application(sign_arguments) => (false, sign_arguments),
    };

    let input_data = std::fs::read(&args.input_path)?;

    log::info!("Reading ELF from {}", args.input_path.display());
    let file = ElfFile32::parse(&input_data[..]).context("Could not parse ELF file")?;

    if is_bootloader {
        log::info!("Extracting prelude");
        let out = objcopy::remove_non_prelude(&input_data)?;
        std::fs::write(args.prelude_path_with_default(), &out).context("Could not write prelude elf file")?;
    }

    log::info!("Generating image for {}", args.input_path.display());
    let (image, base_addr) = objcopy::objcopy(&file)?;

    if is_bootloader {
        if let Some(bootloader) = &config.bootloader {
            if bootloader.run_start != base_addr as u64 {
                return Err(anyhow::anyhow!(
                    "Bootloader image will be run from unexpected address 0x{:x}, should be 0x{:x}",
                    base_addr,
                    bootloader.run_start
                ));
            }
        }
    } else {
        if let Some(application) = &config.application {
            if application.run_start != base_addr as u64 {
                return Err(anyhow::anyhow!(
                    "Application image will be run from unexpected address 0x{:x}, should be 0x{:x}",
                    base_addr,
                    application.run_start
                ));
            }
        }
    }

    let output_unsigned_path = args.output_unsigned_path_with_default();
    log::debug!("Wrote unsigned bare binary image to {}", output_unsigned_path.display());
    std::fs::write(&output_unsigned_path, &image)?;

    let otp = get_otp(config)?;
    let rkth = get_rkth(config)?;

    let output_prestage_path = args.output_prestage_path_with_default();
    log::info!(
        "Generating prestage MBI using pure Rust in {}",
        output_prestage_path.display()
    );
    mbi::prepare_to_sign(
        &output_unsigned_path,
        base_addr,
        &output_prestage_path,
        is_bootloader,
        &rkth,
    )
    .context("Could not generate prestage MBI")?;

    let mut signature_path = args.signature_path.clone();

    if !args.dont_sign && signature_path.is_none() {
        log::info!("Signing image {}", args.input_path.display());

        let default_path = args.input_path.clone().with_extension("signature.bin");
        mbi::sign(&default_path, &output_prestage_path).context("Could not sign image")?;
        signature_path = Some(default_path);
    }

    if let Some(signature_path) = signature_path {
        let output_path = args.output_path_with_default();
        log::info!("Merging signature into image");
        mbi::merge_with_signature(
            &output_unsigned_path,
            base_addr,
            signature_path,
            &output_path,
            is_bootloader,
            Some(otp),
            &rkth,
        )
        .context("Could not merge image with signature")?;
        log::info!("Written merged image to {}", output_path.display());
        Ok(SignOutput {
            output_path: Some(output_path),
        })
    } else {
        Ok(SignOutput { output_path: None })
    }
}
