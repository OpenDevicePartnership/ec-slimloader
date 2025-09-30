use probe_rs::MemoryInterface;

use crate::{RunCommands, commands::download::DownloadOutput, config::Config, processors::otp};

pub async fn process(config: &Config, command: RunCommands) -> anyhow::Result<()> {
    let otp = otp::get_otp(config)?;

    log::debug!("Preparing for run by calling download...");
    let DownloadOutput { mut session, rkth } = super::download::process_other(config, command.clone()).await?;

    let mut core = session.core(0)?;

    log::info!("Setting shadow registers on target");
    core.write_32(0x401301E0, &rkth.as_u32_le())?;
    core.write_32(0x401301C0, &otp.as_reversed_u32_be())?;

    // Enable secure boot, skip DICE
    let mut boot0 = 0u32;
    boot0 |= 0b0101; // Use QSPI B
    boot0 |= 0b111 << 4; // Completely disable ISP mode
    boot0 |= 0b10 << 13; // Force Trust-Zone mode
    boot0 |= 0b01 << 20; // Enable secure boot
    boot0 |= 0b1 << 23; // Skip DICE
    boot0 |= 0b101 << 24; // Configure boot_fail_pin port 5
    boot0 |= 0b00111 << 27; // Configure boot_fail_pin pin 7

    core.write_32(0x40130180, &[boot0])?;

    let mut boot1 = 0u32;
    boot1 |= 1 << 14; // Reset pin enable.
    boot1 |= 2 << 15; // Reset pin port 2.
    boot1 |= 12 << 18; // Reset pin number 12.

    core.write_32(0x40130184, &[boot1])?;

    let mut buf = [0u32; 1];
    core.read_32(0x40130194, &mut buf)?;

    // buf[0] |= 0b1111; // Revoke root cert 2.
    buf[0] &= !(1 << 7); // Set USE_PUF to 0

    core.write_32(0x40130194, &buf)?;

    core.reset().unwrap();
    drop(core);
    drop(session);

    log::info!("Target configured and reset, attaching...");

    let (RunCommands::Bootloader(run_args) | RunCommands::Application { run_args, .. }) = command;

    let mut command = std::process::Command::new(&run_args.probe_rs_path);
    command.args(["attach", "--chip", &run_args.probe_args.chip]);

    if let Some(probe) = run_args.probe_args.probe.as_ref() {
        command.args(["--probe", probe]);
    }

    command.arg(run_args.sign_args.input_path).status().unwrap();

    Ok(())
}
