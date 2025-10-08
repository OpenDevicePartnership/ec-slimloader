use std::path::Path;

use DownloadCommands::Other;
use anyhow::Context;
use probe_rs::flashing::ElfOptions;
use probe_rs::{Session, flashing};

use crate::commands::sign::SignOutput;
use crate::config::Config;
use crate::processors::certificates::Rkth;
use crate::processors::probe;
use crate::{DownloadCommands, ProbeArgs, RunCommands, SignCommands};

pub async fn process(config: &Config, command: DownloadCommands) -> anyhow::Result<()> {
    match command {
        DownloadCommands::Prelude {
            prelude_path,
            probe_args,
        } => {
            download_prelude(&prelude_path, &probe_args).await?;
        }
        Other(args) => {
            process_other(config, args).await?;
        }
    };
    Ok(())
}

pub struct DownloadOutput {
    pub session: Session,
    pub rkth: Rkth,
}

pub async fn process_other(config: &Config, command: RunCommands) -> anyhow::Result<DownloadOutput> {
    let (run_args, is_bootloader, flash_start) = match command {
        RunCommands::Bootloader(run_args) => {
            if let Some(bootloader) = &config.bootloader {
                (run_args, true, bootloader.flash_start)
            } else {
                return Err(anyhow::anyhow!("Bootloader not defined in configuration file"));
            }
        }
        RunCommands::Application { run_args, slot } => {
            if let Some(application) = &config.application {
                let flash_start = *application
                    .slot_starts
                    .get(slot as usize)
                    .ok_or_else(|| anyhow::anyhow!(format!("Slot {} not defined in configuration file", slot)))?;
                (run_args, false, flash_start)
            } else {
                return Err(anyhow::anyhow!("Bootloader not defined in configuration file"));
            }
        }
    };

    log::debug!("Preparing for download by calling sign...");

    let sign_command = if is_bootloader {
        SignCommands::Bootloader(run_args.sign_args.clone())
    } else {
        SignCommands::Application(run_args.sign_args.clone())
    };

    let SignOutput { output_path, rkth } = super::sign::process(config, sign_command).await?;

    let Some(output_path) = output_path else {
        return Err(anyhow::anyhow!("Image was not signed so nothing to run"));
    };

    log::debug!("Starting probe session...");
    let mut session = probe::start_session(&run_args.probe_args.chip, run_args.probe_args.probe.clone()).await?;

    // NOTE: First flash then set secure boot configuration! Doing it the other way around causes
    // flashing to almost always fail.

    log::info!(
        "Flashing {} to target at address 0x{:02x}",
        output_path.display(),
        flash_start
    );

    let options = flashing::DownloadOptions::default();
    flashing::download_file_with_options(
        &mut session,
        output_path,
        flashing::Format::Bin(flashing::BinOptions {
            base_address: Some(flash_start),
            skip: 0,
        }),
        options,
    )
    .context("Failed to flash binary")?;

    Ok(DownloadOutput { session, rkth })
}

async fn download_prelude(path: &Path, probe_args: &ProbeArgs) -> anyhow::Result<Session> {
    log::debug!("Starting probe session...");
    let mut session = probe::start_session(&probe_args.chip, probe_args.probe.clone()).await?;

    log::info!("Flashing {} to target", path.display(),);

    let options = flashing::DownloadOptions::default();
    flashing::download_file_with_options(
        &mut session,
        path,
        flashing::Format::Elf(ElfOptions::default()),
        options,
    )
    .context("Failed to flash binary")?;

    Ok(session)
}
