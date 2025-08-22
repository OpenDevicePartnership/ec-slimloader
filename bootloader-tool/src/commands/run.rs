use probe_rs::MemoryInterface;

use crate::{
    DownloadCommands, RunCommands,
    config::Config,
    processors::{certificates, otp},
};

pub async fn process(config: &Config, command: RunCommands) -> anyhow::Result<()> {
    log::debug!("Fetching key material...");
    let otp = otp::get_otp(config)?;
    let rkth = certificates::get_rkth(config)?;

    let mut session =
        super::download::process(config, DownloadCommands::Other(command.clone())).await?;

    let mut core = session.core(0)?;

    log::info!("Setting shadow registers on target");
    core.write_32(0x401301E0, &rkth.as_u32_le())?;
    core.write_32(0x401301C0, &otp.as_reversed_u32_be())?;

    // Enable secure boot, skip DICE
    core.write_32(0x40130180, &[0x1E900000])?;

    let mut buf = [0u32; 1];
    core.read_32(0x40130194, &mut buf)?;

    // Set USE_PUF to 0
    buf[0] &= !(1 << 7);

    core.write_32(0x40130194, &buf)?;

    //     core.read_32(0x40130020, &mut buf).unwrap();
    //     // Set OTP write lock
    //     buf[0] |= 1 << 8;
    //     core.write_32(0x40130020, &buf).unwrap();
    //     eprintln!("CUST_WR_RD_LOCK0 {:02x}", buf[0]);

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
