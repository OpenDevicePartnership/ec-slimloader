//! Registers that are available as OTP fuses and as shadow registers.
#![allow(dead_code)]

use device_driver::{FieldSet, RegisterInterface};

use crate::otp::Otp;

// Define a Device for all OTP registers,that exist both as fuses accessible from the OTP ROM API as well as the shadow registers.
device_driver::create_device!(
    device_name: Device,
    manifest: "registers.json"
);

/// Interface to access the shadow registers.
pub struct ShadowInterface {
    _private: (),
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NotShadowRegister;

/// The index of an OTP word as defined in the OTP Fuse Map -- 220315.
type OtpWordIndex = u32;

/// A single OTP word as required by the OTP API.
///
/// Note that an OTP register can be smaller than 32 bits.
/// The API however requires the register to then be stretched to 32 bits before being written.
type OtpWord = u32;

/// Convert an OTP word index to offset in the shadow register block.
const fn otp_to_shadow_offset(otp_word_i: OtpWordIndex) -> Result<usize, NotShadowRegister> {
    let shadow_offset = match otp_word_i {
        8..=9 => (otp_word_i - 8) * 4 + 0x020,
        95..=104 => (otp_word_i - 95) * 4 + 0x17C,
        106..=127 => (otp_word_i - 106) * 4 + 0x1A8,
        492..=495 => (otp_word_i - 492) * 4 + 0x7B0,
        _ => return Err(NotShadowRegister),
    };

    Ok(shadow_offset as usize)
}

/// Convert an OTP word index to address in the shadow register block.
fn otp_to_shadow_addr(otp_word_i: OtpWordIndex) -> Result<*mut u32, NotShadowRegister> {
    const OTP_SHADOW_BASE_ADDR: usize = 0x40130000;
    Ok((OTP_SHADOW_BASE_ADDR + otp_to_shadow_offset(otp_word_i)?) as *mut u32)
}

/// Converts a slice into a sequence of words to be written to either shadow registers or OTP fuses.
fn data_to_otp_words(otp_word_i: OtpWordIndex, data: &[u8]) -> impl Iterator<Item = (OtpWordIndex, OtpWord)> + '_ {
    data.chunks(core::mem::size_of::<OtpWord>())
        .enumerate()
        .map(move |(chunk_i, chunk)| {
            let otp_word_i = otp_word_i + chunk_i as u32;

            let word = if chunk.len() < core::mem::size_of::<OtpWord>() {
                let mut buf = [0u8; core::mem::size_of::<OtpWord>()];
                buf[..chunk.len()].copy_from_slice(chunk);
                OtpWord::from_le_bytes(buf)
            } else {
                // Safety: we have chunks of exactly core::mem::size_of::<OtpWord>() bytes, hence the conversion to [u8; 4] is safe.
                OtpWord::from_le_bytes(unsafe { chunk.try_into().unwrap_unchecked() })
            };

            (otp_word_i, word)
        })
}

fn otp_words_to_data<E>(
    otp_word_i: OtpWordIndex,
    data: &mut [u8],
    mut f: impl FnMut(OtpWordIndex) -> Result<OtpWord, E>,
) -> Result<(), E> {
    for (chunk_i, chunk) in data.chunks_mut(core::mem::size_of::<OtpWord>()).enumerate() {
        let otp_word_i = otp_word_i + chunk_i as u32;
        let word = f(otp_word_i)?.to_le_bytes();

        // Note: if the chunk is smaller (example: 16 bits), only copy the LE bytes at the front of the slice.
        chunk.copy_from_slice(&word[..chunk.len()]);
    }

    Ok(())
}

impl RegisterInterface for ShadowInterface {
    type Error = NotShadowRegister;
    type AddressType = OtpWordIndex;

    fn write_register(
        &mut self,
        otp_word_i: Self::AddressType,
        _size_bits: u32,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        for (otp_word_i, word) in data_to_otp_words(otp_word_i, data) {
            let shadow_addr = otp_to_shadow_addr(otp_word_i)?;

            // Safety: we assume that the register yaml definition is correct, and that each register is aligned.
            unsafe { shadow_addr.write_volatile(word) };
        }
        Ok(())
    }

    fn read_register(
        &mut self,
        otp_word_i: Self::AddressType,
        _size_bits: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        otp_words_to_data(otp_word_i, data, |otp_word_i| {
            let shadow_addr = otp_to_shadow_addr(otp_word_i)? as *const u32;
            // Safety: we assume that the register yaml definition is correct, and that each register is aligned.
            Ok(unsafe { shadow_addr.read_volatile() })
        })?;
        Ok(())
    }
}

pub struct OtpInterface<'a> {
    otp: &'a mut Otp,
    allow_write: bool,
    mode_locked: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OtpError {
    WriteNotAllowed,
    Inner(crate::otp::Error),
}

impl From<crate::otp::Error> for OtpError {
    fn from(value: crate::otp::Error) -> Self {
        OtpError::Inner(value)
    }
}

