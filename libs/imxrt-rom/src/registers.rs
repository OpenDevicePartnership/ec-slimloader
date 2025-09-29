//! Registers that are available as OTP fuses and as shadow registers.

use crate::otp::Otp;

use device_driver::RegisterInterface;

// Define a Device for all OTP registers,that exist both as fuses accessible from the OTP ROM API as well as the shadow registers.
device_driver::create_device!(
    device_name: Device,
    manifest: "registers.yaml"
);

/// Interface to access the shadow registers.
pub struct ShadowInterface {
    _private: (),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NotShadowRegister;

/// Convert an OTP word index to address in the shadow register block.
const fn otp_to_shadow_addr(otp_word_i: u32) -> Result<*mut u32, NotShadowRegister> {
    const OTP_SHADOW_BASE_ADDR: usize = 0x40130000;

    let shadow_offset = match otp_word_i {
        8..=9 => (otp_word_i - 8) * 4 + 0x020,
        95..=127 => (otp_word_i - 95) * 4 + 0x17C,
        492..=495 => (otp_word_i - 492) * 4 + 0x7B0,
        _ => return Err(NotShadowRegister),
    };

    Ok((OTP_SHADOW_BASE_ADDR + shadow_offset as usize) as *mut u32)
}

impl RegisterInterface for ShadowInterface {
    type Error = NotShadowRegister;
    type AddressType = u32;

    fn write_register(
        &mut self,
        otp_word_i: Self::AddressType,
        _size_bits: u32,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        for (chunk_i, chunk) in data.chunks_exact(4).enumerate() {
            let otp_word_i = otp_word_i + chunk_i as u32;
            let shadow_addr = otp_to_shadow_addr(otp_word_i)?;

            // Safety: we have chunks of exactly 4 bytes, hence the conversion to [u8; 4] is safe.
            let word = u32::from_le_bytes(unsafe { chunk.try_into().unwrap_unchecked() });

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
        // Safety: we assume that the register yaml definition is correct, no need for volatile memory access.
        let shadow_addr = otp_to_shadow_addr(otp_word_i)? as *const u8;
        let source = unsafe { core::slice::from_raw_parts(shadow_addr, data.len()) };
        data.copy_from_slice(source);
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
    type AddressType = u32;

    fn write_register(&mut self, address: Self::AddressType, _size_bits: u32, data: &[u8]) -> Result<(), Self::Error> {
        if !self.allow_write {
            return Err(OtpError::WriteNotAllowed);
        }

        for (i, chunk) in data.chunks_exact(4).enumerate() {
            // Safety: we have chunks of exactly 4 bytes, hence the conversion to [u8; 4] is safe.
            let word = u32::from_le_bytes(unsafe { chunk.try_into().unwrap_unchecked() });

            self.otp.write_fuse(address + i as u32, word, self.mode_locked)?;
        }
        Ok(())
    }

    fn read_register(
        &mut self,
        address: Self::AddressType,
        _size_bits: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        for (i, chunk) in data.chunks_exact_mut(4).enumerate() {
            chunk.copy_from_slice(&self.otp.read_fuse(address + i as u32)?.to_le_bytes());
        }
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
