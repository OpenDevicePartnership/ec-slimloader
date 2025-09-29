//! API to read and manipulate the OTP fuses.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::api::{api_table, KbStatus};

/// Whether the Otp driver has been initialized.
///
/// If initialized without deinitializing, this results in a panic being thrown.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct Otp {
    _private: (),
}

#[derive(Debug)]
#[allow(dead_code)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Error(u32);

impl Otp {
    pub fn init(system_clock_frequency_hz: u32) -> Self {
        if INITIALIZED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            defmt_or_log::panic!("Using OTP whilst it has already been initialized");
        };

        unsafe {
            (api_table().otp_driver.init)(system_clock_frequency_hz);
        }

        Otp { _private: () }
    }
}

impl Otp {
    /// Read the value directly from the fuse.
    pub fn read_fuse(&mut self, addr: u32) -> Result<u32, Error> {
        let mut result = [0u8; 4];
        let status = unsafe { (api_table().otp_driver.fuse_read)(addr, result.as_mut_ptr()) };
        if status == KbStatus::Success as u32 {
            Ok(u32::from_le_bytes(result))
        } else {
            Err(Error(status))
        }
    }

    pub fn write_fuse(&mut self, addr: u32, data: u32, lock: bool) -> Result<(), Error> {
        let status = unsafe { (api_table().otp_driver.fuse_program)(addr, data, lock) };
        if status == KbStatus::Success as u32 {
            Ok(())
        } else {
            defmt_or_log::error!("OTP write failed with {:x}", status);
            Err(Error(status))
        }
    }

    /// Reload all shadow registers from what is stored in OTP fuses.
    pub fn reload_shadow(&mut self) -> Result<(), Error> {
        let status = unsafe { (api_table().otp_driver.reload)() };
        if status == KbStatus::Success as u32 {
            Ok(())
        } else {
            Err(Error(status))
        }
    }
}

impl Drop for Otp {
    fn drop(&mut self) {
        unsafe {
            (api_table().otp_driver.deinit)();
        }

        INITIALIZED.store(false, Ordering::Release);
    }
}