impl RegisterInterface for OtpInterface<'_> {
    type Error = OtpError;
    type AddressType = OtpWordIndex;

    fn write_register(
        &mut self,
        otp_word_i: Self::AddressType,
        _size_bits: u32,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        if !self.allow_write {
            return Err(OtpError::WriteNotAllowed);
        }

        for (otp_word_i, word) in data_to_otp_words(otp_word_i, data) {
            self.otp.write_fuse(otp_word_i, word, self.mode_locked)?;
        }

        Ok(())
    }

    fn read_register(
        &mut self,
        otp_word_i: Self::AddressType,
        _size_bits: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        otp_words_to_data(otp_word_i, data, |otp_word_i| self.otp.read_fuse(otp_word_i))?;
        Ok(())
    }
}

pub struct ShadowRegisters {
    device: Device<ShadowInterface>,
}

impl ShadowRegisters {
    pub const fn new() -> Self {
        Self {
            device: Device::new(ShadowInterface { _private: () }),
        }
    }
}

impl Default for ShadowRegisters {
    fn default() -> Self {
        Self::new()
    }
}

impl core::ops::Deref for ShadowRegisters {
    type Target = Device<ShadowInterface>;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl core::ops::DerefMut for ShadowRegisters {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}

pub struct OtpFuses<'a> {
    device: Device<OtpInterface<'a>>,
}

impl<'a> OtpFuses<'a> {
    pub fn readonly(otp: &'a mut Otp) -> Self {
        Self {
            device: Device::new(OtpInterface {
                otp,
                allow_write: false,
                mode_locked: false,
            }),
        }
    }

    pub fn writable(otp: &'a mut Otp, mode_locked: bool) -> Self {
        Self {
            device: Device::new(OtpInterface {
                otp,
                allow_write: true,
                mode_locked,
            }),
        }
    }
}

impl<'a> core::ops::Deref for OtpFuses<'a> {
    type Target = Device<OtpInterface<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

impl core::ops::DerefMut for OtpFuses<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.device
    }
}

impl core::fmt::Display for field_sets::Rkth {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for b in self.get_inner_buffer() {
            f.write_fmt(format_args!("{:02x}", b))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(otp_to_shadow_offset(96), Ok(0x180)); // BOOT_CFG[0]
        assert_eq!(otp_to_shadow_offset(97), Ok(0x184)); // BOOT_CFG[1]
        assert_eq!(otp_to_shadow_offset(101), Ok(0x194)); // SEC_BOOT_CFG[5]
        assert_eq!(otp_to_shadow_offset(120), Ok(0x1E0)); // RKTH[0]
        assert_eq!(otp_to_shadow_offset(127), Ok(0x1FC)); // RKTH[7]
    }

    /// Test reading registers that are smaller than the OTP fuse word.
    #[test]
    fn read_words_partial() {
        let target = 0x1234u16;
        let target_buf = target.to_le_bytes();

        assert_eq!(target_buf, [0x34, 0x12]);

        let otp_words: std::vec::Vec<_> = data_to_otp_words(100, &target_buf).collect();
        assert_eq!(otp_words, [(100, 0x00001234)]);
    }

    /// Test writing registers that are smaller than the OTP fuse word.
    #[test]
    fn write_words_partial() {
        let mut buf = [0u8; 2];
        assert!(
            otp_words_to_data::<core::convert::Infallible>(100, &mut buf, |otp_word_i| {
                assert_eq!(otp_word_i, 100);
                Ok(0x00001234)
            })
            .is_ok()
        );
        assert_eq!(buf, [0x34, 0x12]);
    }

    const MULTIPLE_DATASET: [(OtpWordIndex, OtpWord); 8] = [
        (100, 0x04030201),
        (101, 0x08070605),
        (102, 0x0C0B0A09),
        (103, 0x100F0E0D),
        (104, 0x14131211),
        (105, 0x18171615),
        (106, 0x1C1B1A19),
        (107, 0x201F1E1D),
    ];

    fn generate_multiple_bytebuf() -> [u8; 32] {
        let mut target_buf: [u8; 32] = [0u8; 32];
        for (i, b) in target_buf.iter_mut().enumerate() {
            *b = (i + 1) as u8;
        }
        target_buf
    }

    /// Test reading registers that are several OTP words large.
    #[test]
    fn read_words_multiple() {
        let otp_words: std::vec::Vec<_> = data_to_otp_words(100, &generate_multiple_bytebuf()).collect();
        assert_eq!(otp_words, MULTIPLE_DATASET);
    }

    /// Test writing registers that are several OTP words large.
    #[test]
    fn write_words_multiple() {
        let mut buf = [0u8; 32];
        assert!(
            otp_words_to_data::<core::convert::Infallible>(100, &mut buf, |otp_word_i| {
                let (_, word) = MULTIPLE_DATASET
                    .into_iter()
                    .find(move |(i, _)| *i == otp_word_i)
                    .expect("Register should exist in the dataset");
                Ok(word)
            })
            .is_ok()
        );
        assert_eq!(buf, generate_multiple_bytebuf());
    }
}
