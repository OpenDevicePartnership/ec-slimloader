#![allow(unused)]

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct Config {
    /// Path of the directory where artifacts are put and can be found.
    pub artifacts_path: PathBuf,

    /// Path of the file containing the OTP Master Key, used to encrypt the bootloader image.
    pub otp_path: PathBuf,

    /// Certificate chains as used by this project.
    pub certificates: Vec<CertificateChain>,

    /// Arguments related to the setup of the bootloader.
    pub bootloader: Option<BootloaderArgs>,

    /// Arguments related to application images.
    pub application: Option<ApplicationArgs>,
}

#[derive(Deserialize, Debug)]
pub struct CertificateChain(pub Vec<Certificate>);

#[derive(Deserialize, Debug, Clone)]
pub struct Certificate {
    /// Path of the file containing the public facing certificate.
    pub path: PathBuf,

    /// When set, the certificate can be generated and the private key can be directly used to generate signatures for binaries.
    pub prototype: Option<CertificatePrototype>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CertificatePrototype {
    /// Key type to generate.
    pub key_type: KeyType,

    /// Path of the file containing the private key, used to generate signatures for binaries.
    pub key_path: PathBuf,
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

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum KeyType {
    Rsa2048,
    Rsa3072,
    Rsa4096,
}

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::Rsa2048 => "rsa2048",
            KeyType::Rsa3072 => "rsa3072",
            KeyType::Rsa4096 => "rsa4096",
        }
    }
}

impl Config {
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Ok(toml::from_str::<Config>(&std::fs::read_to_string(path)?)?)
    }
}
