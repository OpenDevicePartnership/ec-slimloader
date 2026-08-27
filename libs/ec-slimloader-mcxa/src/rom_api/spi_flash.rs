#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Safety: null, misaligned, invalid inputs or out of bounds references will cause ROM API to return an error code, which is handled by the caller.
use core::ffi::c_void;

use super::Status;
use crate::error::SpiFlashStatus;

// LPSPI external flash (SPI NOR/EEPROM) ROM API

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SpiMemConfigOption {
    pub option0: u32,
    pub option1: u32,
}

#[repr(C)]
pub(super) struct SpiFlashDriverRaw {
    pub spi_eeprom_init: unsafe extern "C" fn() -> Status,
    pub spi_eeprom_read: unsafe extern "C" fn(address: u32, NoOfBytes: u32, buffer: *mut u8) -> Status,
    pub spi_eeprom_write: unsafe extern "C" fn(address: u32, NoOfBytes: u32, buffer: *const u8) -> Status,
    pub spi_eeprom_erase: unsafe extern "C" fn(address: u32, length: u32) -> Status,
    pub spi_eeprom_config: unsafe extern "C" fn(config: *mut u32) -> Status,
    pub spi_eeprom_flush: unsafe extern "C" fn() -> Status,
    pub reserved0: *mut c_void,
    pub spi_eeprom_erase_all: unsafe extern "C" fn() -> Status,
}

#[derive(Clone, Copy)]
pub struct SpiFlashDriver {
    raw: &'static SpiFlashDriverRaw,
}

impl SpiFlashDriver {
    pub(super) const fn from_raw(raw: &'static SpiFlashDriverRaw) -> Self {
        Self { raw }
    }

    pub fn spi_eeprom_init(&self) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_init)()) }
    }

    pub fn spi_eeprom_read(&self, address: u32, no_of_bytes: u32, buffer: *mut u8) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_read)(address, no_of_bytes, buffer)) }
    }

    pub fn spi_eeprom_write(&self, address: u32, no_of_bytes: u32, buffer: *const u8) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_write)(address, no_of_bytes, buffer)) }
    }

    pub fn spi_eeprom_erase(&self, address: u32, length: u32) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_erase)(address, length)) }
    }

    pub fn spi_eeprom_config(&self, config: *mut u32) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_config)(config)) }
    }

    pub fn spi_eeprom_flush(&self) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_flush)()) }
    }

    pub fn spi_eeprom_erase_all(&self) -> SpiFlashStatus {
        unsafe { SpiFlashStatus::from((self.raw.spi_eeprom_erase_all)()) }
    }
}
