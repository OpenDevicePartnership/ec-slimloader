use crate::SignCommands;
use crate::processors::{mbi, objcopy};
use anyhow::Context;
use object::read::elf::ElfFile32;
use std::path::PathBuf;

pub struct SignOutput {
    pub output_path: PathBuf,
}

pub async fn process(command: SignCommands) -> anyhow::Result<SignOutput> {
    let (is_bootloader, args) = match command {
        SignCommands::Bootloader(sign_arguments) => (true, sign_arguments),
        SignCommands::Application(sign_arguments) => (false, sign_arguments),
    };

    let output_path = args.output_path_with_default();
    let input_data = std::fs::read(&args.input_path)?;

    log::info!("Reading ELF from {}", args.input_path.display());
    let file = ElfFile32::parse(&input_data[..]).context("Could not parse ELF file")?;

    if is_bootloader {
        log::info!("Extracting prelude");
        let out = objcopy::remove_non_prelude(&input_data)?;
        std::fs::write(args.prelude_path_with_default(), &out)
            .context("Could not write prelude elf file")?;
    }

    log::info!("Generating image for {}", args.input_path.display());
    let (image, base_addr) = objcopy::objcopy(&file)?;

    log::info!("Signing image {}", args.input_path.display());

    mbi::generate(
        &args.nxpimage_path,
        &image,
        base_addr,
        &output_path,
        is_bootloader,
    )?;

    Ok(SignOutput { output_path })
}
