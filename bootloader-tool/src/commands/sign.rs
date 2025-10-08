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
        if let Some(bootloader) = &config.bootloader
            && bootloader.run_start != base_addr as u64
        {
            return Err(anyhow::anyhow!(
                "Bootloader image will be run from unexpected address 0x{:x}, should be 0x{:x}",
                base_addr,
                bootloader.run_start
            ));
        }
    } else if let Some(application) = &config.application
        && application.run_start != base_addr as u64
    {
        return Err(anyhow::anyhow!(
            "Application image will be run from unexpected address 0x{:x}, should be 0x{:x}",
            base_addr,
            application.run_start
        ));
    }

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

    let mut signature_path = args.signature_path.clone();

    if !args.dont_sign && signature_path.is_none() {
        log::info!("Signing image {}", args.input_path.display());

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

        let default_path = args.input_path.clone().with_extension("signature.bin");
        mbi::sign(&default_path, &output_prestage_path, &cert_proto.key_path).context("Could not sign image")?;
        signature_path = Some(default_path);
    }

    let rkth = cert_block.rkth();

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
