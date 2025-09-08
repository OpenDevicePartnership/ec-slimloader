use aes::{
    Aes256,
    cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray},
};
use anyhow::Context;

use crate::{
    config::Config,
    util::{bytes_to_u32_be, generate_hex, parse_hex},
};

#[derive(Clone)]
pub struct Otp(pub [u8; 32]);

impl Otp {
    pub fn generate() -> Self {
        let mut buf = [0u8; 32];
        rand::fill(&mut buf);
        Self(buf)
    }

    pub fn as_hex(&self) -> String {
        generate_hex(&self.0)
    }

    pub fn from_hex(str: &str) -> anyhow::Result<Self> {
        Ok(Self(parse_hex(str)?.try_into().map_err(|_| {
            anyhow::anyhow!("Input not appropriate size")
        })?))
    }

    pub fn as_reversed_u32_be(&self) -> Vec<u32> {
        let mut result = bytes_to_u32_be(&self.0);
        result.reverse();
        result
    }

    pub fn hmac_key(&self) -> anyhow::Result<HmacKey> {
        // See UM11147 page 1246, 43.2.3.1 HMAC_KEY
        let aes = Aes256::new_from_slice(&self.0)?;
        let mut block = GenericArray::from([0u8; 16]);
        aes.encrypt_block(&mut block);

        Ok(HmacKey(block.as_slice().try_into().unwrap()))
    }
}

pub struct HmacKey(pub [u8; 16]);

pub fn generate(config: &Config) -> anyhow::Result<Otp> {
    if std::fs::exists(&config.otp_path)? {
        log::warn!(
            "OTP file {} already generated, skipping...",
            &config.otp_path.display()
        );
        return get_otp(config);
    }

    let otp = Otp::generate();
    std::fs::write(&config.otp_path, otp.as_hex())?;

    log::info!("Generated and wrote OTP key");
    Ok(otp)
}

pub fn get_otp(config: &Config) -> anyhow::Result<Otp> {
    let path = &config.otp_path;
    let otp_hex_str = std::fs::read_to_string(&config.otp_path)
        .with_context(|| format!("Failed to open OTP file {}", path.display()))?;

    Otp::from_hex(&otp_hex_str)
}
