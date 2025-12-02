use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Context;
use object::read::elf::ElfFile32;

use crate::SignCommands;
use crate::config::Config;
use crate::processors::certificates::Rkth;
use crate::processors::mbi::cert_block;
use crate::processors::otp::get_otp;
use crate::processors::{mbi, objcopy};

pub struct SignOutput {
    pub output_path: Option<PathBuf>,
    pub rkth: Rkth,
}

fn perform_checks(config: &Config, is_bootloader: bool, image: &[u8], base_addr: u32) -> anyhow::Result<()> {
    struct Values {
        run_start: u64,
        max_size: u64,
    }

    let values = if is_bootloader {
        let Some(bootloader) = &config.bootloader else {
            return Err(anyhow::anyhow!("Bootloader field not set in config"));
        };

        Values {
            run_start: bootloader.run_start,
            max_size: bootloader.max_size,
        }
    } else {
        let Some(application) = &config.application else {
            return Err(anyhow::anyhow!("Application field not set in config"));
        };

        Values {
            run_start: application.run_start,
            max_size: application.slot_size,
        }
    };

    if values.run_start != base_addr as u64 {
        return Err(anyhow::anyhow!(
            "Image will be run from unexpected address 0x{:x}, should be 0x{:x}",
            base_addr,
            values.run_start
        ));
    }

    if values.max_size < image.len() as u64 {
        return Err(anyhow::anyhow!(
            "Image can not fit in 0x{:x}, actual size is 0x{:x}",
            values.max_size,
            image.len(),
        ));
    }

    Ok(())
}

pub async fn process(config: &Config, command: SignCommands) -> anyhow::Result<SignOutput> {
    let (is_bootloader, args) = match command {
        SignCommands::Bootloader(sign_arguments) => (true, sign_arguments),
        SignCommands::Application(sign_arguments) => (false, sign_arguments),
    };

    let files: Vec<Vec<u8>> = args
        .input_paths
        .iter()
        .map(|input_path| {
            log::info!("Reading ELF from {}", input_path.display());
            std::fs::read(input_path)
        })
        .collect::<Result<Vec<Vec<u8>>, std::io::Error>>()?;

    let files: Vec<ElfFile32> = files
        .iter()
        .map(|input_data| ElfFile32::parse(&input_data[..]).context("Could not parse ELF file"))
        .collect::<Result<Vec<ElfFile32>, anyhow::Error>>()?;

    if is_bootloader {
        log::info!("Extracting prelude");
        let out = objcopy::remove_non_prelude(files.first().unwrap())?;
        std::fs::write(args.prelude_path_with_default(), &out).context("Could not write prelude elf file")?;
    }

    let objcopy_config = if is_bootloader {
        objcopy::Config {
            mappings: &BTreeSet::new(),
            max_size: Some(config.bootloader.as_ref().unwrap().max_size as u32),
        }
    } else {
        let app = config.application.as_ref().unwrap();
        objcopy::Config {
            mappings: &app.address_mapping,
            max_size: Some(app.slot_size as u32),
        }
    };

    log::info!("Generating image");
    let (image, base_addr) = objcopy::objcopy(files.iter(), objcopy_config)?;

    perform_checks(config, is_bootloader, &image, base_addr)?;

    let output_unsigned_path = args.output_unsigned_path_with_default();
    log::debug!("Wrote unsigned bare binary image to {}", output_unsigned_path.display());
    std::fs::write(&output_unsigned_path, &image)?;

    let otp = get_otp(config)?;

    let output_prestage_path = args.output_prestage_path_with_default();
    log::info!(
        "Generating prestage MBI using pure Rust in {}",
        output_prestage_path.display()
    );

    let cert_block = cert_block::generate(&args.nxpimage_path, config, args.certificate)?;

    mbi::prepare_to_sign(
        &output_unsigned_path,
        base_addr,
        &output_prestage_path,
        is_bootloader,
        cert_block.clone(),
    )
    .context("Could not generate prestage MBI")?;

    let rkth = cert_block.rkth();
    let signature_path = args.signature_path_with_default();

    if !args.dont_sign {
        log::info!("Signing image");

        let Some(cert_chain) = config.certificates.get(args.certificate) else {
            return Err(anyhow::anyhow!("Certificate chain {} does not exist", args.certificate));
        };

        let Some(cert) = cert_chain.0.last() else {
            return Err(anyhow::anyhow!("Empty certificate chain"));
        };

        let Some(cert_proto) = &cert.prototype else {
            return Err(anyhow::anyhow!(
                "No prototype configured for leaf of chain {}",
                args.certificate
            ));
        };

        mbi::sign(&signature_path, &output_prestage_path, &cert_proto.key_path).context("Could not sign image")?;

        let output_path = args.output_path_with_default();
        log::info!("Merging signature into image");
        mbi::merge_with_signature(
            &output_unsigned_path,
            base_addr,
            signature_path,
            &output_path,
            is_bootloader,
            Some(otp),
            cert_block,
        )
        .context("Could not merge image with signature")?;
        log::info!("Written merged image to {}", output_path.display());
        Ok(SignOutput {
            output_path: Some(output_path),
            rkth,
        })
    } else {
        Ok(SignOutput {
            output_path: None,
            rkth,
        })
    }
}
