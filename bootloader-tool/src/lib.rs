use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub use crate::config::Config;

pub mod commands;
mod config;
pub mod processors;
mod util;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE", default_value = "./config.toml")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub commands: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
    /// Run binaries by going through the bootloader chain for testing purposes
    Run {
        #[command(subcommand)]
        subcommand: RunCommands,
    },
    /// Burn fuse registers with key material and settings
    Fuse,
}

#[derive(Args, Debug, Clone)]
pub struct GenerateCertificatesArguments {
    /// Where the nxpcrypto binary can be found. May be on PATH
    #[arg(long, default_value = "nxpcrypto")]
    nxpcrypto_path: PathBuf,

    /// Where the nxpimage binary can be found. May be on PATH
    #[arg(long, default_value = "nxpimage")]
    nxpimage_path: PathBuf,
}

#[derive(Subcommand, Debug, Clone)]
pub enum GenerateCommands {
    /// Generate the certificates, certificate block and RKTH
    Certificates(GenerateCertificatesArguments),
    /// Generate an OTP encryption master key (used for header integrity validation)
    Otp,
}

#[derive(Args, Debug, Clone)]
pub struct SignArguments {
    /// Input file path (ELF)
    #[arg(short, long, value_name = "INPUT_FILE")]
    input_path: PathBuf,
    /// Signature file
    ///
    /// If present, will be checked against image and merged into output path
    #[arg(short, long, value_name = "SIGNATURE_FILE")]
    signature_path: Option<PathBuf>,
    /// Output file path of unsigned application (BIN) [default: <INPUT_FILE>.unsigned.bin]
    #[arg(long, value_name = "OUTPUT_UNSIGNED_FILE")]
    output_unsigned_path: Option<PathBuf>,
    /// Output file path of unsigned Master Boot Image (BIN, without signature) [default: <INPUT_FILE>.mbi-proto.bin]
    #[arg(long, value_name = "OUTPUT_PRESTAGE_FILE")]
    output_prestage_path: Option<PathBuf>,
    /// Output file path (BIN) [default: <INPUT_FILE>.signed.bin]
    #[arg(short, long, value_name = "OUTPUT_FILE")]
    output_path: Option<PathBuf>,
    /// Do not actually sign the image only export the prestage for external signing by HSM
    #[arg(long)]
    dont_sign: bool,
    /// Index of the certificate intended to sign the image with
    ///
    /// Used to generate the appropriate certificate block for this image
    ///
    /// When this tool is used to generate a signature, the private key also needs to be configured
    #[arg(long, value_name = "CERTIFICATE_IDX", default_value = "0")]
    certificate: usize,
    /// Prelude output file path (BIN) [default: <INPUT_FILE>.prelude.bin]
    #[arg(long)]
    prelude_path: Option<PathBuf>,
    /// Where the nxpimage binary can be found. May be on PATH
    #[arg(long, default_value = "nxpimage")]
    nxpimage_path: PathBuf,
}

impl SignArguments {
    pub fn output_unsigned_path_with_default(&self) -> PathBuf {
        self.output_unsigned_path
            .clone()
            .unwrap_or_else(|| self.input_path.clone().with_extension("unsigned.bin"))
    }

    pub fn output_prestage_path_with_default(&self) -> PathBuf {
        self.output_prestage_path
            .clone()
            .unwrap_or_else(|| self.input_path.clone().with_extension("mbi-proto.bin"))
    }

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
pub enum SignCommands {
    /// Sign a bootloader image
    Bootloader(SignArguments),
    /// Sign an application image
    Application(SignArguments),
}

#[derive(Args, Debug, Clone)]
pub struct RunArguments {
    #[command(flatten)]
    sign_args: SignArguments,

    #[command(flatten)]
    probe_args: ProbeArgs,

    /// Where the probe-rs binary can be found. May be on PATH
    #[arg(long, default_value = "probe-rs")]
    probe_rs_path: PathBuf,
}

#[derive(Args, Debug, Clone)]
pub struct ProbeArgs {
    /// Which probe to use (passed to probe-rs)
    #[arg(short, long, value_name = "PROBE")]
    probe: Option<String>,

    /// Type of chip to be programmed (passed to probe-rs)
    #[arg(short, long, value_name = "CHIP", default_value = "MIMXRT685SFVKB")]
    chip: String,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RunCommands {
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
pub enum DownloadCommands {
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
