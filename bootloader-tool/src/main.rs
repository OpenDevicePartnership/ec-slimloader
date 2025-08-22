use crate::config::Config;
use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

extern crate log;
extern crate pretty_env_logger;

mod commands;
mod config;
mod processors;
mod util;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE", default_value = "./config.toml")]
    config: PathBuf,

    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate keys and certificates
    Generate {
        #[command(subcommand)]
        subcommand: GenerateCommands,
    },
    /// Sign binaries for flashing or OTA
    Sign {
        #[command(subcommand)]
        subcommand: SignCommands,
    },
    /// Download binaries to the device
    Download {
        #[command(subcommand)]
        subcommand: DownloadCommands,
    },
    /// Run binaries, setting the shadow registers, by going through the bootloader chain for testing purposes
    Run {
        #[command(subcommand)]
        subcommand: RunCommands,
    },
    /// Burn fuse registers with key material and settings
    Fuse,
}

#[derive(Args, Debug, Clone)]
struct GenerateCertificatesArguments {
    /// Where the nxpcrypto binary can be found. May be on PATH
    #[arg(long, default_value = "nxpcrypto")]
    nxpcrypto_path: PathBuf,

    /// Where the nxpimage binary can be found. May be on PATH
    #[arg(long, default_value = "nxpimage")]
    nxpimage_path: PathBuf,
}

#[derive(Subcommand, Debug, Clone)]
enum GenerateCommands {
    /// Generate the certificates, certificate block and RKTH
    Certificates(GenerateCertificatesArguments),
    /// Generate an OTP encryption master key (used for header integrity validation)
    Otp,
}

#[derive(Args, Debug, Clone)]
struct SignArguments {
    /// Input file path (ELF)
    #[arg(short, long, value_name = "INPUT_FILE")]
    input_path: PathBuf,
    /// Output file path (BIN) [default: <INPUT_FILE>.signed.bin]
    #[arg(short, long, value_name = "OUTPUT_FILE")]
    output_path: Option<PathBuf>,
    /// Prelude output file path (BIN) [default: <INPUT_FILE>.prelude.bin]
    #[arg(long)]
    prelude_path: Option<PathBuf>,
    /// Where the nxpimage binary can be found. May be on PATH
    #[arg(long, default_value = "nxpimage")]
    nxpimage_path: PathBuf,
}

impl SignArguments {
    pub fn output_path_with_default(&self) -> PathBuf {
        self.output_path
            .clone()
            .unwrap_or_else(|| self.input_path.clone().with_extension("signed.bin"))
    }
    pub fn prelude_path_with_default(&self) -> PathBuf {
        self.prelude_path
            .clone()
            .unwrap_or_else(|| self.input_path.clone().with_extension("prelude.elf"))
    }
}

#[derive(Subcommand, Debug, Clone)]
enum SignCommands {
    /// Sign a bootloader image
    Bootloader(SignArguments),
    /// Sign an application image
    Application(SignArguments),
}

#[derive(Args, Debug, Clone)]
struct RunArguments {
    #[command(flatten)]
    sign_args: SignArguments,

    #[command(flatten)]
    probe_args: ProbeArgs,

    /// Where the probe-rs binary can be found. May be on PATH
    #[arg(long, default_value = "probe-rs")]
    probe_rs_path: PathBuf,
}

#[derive(Args, Debug, Clone)]
struct ProbeArgs {
    /// Which probe to use (passed to probe-rs)
    #[arg(short, long, value_name = "PROBE")]
    probe: Option<String>,

    /// Type of chip to be programmed (passed to probe-rs)
    #[arg(short, long, value_name = "CHIP", default_value = "MIMXRT685SFVKB")]
    chip: String,
}

#[derive(Subcommand, Debug, Clone)]
enum RunCommands {
    /// Sign a bootloader image
    Bootloader(RunArguments),
    /// Run an application image in a preferred slot
    ///
    /// Will also set the appropriate 2nd stage bootloader state to start up the image
    Application {
        #[command(flatten)]
        run_args: RunArguments,

        /// Image slot to which to upload the binary to
        #[arg(long, default_value_t = 0)]
        slot: u8,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum DownloadCommands {
    /// Download the flash prelude containing OTFAD, FCB, etc.
    Prelude {
        /// Path to the ELF file containing the prelude
        #[arg(long)]
        prelude_path: PathBuf,

        #[command(flatten)]
        probe_args: ProbeArgs,
    },

    #[command(flatten)]
    Other(RunCommands),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let cli = Cli::parse();

    let config = Config::read(&cli.config)
        .with_context(|| format!("Tried to open --config {}", cli.config.display()))?;

    if let Some(command) = cli.commands {
        commands::process(&config, command).await
    } else {
        eprintln!("Done nothing");
        Ok(())
    }
}
