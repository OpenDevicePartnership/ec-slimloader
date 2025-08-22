#![allow(unused)]

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct Config {
    /// Path of the directory where artifacts are put and can be found.
    pub artifacts_path: PathBuf,

    /// Path of the file containing the Root Key Table Hash, generated from the Certificate Block.
    ///
    /// This hash is either uploaded to the shadow registers when testing, or fused to the permanent register.
    pub rkth_path: PathBuf,

    /// Path of the file containing the OTP Master Key, used to encrypt the bootloader image.
    pub otp_path: PathBuf,

    /// Arguments related to the setup of the bootloader.
    pub bootloader: Option<BootloaderArgs>,

    /// Arguments related to application images.
    pub application: Option<ApplicationArgs>,
}

#[derive(Deserialize, Debug)]
pub struct MemoryRange {
    pub start: u64,
    pub size: u64,
}

#[derive(Deserialize, Debug)]
pub struct BootloaderArgs {
    /// Location in external NOR flash in which the bootloader should live. (must be 0x08001000)
    pub flash_start: u64,
    /// Location in RAM which the bootloader should run from. (can be anything in RAM)
    pub run_start: u64,
    /// Maximum binary size of the image. (including certificates, hashes and encryption key)
    pub max_size: u64,
    /// Memory location of bootloader state.
    ///
    /// Used to set a new state when ordering to start a specific application image slot.
    pub state: MemoryRange,
}

#[derive(Deserialize, Debug)]
pub struct ApplicationArgs {
    /// Starting addresses in external NOR flash for each slot.
    pub slot_starts: Vec<u64>,
    /// Starting RAM address for all images.
    ///
    /// This address is hard-coded and checked in the bootloader.
    pub run_start: u64,
    /// Exactly the slot size, which is also the maximum size of the binary image. (including certificates and hashes)
    ///
    /// This size is hard-coded and checked in the bootloader.
    pub slot_size: u64,
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(toml::from_str::<Config>(&std::fs::read_to_string(path)?)?)
    }
}
