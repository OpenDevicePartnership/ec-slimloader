use super::{StandardVersion, Status};
use crate::error::FlashStatus;

#[repr(C)]
pub struct FlashFfrConfig {
    // FFR block base address.
    pub ffr_block_base: u32,
    // FFR total size in bytes.
    pub ffr_total_size: u32,
    // FFR page size in bytes.
    pub ffr_page_size: u32,
    // Sector size in bytes.
    pub sector_size: u32,
    // CFPA page version.
    pub cfpa_page_version: u32,
    // CFPA page offset within FFR.
    pub cfpa_page_offset: u32,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashReadEccOption {
    On = 0,
    Off = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashReadMarginOption {
    Normal = 0,
    VsProgram = 1,
    VsErase = 2,
    IllegalBitCombination = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashReadDmaccOption {
    Disabled = 0,
    Enabled = 1,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashRampControlOption {
    Reserved = 0,
    DivisionFactor256 = 1,
    DivisionFactor128 = 2,
    DivisionFactor64 = 3,
}

#[repr(C)]
pub struct FlashReadSingleWordConfig {
    pub packed_options: u8,
    pub reserved1: [u8; 3],
}

impl FlashReadSingleWordConfig {
    #[inline(always)]
    pub const fn new(ecc: FlashReadEccOption, margin: FlashReadMarginOption, dmacc: FlashReadDmaccOption) -> Self {
        Self {
            packed_options: (ecc as u8) | ((margin as u8) << 1) | ((dmacc as u8) << 3),
            reserved1: [0; 3],
        }
    }
}

#[repr(C)]
pub struct FlashSetWriteModeConfig {
    pub program_ramp_control: u8,
    pub erase_ramp_control: u8,
    pub reserved: [u8; 2],
}

impl FlashSetWriteModeConfig {
    #[inline(always)]
    pub const fn new(program: FlashRampControlOption, erase: FlashRampControlOption) -> Self {
        Self {
            program_ramp_control: program as u8,
            erase_ramp_control: erase as u8,
            reserved: [0; 2],
        }
    }
}

#[repr(C)]
pub struct FlashSetReadModeConfig {
    pub read_interface_timing_trim: u16,
    pub read_controller_timing_trim: u16,
    pub read_wait_states: u8,
    pub reserved: [u8; 3],
}

impl FlashSetReadModeConfig {
    #[inline(always)]
    pub const fn new(read_interface_timing_trim: u16, read_controller_timing_trim: u16, read_wait_states: u8) -> Self {
        Self {
            read_interface_timing_trim,
            read_controller_timing_trim,
            read_wait_states,
            reserved: [0; 3],
        }
    }
}

#[repr(C)]
pub struct FlashModeConfig {
    pub sys_freq_in_m_hz: u32,
    pub read_single_word: FlashReadSingleWordConfig,
    pub set_write_mode: FlashSetWriteModeConfig,
    pub set_read_mode: FlashSetReadModeConfig,
}

impl FlashModeConfig {
    #[inline(always)]
    pub const fn new(
        sys_freq_in_m_hz: u32,
        read_single_word: FlashReadSingleWordConfig,
        set_write_mode: FlashSetWriteModeConfig,
        set_read_mode: FlashSetReadModeConfig,
    ) -> Self {
        Self {
            sys_freq_in_m_hz,
            read_single_word,
            set_write_mode,
            set_read_mode,
        }
    }
}

#[repr(C)]
pub struct FlashConfig {
    // P-Flash block base address.
    pub pflash_block_base: u32,
    // P-Flash total size in bytes.
    pub pflash_total_size: u32,
    // P-Flash block count.
    pub pflash_block_count: u32,
    // P-Flash page size in bytes.
    pub pflash_page_size: u32,
    // P-Flash sector size in bytes.
    pub pflash_sector_size: u32,
    // FFR configuration.
    pub ffr_config: FlashFfrConfig,
    // Flash controller parameter configuration.
    pub mode_config: FlashModeConfig,
    // ROM NBOOT context pointer.
    pub nboot_ctx: *mut u32,
    // Use AHB read (true) or alternative read path (false), per ROM.
    pub use_ahb_read: bool,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashPropertyTag {
    PflashSectorSize = 0x00,
    PflashTotalSize = 0x01,
    PflashBlockSize = 0x02,
    PflashBlockCount = 0x03,
    PflashBlockBaseAddr = 0x04,
    PflashPageSize = 0x30,
    PflashSystemFreq = 0x31,
    FfrSectorSize = 0x40,
    FfrTotalSize = 0x41,
    FfrBlockBaseAddr = 0x42,
    FfrPageSize = 0x43,
}

// Flash erase key required by the ROM flash erase-sector entry point.
// Defined as FOUR_CHAR_CODE('l', 'f', 'e', 'k') = (('k' << 24) | ('e' << 16) | ('f' << 8) | ('l'))
pub const FLASH_API_ERASE_KEY: u32 = 0x6b65666c;

#[repr(C)]
pub(super) struct FlashDriverRaw {
    // Initialize flash driver/config.
    pub flash_init: unsafe extern "C" fn(config: *mut FlashConfig) -> Status,
    // Erase sector(s). Requires FLASH_API_ERASE_KEY.
    pub flash_erase_sector:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32, key: u32) -> Status,
    // Program phrase (alignment requirements apply).
    pub flash_program_phrase:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, src: *const u8, length_in_bytes: u32) -> Status,
    // Program page.
    pub flash_program_page:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, src: *const u8, length_in_bytes: u32) -> Status,
    // Verify programmed data.
    pub flash_verify_program: unsafe extern "C" fn(
        config: *mut FlashConfig,
        start: u32,
        length_in_bytes: u32,
        expected_data: *const u8,
        failed_address: *mut u32,
        failed_data: *mut u32,
    ) -> Status,
    // Verify phrase erase.
    pub flash_verify_erase_phrase:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Verify page erase.
    pub flash_verify_erase_page:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Verify sector erase.
    pub flash_verify_erase_sector:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Get flash property.
    pub flash_get_property:
        unsafe extern "C" fn(config: *mut FlashConfig, which_property: u32, value: *mut u32) -> Status,
    // Verify phrase erase in IFR.
    pub ifr_verify_erase_phrase:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Verify page erase in IFR.
    pub ifr_verify_erase_page:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Verify sector erase in IFR.
    pub ifr_verify_erase_sector:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> Status,
    // Read flash into dest.
    pub flash_read:
        unsafe extern "C" fn(config: *mut FlashConfig, start: u32, dest: *mut u8, length_in_bytes: u32) -> Status,
    // Flash API version.
    pub version: StandardVersion,
}

#[derive(Clone, Copy)]
pub struct FlashDriver {
    raw: &'static FlashDriverRaw,
}

impl FlashDriver {
    pub(super) const fn from_raw(raw: &'static FlashDriverRaw) -> Self {
        Self { raw }
    }

    pub fn flash_init(&self, config: *mut FlashConfig) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_init)(config)) }
    }

    pub fn flash_erase_sector(
        &self,
        config: *mut FlashConfig,
        start: u32,
        length_in_bytes: u32,
        key: u32,
    ) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_erase_sector)(config, start, length_in_bytes, key)) }
    }

    pub fn flash_program_phrase(
        &self,
        config: *mut FlashConfig,
        start: u32,
        src: *const u8,
        length_in_bytes: u32,
    ) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_program_phrase)(config, start, src, length_in_bytes)) }
    }

    pub fn flash_program_page(
        &self,
        config: *mut FlashConfig,
        start: u32,
        src: *const u8,
        length_in_bytes: u32,
    ) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_program_page)(config, start, src, length_in_bytes)) }
    }

    pub fn flash_verify_program(
        &self,
        config: *mut FlashConfig,
        start: u32,
        length_in_bytes: u32,
        expected_data: *const u8,
        failed_address: *mut u32,
        failed_data: *mut u32,
    ) -> FlashStatus {
        unsafe {
            FlashStatus::from_raw((self.raw.flash_verify_program)(
                config,
                start,
                length_in_bytes,
                expected_data,
                failed_address,
                failed_data,
            ))
        }
    }

    pub fn flash_verify_erase_phrase(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_verify_erase_phrase)(config, start, length_in_bytes)) }
    }

    pub fn flash_verify_erase_page(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_verify_erase_page)(config, start, length_in_bytes)) }
    }

    pub fn flash_verify_erase_sector(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_verify_erase_sector)(config, start, length_in_bytes)) }
    }

    pub fn flash_get_property(
        &self,
        config: *mut FlashConfig,
        which_property: FlashPropertyTag,
        value: *mut u32,
    ) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_get_property)(config, which_property as u32, value)) }
    }

    pub fn ifr_verify_erase_phrase(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.ifr_verify_erase_phrase)(config, start, length_in_bytes)) }
    }

    pub fn ifr_verify_erase_page(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.ifr_verify_erase_page)(config, start, length_in_bytes)) }
    }

    pub fn ifr_verify_erase_sector(&self, config: *mut FlashConfig, start: u32, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.ifr_verify_erase_sector)(config, start, length_in_bytes)) }
    }

    pub fn flash_read(&self, config: *mut FlashConfig, start: u32, dest: *mut u8, length_in_bytes: u32) -> FlashStatus {
        unsafe { FlashStatus::from_raw((self.raw.flash_read)(config, start, dest, length_in_bytes)) }
    }

    pub fn version(&self) -> StandardVersion {
        self.raw.version
    }
}
