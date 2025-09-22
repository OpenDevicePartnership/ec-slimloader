//! Registers that are available as OTP fuses and as shadow registers.

use crate::otp::OtpRegisterBlock;

const fn otp_addr(offset: u32) -> *mut u32 {
    const OTP_SHADOW_BASE_ADDR: usize = 0x40130000;

    (OTP_SHADOW_BASE_ADDR + offset as usize) as *mut u32
}

pub trait ShadowRegister {
    fn read_shadow() -> Self;
    fn write_shadow(&self);
}

macro_rules! define_register {
    ($name:ident, $otp_offset:literal) => {
        #[derive(PartialEq, Debug)]
        #[repr(C)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $name(pub u32);

        impl $name {
            const OTP_OFFSET: u32 = $otp_offset;
            const PTR: *mut u32 = otp_addr(Self::OTP_OFFSET) as *mut u32;
        }

        impl ShadowRegister for $name {
            fn read_shadow() -> Self {
                Self(unsafe { Self::PTR.read_volatile() })
            }

            fn write_shadow(&self) {
                unsafe { Self::PTR.write_volatile(self.0) }
            }
        }

        impl OtpRegisterBlock for $name {
            fn read_fuse(otp: &mut crate::otp::Otp) -> Result<Self, crate::otp::Error> {
                Ok(Self(otp.read_fuse(Self::OTP_OFFSET as u32)?))
            }

            fn write_fuse(&self, otp: &mut crate::otp::Otp, lock: bool) -> Result<(), crate::otp::Error> {
                let addr = Self::OTP_OFFSET as u32;
                defmt_or_log::info!("Writing fuse {:x} with {:x} (lock: {})", addr, self.0, lock);
                otp.write_fuse(addr, self.0, lock)?;
                Ok(())
            }
        }
    };
}

macro_rules! define_register_block {
    ($name:ident, $otp_offset:literal, $register_num:literal) => {
        #[derive(PartialEq, Debug)]
        #[repr(C)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct $name(pub [u32; $register_num]);

        impl $name {
            const OTP_OFFSET: u32 = $otp_offset;
            const PTR: *mut [u32; $register_num] = otp_addr(Self::OTP_OFFSET) as *mut [u32; $register_num];

            pub fn from_bytes(bytes: [u8; $register_num * 4]) -> Self {
                let mut result = [0u32; $register_num];
                for (i, chunk) in bytes.chunks(4).enumerate() {
                    result[i] = u32::from_le_bytes(unsafe { chunk.try_into().unwrap_unchecked() });
                }
                Self(result)
            }
        }

        impl ShadowRegister for $name {
            fn read_shadow() -> Self {
                Self(unsafe { Self::PTR.read_volatile() })
            }

            fn write_shadow(&self) {
                unsafe { Self::PTR.write_volatile(self.0) }
            }
        }

        impl OtpRegisterBlock for $name {
            fn read_fuse(otp: &mut crate::otp::Otp) -> Result<Self, crate::otp::Error> {
                let mut data = [0u32; $register_num];
                for i in 0..$register_num {
                    data[i] = otp.read_fuse(Self::OTP_OFFSET + i as u32)?;
                }
                Ok(Self(data))
            }

            fn write_fuse(&self, otp: &mut crate::otp::Otp, lock: bool) -> Result<(), crate::otp::Error> {
                for i in 0..$register_num {
                    let addr = Self::OTP_OFFSET + i as u32;
                    defmt_or_log::info!("Writing fuse {:x} with {:x} (lock: {})", addr, self.0[i], lock);
                    otp.write_fuse(addr, self.0[i], lock)?;
                }
                Ok(())
            }
        }
    };
}

define_register!(Boot0, 0x180);
define_register!(Boot1, 0x184);
define_register_block!(Rkth, 0x1E0, 8);
define_register_block!(Otp, 0x1C0, 8);

impl Boot0 {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn secure_boot(&self) -> bool {
        // 0b01, 0b10 and 0b11 imply 'secure boot'.
        self.0 >> 20 & 0b11 != 0b00
    }
}

impl Boot1 {
    pub fn set_qspi_reset_pin(&mut self, port: u8, pin: u8) {
        self.0 &= 0b1_1111_1111 << 14;
        self.0 |= 1 << 14; // Reset pin enable.
        self.0 |= (port as u32) << 15; // Reset pin port 2.
        self.0 |= (pin as u32) << 18; // Reset pin number 12.
    }
}
