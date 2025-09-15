//! OTP shadow registers

/// A Root Key Table Hash.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rkth(pub [u8; 32]);

impl Rkth {
    const PTR: *mut [u8; 32] = 0x401301E0 as *mut [u8; 32];
    const OTP_ADDR: u32 = 0x1E0;

    pub fn read_fuse(otp: &mut crate::otp::Otp) -> Result<Self, crate::otp::Error> {
        let mut data = [0u32; 8];
        for i in 0..8 {
            data[i] = otp.read_fuse(Self::OTP_ADDR + i as u32)?;
        }

        let data: [u8; 32] = unsafe { core::mem::transmute(data) };
        Ok(Self(data))
    }

    pub fn write_fuse(&self, otp: &mut crate::otp::Otp, lock: bool) -> Result<(), crate::otp::Error> {
        let data: [u32; 8] = unsafe { core::mem::transmute(self.0) };
        for i in 0..8 {
            let addr = Self::OTP_ADDR + i as u32;
            defmt_or_log::info!("Writing fuse {:x} with {:x} (lock: {})", addr, data[i], lock);
            otp.write_fuse(addr, data[i], lock)?;
        }
        Ok(())
    }

    pub fn read_shadow() -> Self {
        Self(unsafe { Self::PTR.read_volatile() })
    }

    pub fn write_shadow(&self) {
        unsafe { Self::PTR.write_volatile(self.0) }
    }
}

#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Otp(pub [u8; 32]);

impl Otp {
    const PTR: *mut [u8; 32] = 0x401301C0 as *mut [u8; 32];

    pub fn read_shadow() -> Self {
        Self(unsafe { Self::PTR.read_volatile() })
    }

    pub fn write_shadow(&self) {
        unsafe { Self::PTR.write_volatile(self.0) }
    }
}

#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Boot0(pub u32);

impl Boot0 {
    const PTR: *mut u32 = 0x40130180 as *mut u32;

    pub fn read_shadow() -> Self {
        Self(unsafe { Self::PTR.read_volatile() })
    }

    pub fn secure_boot(&self) -> bool {
        // 0b01, 0b10 and 0b11 imply 'secure boot'.
        self.0 >> 20 & 0b11 != 0b00
    }
}

#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Boot1(pub u32);

impl Boot1 {
    const PTR: *mut u32 = 0x40130184 as *mut u32;

    pub fn read_shadow() -> Self {
        Self(unsafe { Self::PTR.read_volatile() })
    }

    pub fn write_shadow(&self) {
        unsafe { Self::PTR.write_volatile(self.0) }
    }

    pub fn set_qspi_reset_pin(&mut self, port: u8, pin: u8) {
        self.0 &= 0b1_1111_1111 << 14;
        self.0 |= 1 << 14; // Reset pin enable.
        self.0 |= (port as u32) << 15; // Reset pin port 2.
        self.0 |= (pin as u32) << 18; // Reset pin number 12.
    }
}
