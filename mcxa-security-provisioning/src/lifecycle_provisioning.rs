use core::convert::Infallible;
use core::{mem, ptr};

use defmt_or_log::error;
use ec_slimloader_mcxa::certificate::derive_image_rkth_pair;
pub use ec_slimloader_mcxa::error::FlashStatus;
use ec_slimloader_mcxa::header::ImageHeader;
pub use ec_slimloader_mcxa::lifecycle::{
    cmpa_header_marker_is_valid, cnsa_enforced, fast_boot_enabled, hybrid_secure_boot_enforced, is_cfpa_erased,
    is_cmpa_erased, load_cfpa_header_word, load_lifecycle_from_cfpa, load_pqc_rotkh_from_cmpa, load_rotkh_from_cmpa,
    low_power_authentication_enforced, CmpaUpdateConfigData, CnsaLevel, IFRConfigAreaBase, IFRPage, LpWakePolicy,
    SecureBootLevel, XipImageProtect,
};
pub use ec_slimloader_mcxa::rom_api::{
    flash_cfg_for_rom_api, flash_driver, ActualLifecycleState, Bricked, CanAdvanceTo, Develop, Develop2,
    FailureAnalysis, FlashConfig, InField, InFieldLocked, NbootLifecycleState, NbootRootKeyUsage, OemFieldReturn,
    FLASH_API_ERASE_KEY,
};
use embassy_mcxa::{peripherals, Peri};

/// Token produced by `verify_lifecycle_transition()'. Carries the verified target
/// `NbootLifecycleState` at runtime. The type parameter `Next` is compile-time
/// proof that the transition was constructed via a valid `CanAdvanceTo` impl.
/// The constructor and inner state are intentionally crate-private so no caller
/// can forge a token without going through the full verification path.
pub struct LifecycleAdvanceToken<Next> {
    pub(crate) next: NbootLifecycleState,
    _next: core::marker::PhantomData<Next>,
}

impl<Next> LifecycleAdvanceToken<Next> {
    pub(crate) fn new(next: NbootLifecycleState) -> Self {
        Self {
            next,
            _next: core::marker::PhantomData,
        }
    }
}

// The following CFPA fields are documented here for reference and can be localized when
// readers/writers are added for them:
// const CFPA_PAGE_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0014;
// const CFPA_DBG_REVOKE_VU: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x001C;
// const CFPA_EE0_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0020;
// const CFPA_EE1_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0024;
// const CFPA_EE2_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0028;
// const CFPA_EE3_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x002C;
// const CFPA_RECOVERY_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0034;
// const CFPA_UPDATE_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0038;
// const CFPA_LP_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x003C;
// Currently unused CFPA ITRC error-counter addresses. Keep the documented offsets here until
// a reader/writer is added, rather than exposing unused file-scope constants.
// const CFPA_ERR_ITRC_COUNT_OFFSET: u32 = 0x0054;
// const CFPA_ERR_ITRC_COUNT: u32 = IFRConfigAreaBase::Cfpa as u32 + CFPA_ERR_ITRC_COUNT_OFFSET;
// const CFPA_SCRATCH_ERR_ITRC_COUNT: u32 = IFRScratchAreaBase::Cfpa as u32 + CFPA_ERR_ITRC_COUNT_OFFSET;

/// SCRATCH update type constants (DEV_UPD_TYPE @ offset 0x0), per MCXA reference manual.
/// - UPD_TYPE_CFPA (0x55504446): update CFPA only
/// - UPD_TYPE_CMPA (0x5550444D): update CMPA + CFPA using the populated scratch regions.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IFRWriteGeometry {
    FlashPhraseBytes,
    // IFR sector size per ROM API docs ("erasing specified flash sector (8 KB) in flash or User IFR").
    // Used for flash_erase_sector; may span into NMPA; see erase call sites for details.
    ScratchSectorBytes,
}

impl IFRWriteGeometry {
    #[inline(always)]
    const fn as_usize(self) -> usize {
        match self {
            Self::FlashPhraseBytes => 16,
            // IFR sector = 8 KB. 0x01002000 + 0x2000 - 1 = 0x01003FFF which includes NMPA.
            // flash_erase_sector must accept the full 8 KB sector even though NMPA is at 0x01003800.
            Self::ScratchSectorBytes => 0x2000,
        }
    }

    #[inline(always)]
    const fn as_u32(self) -> u32 {
        self.as_usize() as u32
    }
}

// Future complete CFPA write/update counter list. For now just read-modifysome-write the page.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CfpaWriteField {
    DevcfgUpdType,
    Header,
    PageVersion,
    ImageKeyRevoke,
    DbgRevokeVu,
    Ee0FirmwareVersion,
    Ee1FirmwareVersion,
    Ee2FirmwareVersion,
    Ee3FirmwareVersion,
    FmcSblFirmwareVersion,
    RecoverySb3Version,
    UpdateSb3Version,
    LpFirmwareVersion,
    RotkRevoke,
    ErrAuthFailCount,
    ErrItrcCount,
}

// Offsets here are relative to IFRConfigAreaBase::Cfpa (the start of the CFPA page), not
// relative to the lifecycle header word at 0x10. Do not start counting at the header; each
// field appears shifted down by 0x10, but the memory-mapped reads/writes in this file need
// full page relative offsets.
//TODO: account for all the Monotonic counter words in CFPA so that they are tracked and +1'd, otherwise ROM will silently reject the update. The page version is the only one we currently
// track since it's the only one we read-modify-write; the rest are currently unused and left at 0.
impl CfpaWriteField {
    #[inline(always)]
    const fn byte_offset(self) -> usize {
        match self {
            Self::DevcfgUpdType => 0x0000,
            Self::Header => 0x0010,
            Self::PageVersion => 0x0014,
            Self::ImageKeyRevoke => 0x0018,
            Self::DbgRevokeVu => 0x001C,
            Self::Ee0FirmwareVersion => 0x0020,
            Self::Ee1FirmwareVersion => 0x0024,
            Self::Ee2FirmwareVersion => 0x0028,
            Self::Ee3FirmwareVersion => 0x002C,
            Self::FmcSblFirmwareVersion => 0x0030,
            Self::RecoverySb3Version => 0x0034,
            Self::UpdateSb3Version => 0x0038,
            Self::LpFirmwareVersion => 0x003C,
            Self::RotkRevoke => 0x0040,
            Self::ErrAuthFailCount => 0x0050,
            Self::ErrItrcCount => 0x0054,
        }
    }
}

fn build_cfpa_page_for_cmpa_update() -> Result<[u8; IFRPage::Cfpa.byte_len()], CmpaWriteError> {
    if !is_cfpa_erased() {
        match load_lifecycle_from_cfpa() {
            Some(NbootLifecycleState::Develop) => {}
            Some(_) | None => return Err(CmpaWriteError::LCStateInvalid),
        }
    }

    let mut cfpa_page = match read_cfpa_page_for_update() {
        Ok(page) => page,
        Err(CfpaWriteError::InvalidFlashGeometry) => return Err(CmpaWriteError::InvalidFlashGeometry),
        Err(CfpaWriteError::ConfigError) => return Err(CmpaWriteError::ConfigError),
        Err(CfpaWriteError::CounterOverflow) => return Err(CmpaWriteError::CounterOverflow),
        Err(CfpaWriteError::SecurePolicyViolation) => return Err(CmpaWriteError::ConfigError),
        Err(CfpaWriteError::LifecycleRegression) => return Err(CmpaWriteError::LCStateInvalid),
        Err(CfpaWriteError::LifecycleStateMismatch) => return Err(CmpaWriteError::LCStateInvalid),
        Err(CfpaWriteError::FlashError(status)) => return Err(CmpaWriteError::FlashError(status)),
        Err(CfpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        }) => {
            return Err(CmpaWriteError::FlashVerify {
                status,
                failed_address,
                failed_data,
            })
        }
    };

    unsafe {
        let upd_type_ptr = cfpa_page.as_mut_ptr().add(CfpaWriteField::DevcfgUpdType.byte_offset()) as *mut u32;
        ptr::write_unaligned(upd_type_ptr, ScratchUpdateType::Cmpa as u32);

        let header_ptr = cfpa_page.as_mut_ptr().add(CfpaWriteField::Header.byte_offset()) as *mut u32;
        ptr::write_unaligned(header_ptr, NbootLifecycleState::Develop as u32);
        // We are updating CMPA and CFPA comes along, CMPA can only be updated in Develop LC.

        let page_version_ptr = cfpa_page.as_mut_ptr().add(CfpaWriteField::PageVersion.byte_offset()) as *mut u32;
        let live_page_version = ptr::read_unaligned(page_version_ptr);
        let next_page_version = live_page_version
            .checked_add(1)
            .ok_or(CmpaWriteError::CounterOverflow)?;
        ptr::write_unaligned(page_version_ptr, next_page_version);
    }

    Ok(cfpa_page)
}

#[inline(always)]
fn initialize_cmpa_page_for_first_write(cmpa_page: &mut [u8]) {
    cmpa_page.fill(0);

    const CMPA_HEADER_MARKER: u32 = 0x5963_0000  // Marker in upper half-word
        | (1 << 2)  // EFLASH_BOOTEN = 1 (always enabled)
        | (1 << 0); // IFLASH_BOOTEN = 1 (always enabled; without this the ROM has no boot source on reset)
    let boot_cfg0_ptr = cmpa_page.as_mut_ptr() as *mut u32;
    unsafe {
        ptr::write_unaligned(boot_cfg0_ptr, CMPA_HEADER_MARKER);
    }
}

// SCRATCH bases (use for staging writes)
// NOTE: ROM API flash_erase_sector / flash_program_phrase require secure aliases (0x1100_xxxx)
// for IFR scratch writes. The non-secure alias (0x0100_xxxx) will be rejected.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IFRScratchAreaBase {
    Cfpa = 0x1100_2000,
    Cmpa = 0x1100_2200,
}

/// CMPA.BOOT_CFG0 [15:14] BOOT_SPEED
/// Selects the core frequency and voltage used during ROM execution.
/// If the CMPA header is not valid, the ROM defaults to 0b00 (48 MHz MD mode).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootSpeed {
    Fro192Md = 0b00, // 48 MHz, FRO192, MD mode (default; used when CMPA header invalid)
    Fro192Sd = 0b01, // 96 MHz, FRO192, SD mode
    Spll200 = 0b10,  // 200 MHz, SPLL, OD mode
    Spll250 = 0b11,  // 250 MHz, SPLL, OD mode (actual speed depends on test results)
}

/// Typed write-side representation of CMPA.BOOT_CFG0 config bits [15:0].
/// The header marker (0x5963) in bits [31:16] is preserved from the existing page.
///
/// Bit layout [15:0]:
/// [15:14] Reserved
/// [13:12] BOOT_SPEED
/// [11:10] Reserved
/// [9:8]   REC_BOOT_EN    (0b00=disabled, 0b11=enabled)
/// [7:6]   Reserved
/// [5]     REC_LSPI       (recovery via LSPI)
/// [4]     REC_FLEXSPI    (recovery via FlexSPI)
/// [3]     EFLASH_DUAL_EN = 0 (hardcoded: single-channel)
/// [2]     EFLASH_BOOTEN  = 1 (hardcoded: external FlexSPI boot always enabled)
/// [1]     IFLASH_DUAL_EN = 0 (hardcoded: single-channel)
/// [0]     IFLASH_BOOTEN  = 1 (hardcoded: internal flash boot always enabled)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmpaBootCfg0Write {
    pub boot_speed: BootSpeed,
    pub rec_boot_en: Option<bool>, // maps to 0b11 (enabled) or 0b00 (disabled)
    pub rec_lspi: Option<bool>,    // None = disabled (false)
    pub rec_flexspi: Option<bool>, // None = disabled (false)
}

impl CmpaBootCfg0Write {
    /// Returns only config bits [15:0]. Caller must OR with the header marker in [31:16].
    pub fn to_config_bits(self) -> u32 {
        ((self.boot_speed as u32) << 12)                                       // [13:12]
            | (if self.rec_boot_en.unwrap_or(false) { 0b11 << 8 } else { 0 }) // [9:8] 0b11=enabled
            | ((self.rec_lspi.unwrap_or(false)    as u32) << 5)               // [5]
            | ((self.rec_flexspi.unwrap_or(false) as u32) << 4)               // [4]
            // [3] EFLASH_DUAL_EN = 0 (always single-channel)
            | (1 << 2) // [2] EFLASH_BOOTEN = 1 (always enable external FlexSPI boot)
            // [1] IFLASH_DUAL_EN = 0 (always single-channel)
            | (1 << 0) // [0] IFLASH_BOOTEN = 1 (always enable internal flash boot)
    }
}

impl Default for CmpaBootCfg0Write {
    fn default() -> Self {
        Self {
            boot_speed: BootSpeed::Fro192Md,
            rec_boot_en: None,
            rec_lspi: None,
            rec_flexspi: None,
        }
    }
}

/// CMPA.BOOT_CFG1 [15:14]/[13:12]/[11:10]/[9:8] ISP entry policy (2-bit fields).
/// 0b01 = entry disabled; 0b00 / 0b10 / 0b11 = entry allowed. Use 0b00 for allowed.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IspEntryPolicy {
    Allowed = 0b00,  // ISP entry allowed (0b00, 0b10, 0b11 all permit entry; use 0b00)
    Disabled = 0b01, // ISP entry disabled (only 0b01 disables)
}

/// Typed write-side representation of CMPA.BOOT_CFG1 config bits.
/// The EXT_CMPA_32B_SIZE field in bits [31:24] is fixed at 160 (5 KB IFR / 32-byte pages).
///
/// Bit layout:
/// [31:24] EXT_CMPA_32B_SIZE = 160 (hardcoded); MAX CMPA customer defined area size in 32b chunks, 5k total (5120), divided by 32 is 160.
/// [23:21] Reserved
/// [20:16] FLASH_REMAP_SIZE  = 0 (hardcoded; feature unused)
/// [15:14] ISP_FT_ENTRY      (0b01=disabled, 0b00/0b10/0b11=allowed); allow ISP entry after Image auth. fail.
/// [13:12] ISP_API_ENTRY     (0b01=disabled, 0b00/0b10/0b11=allowed); allow ROM API ISP entry
/// [11:10] ISP_DM_ENTRY      (0b01=disabled, 0b00/0b10/0b11=allowed); allow DEBUG MODE ISP entry (authenticated debug);
/// [9:8]   ISP_PIN_ENTRY     (0b01=disabled, 0b00/0b10/0b11=allowed); allow PIN-based ISP entry
/// [7:5]   Reserved
/// [4]     ISP_USB_EN        (0=disabled, 1=enabled)
/// [3]     ISP_I2C_EN        (0=disabled, 1=enabled)
/// [2]     ISP_CAN_EN        (0=disabled, 1=enabled)
/// [1]     ISP_SPI_EN        (0=disabled, 1=enabled)
/// [0]     ISP_UART_EN       (0=disabled, 1=enabled; default 1)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmpaBootCfg1Write {
    pub isp_ft_entry: IspEntryPolicy,
    // ISP entry via ROM API is allowed via default.
    pub isp_dm_entry: IspEntryPolicy,
    pub isp_pin_entry: IspEntryPolicy,
    pub isp_usb_en: Option<bool>,  // None = disabled
    pub isp_i2c_en: Option<bool>,  // None = disabled
    pub isp_can_en: Option<bool>,  // None = disabled
    pub isp_spi_en: Option<bool>,  // None = disabled
    pub isp_uart_en: Option<bool>, // None = enabled (default on)
}

impl CmpaBootCfg1Write {
    pub fn to_config_bits(self) -> u32 {
        const MAX_32B_USER_DEFINED_SIZE: u32 = 160; // Max size of user-defined CMPA area when using 32-byte pages, per ROM/API documentation.
                                                    //  This is the size we use for the entire CMPA since we want to allow full customer usage.
        (MAX_32B_USER_DEFINED_SIZE << 24)                                        // [31:24] EXT_CMPA_32B_SIZE = 160 (hardcoded)
            // [20:16] FLASH_REMAP_SIZE = 0 (hardcoded; feature unused)
            | ((self.isp_ft_entry  as u32) << 14)              // [15:14]
            | ((IspEntryPolicy::Allowed as u32) << 12)         // [13:12] ISP_API_ENTRY = Allowed (default enabled)
            | ((self.isp_dm_entry  as u32) << 10)              // [11:10]
            | ((self.isp_pin_entry as u32) <<  8)              // [9:8]
            | ((self.isp_usb_en.unwrap_or(false) as u32) << 4) // [4]
            | ((self.isp_i2c_en.unwrap_or(false) as u32) << 3) // [3]
            | ((self.isp_can_en.unwrap_or(false) as u32) << 2) // [2]
            | ((self.isp_spi_en.unwrap_or(false) as u32) << 1) // [1]
            | (self.isp_uart_en.unwrap_or(true)  as u32) // [0] default enabled
    }
}

impl Default for CmpaBootCfg1Write {
    fn default() -> Self {
        Self {
            isp_ft_entry: IspEntryPolicy::Allowed,
            isp_dm_entry: IspEntryPolicy::Allowed,
            isp_pin_entry: IspEntryPolicy::Allowed,
            isp_usb_en: None,
            isp_i2c_en: None,
            isp_can_en: None,
            isp_spi_en: None,
            isp_uart_en: None,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiPort {
    PortA1 = 0b00,   // 4 bit
    PortB1 = 0b01,   // 4 bit
    PortA1B1 = 0b10, // 8 bit
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiResetEnable {
    Disabled = 0b0,
    Enabled = 0b1,
}

/// Q/O-SPI flash interface frequency (FLEXSPI_FREQ, 3-bit field).
/// Used when FLEXSPI_AUTO_PROBE_EN is set.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiFrequency {
    Freq75MHz = 0b000, // 75 MHz
    Freq60MHz = 0b001, // 60 MHz
    Freq50MHz = 0b010, // 50 MHz
    Freq100MHz = 0b011, // 100 MHz
                       // 0b100–0b111: Reserved
}

/// Delay after POR before accessing Quad/Octal-SPI flash (FLEXSPI_PWR_HOLD_TIME, [28:25], 4-bit).
/// Added on top of the delay defined by FLEXSPI_HOLD_TIME.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiPwrHoldTime {
    NoDelay = 0b0000,    // No additional delay
    Delay100us = 0b0001, // 100 microseconds
    Delay500us = 0b0010, // 500 microseconds
    Delay1ms = 0b0011,   // 1 millisecond
    Delay10ms = 0b0100,  // 10 milliseconds
    Delay20ms = 0b0101,  // 20 milliseconds
    Delay40ms = 0b0110,  // 40 milliseconds
    Delay60ms = 0b0111,  // 60 milliseconds
    Delay80ms = 0b1000,  // 80 milliseconds
    Delay100ms = 0b1001, // 100 milliseconds
    Delay120ms = 0b1010, // 120 milliseconds
    Delay140ms = 0b1011, // 140 milliseconds
    Delay160ms = 0b1100, // 160 milliseconds
    Delay180ms = 0b1101, // 180 milliseconds
    Delay200ms = 0b1110, // 200 milliseconds
    Delay220ms = 0b1111, // 220 milliseconds
}

/// Delay after reset before accessing Quad/Octal-SPI flash (FLEXSPI_HOLD_TIME, [24:23], 2-bit).
/// For POR, FLEXSPI_PWR_HOLD_TIME is added on top of this.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiHoldTime {
    Delay500us = 0b00, // 500 microseconds
    Delay1ms = 0b01,   // 1 millisecond
    Delay3ms = 0b10,   // 3 milliseconds
    Delay10ms = 0b11,  // 10 milliseconds
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QSpiAutoProbe {
    Disabled = 0b0,
    Enabled = 0b1,
}

/// Typed write-side representation of CMPA.LSPI_QFLASH_CFG0.
///
/// Bit layout:
/// [31:30] QSPI_PORT          (2-bit: PortA1=0b00, PortB1=0b01, PortA1B1=0b10)
/// [29]    Reserved
/// [28:25] QSPI_PWR_HOLD_TIME (4-bit raw value)
/// [24:23] QSPI_HOLD_TIME     (2-bit raw value)
/// [22:18] QSPI_RESET_GPIO_PIN  (5-bit GPIO pin number)
/// [17:15] QSPI_RESET_GPIO_PORT (3-bit GPIO port number)
/// [14]    QSPI_RESET_ENABLE
/// [13:11] QSPI_FREQUENCY     (3-bit: see QSpiFrequency)
/// [10:7]  QSPI_DUMMY_CYCLES  (4-bit raw value)
/// [6:1]   Reserved
/// [0]     QSPI_AUTO_PROBE_EN
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmpaLspiCfg0Write {
    pub qspi_port: QSpiPort,
    pub qspi_pwr_hold_time: QSpiPwrHoldTime, // [28:25] 4-bit
    pub qspi_hold_time: QSpiHoldTime,        // [24:23] 2-bit
    pub qspi_reset_gpio_pin: u8,             // [22:18] 5-bit
    pub qspi_reset_gpio_port: u8,            // [17:15] 3-bit
    pub qspi_reset_enable: QSpiResetEnable,
    pub qspi_frequency: QSpiFrequency,
    pub qspi_dummy_cycles: u8, // [10:7]  4-bit
    pub qspi_auto_probe: QSpiAutoProbe,
}

impl CmpaLspiCfg0Write {
    pub fn to_config_bits(self) -> u32 {
        ((self.qspi_port as u32)                       << 30) // [31:30] QSPI_PORT
        // [29] Reserved
        | ((self.qspi_pwr_hold_time as u32)            << 25) // [28:25] QSPI_PWR_HOLD_TIME
        | ((self.qspi_hold_time as u32)                << 23) // [24:23] QSPI_HOLD_TIME
        | (((self.qspi_reset_gpio_pin as u32) & 0x1F)  << 18) // [22:18] QSPI_RESET_GPIO_PIN
        | (((self.qspi_reset_gpio_port as u32) & 0x7)  << 15) // [17:15] QSPI_RESET_GPIO_PORT
        | ((self.qspi_reset_enable as u32)             << 14) // [14]    QSPI_RESET_ENABLE
        | ((self.qspi_frequency as u32)                << 11) // [13:11] QSPI_FREQUENCY
        | (((self.qspi_dummy_cycles as u32) & 0xF)     <<  7) // [10:7]  QSPI_DUMMY_CYCLES
        // [6:1] Reserved
        | (self.qspi_auto_probe as u32) // [0]     QSPI_AUTO_PROBE_EN
    }
}

impl Default for CmpaLspiCfg0Write {
    fn default() -> Self {
        Self {
            #[cfg(not(feature = "mcxa5xxevk"))]
            qspi_port: QSpiPort::PortB1,
            #[cfg(feature = "mcxa5xxevk")]
            qspi_port: QSpiPort::PortA1,
            qspi_pwr_hold_time: QSpiPwrHoldTime::NoDelay,
            qspi_hold_time: QSpiHoldTime::Delay500us,
            qspi_reset_gpio_pin: 0, //TODO, check schematics for correct GPIO pin number for QSPI reset.
            qspi_reset_gpio_port: 0, //TODO
            qspi_reset_enable: QSpiResetEnable::Disabled, //TODO
            qspi_frequency: QSpiFrequency::Freq100MHz,
            qspi_dummy_cycles: 0,
            qspi_auto_probe: QSpiAutoProbe::Enabled,
        }
    }
}

/// CMPA.SECURE_BOOT_CFG [11:10] ENF_TZM_PRESET
/// Controls whether the ROM enforces the TrustZone-M preset from the image manifest.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TzmPreset {
    Ignored = 0b00, // TZM preset in image manifest is ignored
    Enforce = 0b11, // Enforce preset TZM data in image manifest (0b01, 0b10, 0b11 all enforce)
}

/// CMPA.SECURE_BOOT_CFG [7:6] DICE_CSR_KEY_TYPE
/// Defines the DICE CSR and key generation algorithm.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiceCsrKeyType {
    EccP384 = 0b00,           // 0b00 or 0b01: Generate DICE ECC P-384 keys
    EccP384AndMlDsa87 = 0b11, // 0b10 or 0b11: Generate DICE SHA-384 & ML-DSA keys
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScratchUpdateType {
    Cfpa = 0x5550_4446,
    Cmpa = 0x5550_444D,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpaWriteError {
    LCStateInvalid,
    InvalidFlashGeometry,
    ConfigError,
    CounterOverflow,
    RotkhMismatch,
    HashError,
    InvalidInput,
    InvalidImageSlot,
    FlashError(FlashStatus),
    FlashVerify {
        status: FlashStatus,
        failed_address: u32,
        failed_data: u32,
    },
}

pub fn log_cmpa_write_error(e: CmpaWriteError) {
    match e {
        CmpaWriteError::LCStateInvalid => error!("CmpaWriteError: LCStateInvalid"),
        CmpaWriteError::InvalidFlashGeometry => error!("CmpaWriteError: InvalidFlashGeometry"),
        CmpaWriteError::ConfigError => error!("CmpaWriteError: ConfigError"),
        CmpaWriteError::CounterOverflow => error!("CmpaWriteError: CounterOverflow"),
        CmpaWriteError::RotkhMismatch => error!("CmpaWriteError: RotkhMismatch"),
        CmpaWriteError::HashError => error!("CmpaWriteError: HashError"),
        CmpaWriteError::InvalidInput => error!("CmpaWriteError: InvalidInput"),
        CmpaWriteError::InvalidImageSlot => error!("CmpaWriteError: InvalidImageSlot"),
        CmpaWriteError::FlashError(s) => error!("CmpaWriteError: FlashError({:?})", s),
        CmpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        } => {
            error!(
                "CmpaWriteError: FlashVerify status={:?} addr={:x} data={:x}",
                status, failed_address, failed_data
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CfpaWriteError {
    InvalidFlashGeometry,
    ConfigError,
    CounterOverflow,
    /// CMPA secure boot policy is not fully configured; lifecycle advance refused.
    SecurePolicyViolation,
    /// Requested lifecycle state is not a forward progression from current state.
    LifecycleRegression,
    /// The device's actual lifecycle state in CFPA does not match the expected `From` state.
    LifecycleStateMismatch,
    FlashError(FlashStatus),
    FlashVerify {
        status: FlashStatus,
        failed_address: u32,
        failed_data: u32,
    },
}

fn read_cfpa_page_for_update() -> Result<[u8; IFRPage::Cfpa.byte_len()], CfpaWriteError> {
    if !IFRPage::Cfpa
        .byte_len()
        .is_multiple_of(IFRWriteGeometry::FlashPhraseBytes.as_usize())
    {
        return Err(CfpaWriteError::InvalidFlashGeometry);
    }

    if is_cfpa_erased() {
        // First-time provisioning: skip the flash read (would give back 0xFF).
        // Initialize a valid Develop-lifecycle header.
        let mut page = [0u8; IFRPage::Cfpa.byte_len()];
        unsafe {
            let header_ptr = page.as_mut_ptr().add(CfpaWriteField::Header.byte_offset()) as *mut u32;
            ptr::write_unaligned(header_ptr, NbootLifecycleState::Develop as u32);

            // is_cfpa_erased() skips ERR_AUTH_FAIL_COUNT and ERR_ITRC_COUNT so the ROM can
            // update them autonomously. Preserve whatever the ROM has written so we don't
            // reset them to zero when staging this fresh page.
            for field in [CfpaWriteField::ErrAuthFailCount, CfpaWriteField::ErrItrcCount] {
                let addr = IFRConfigAreaBase::Cfpa as u32 + field.byte_offset() as u32;
                let live_val = core::ptr::read_volatile(addr as *const u32);
                let dst = page.as_mut_ptr().add(field.byte_offset()) as *mut u32;
                ptr::write_unaligned(dst, live_val);
            }
        }
        return Ok(page);
    } else if load_cfpa_header_word().is_none() {
        // Non-erased but invalid header: corrupted or unknown state, refuse to write.
        return Err(CfpaWriteError::ConfigError);
    }

    let mut page = [0u8; IFRPage::Cfpa.byte_len()];
    // Use read_volatile instead of flash_read, the ROM API flash_read rejects CFG area
    // addresses via an internal whitelist. read_volatile works fine since the IFR CFG area
    // is memory-mapped and readable directly.
    for i in 0..(IFRPage::Cfpa.byte_len() / 4) {
        let addr = IFRConfigAreaBase::Cfpa as u32 + (i as u32 * 4);
        let word = unsafe { core::ptr::read_volatile(addr as *const u32) };
        page[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }

    Ok(page)
}

fn write_cfpa_page_to_scratch(page: &[u8; IFRPage::Cfpa.byte_len()]) -> Result<(), CfpaWriteError> {
    let drv = flash_driver();
    let mut cfg = flash_cfg_for_rom_api();

    let s = drv.flash_init(&mut cfg);
    if s != FlashStatus::Success {
        error!("cfpa: flash_init failed");
        return Err(CfpaWriteError::FlashError(s));
    }

    // flash_erase_sector supports "flash or User IFR(IFR0)" per ROM API docs.
    // Start only needs to be phrase-aligned (16-byte); 0x11002000 qualifies.
    // 0x11002000 + 0x2000 - 1 = 0x110037FF, which is the last byte before NMPA at 0x11003800.
    let s = drv.flash_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
        FLASH_API_ERASE_KEY,
    );
    if s != FlashStatus::Success {
        error!(
            "cfpa: flash_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CfpaWriteError::FlashError(s));
    }
    let s = drv.ifr_verify_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
    );
    if s != FlashStatus::Success {
        error!(
            "cfpa: ifr_verify_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CfpaWriteError::FlashError(s));
    }

    let s = drv.flash_program_phrase(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        page.as_ptr(),
        page.len() as u32,
    );
    if s != FlashStatus::Success {
        error!(
            "cfpa: flash_program_phrase(addr=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            page.len() as u32
        );
        return Err(CfpaWriteError::FlashError(s));
    }

    let mut failed_address = 0u32;
    let mut failed_data = 0u32;
    let s = drv.flash_verify_program(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        page.len() as u32,
        page.as_ptr(),
        &mut failed_address,
        &mut failed_data,
    );
    if s != FlashStatus::Success {
        error!(
            "cfpa: flash_verify_program failed @ 0x{:08x} (data=0x{:08x})",
            failed_address, failed_data
        );
        return Err(CfpaWriteError::FlashVerify {
            status: s,
            failed_address,
            failed_data,
        });
    }

    Ok(())
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotkState {
    // The ROM accepts 0b00 and 0b01 as enabled. Use 0b00 as the canonical encoding.
    Enabled = 0b00,
    // The ROM accepts 0b10 and 0b11 as revoked. Use 0b10 as the canonical encoding.
    Revoked = 0b10,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiceAliasGeneration {
    NotGenerated = 0,
    Generated = 1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotkRevokeConfig {
    pub rotk0: Option<RotkState>,
    pub rotk1: Option<RotkState>,
    pub rotk2: Option<RotkState>,
    pub rotk3: Option<RotkState>,
    pub upd_alias_key: Option<DiceAliasGeneration>,
    pub upd_alias_cert: Option<DiceAliasGeneration>,
}

impl RotkRevokeConfig {
    fn apply_to_word(self, current_word: u32) -> u32 {
        let mut next = current_word;

        if let Some(value) = self.rotk0 {
            next = (next & !0b11) | (value as u32);
        }
        if let Some(value) = self.rotk1 {
            next = (next & !(0b11 << 2)) | ((value as u32) << 2);
        }
        if let Some(value) = self.rotk2 {
            next = (next & !(0b11 << 4)) | ((value as u32) << 4);
        }
        if let Some(value) = self.rotk3 {
            next = (next & !(0b11 << 6)) | ((value as u32) << 6);
        }
        if let Some(value) = self.upd_alias_key {
            next = (next & !(1 << 28)) | ((value as u32) << 28);
        }
        if let Some(value) = self.upd_alias_cert {
            next = (next & !(1 << 29)) | ((value as u32) << 29);
        }

        next
    }
}

fn bump_cfpa_monotonic_ctr_in_scratch(field: CfpaWriteField) -> Result<(), CfpaWriteError> {
    let mut page = read_cfpa_page_for_update()?;

    unsafe {
        let upd_type_ptr = page.as_mut_ptr().add(CfpaWriteField::DevcfgUpdType.byte_offset()) as *mut u32;
        ptr::write_unaligned(upd_type_ptr, ScratchUpdateType::Cfpa as u32);

        let ptr = page.as_mut_ptr().add(field.byte_offset()) as *mut u32;
        let val = ptr::read_unaligned(ptr);
        let next = match field {
            CfpaWriteField::PageVersion
            | CfpaWriteField::Ee0FirmwareVersion
            | CfpaWriteField::Ee1FirmwareVersion
            | CfpaWriteField::Ee2FirmwareVersion
            | CfpaWriteField::Ee3FirmwareVersion
            | CfpaWriteField::FmcSblFirmwareVersion
            | CfpaWriteField::RecoverySb3Version
            | CfpaWriteField::UpdateSb3Version
            | CfpaWriteField::LpFirmwareVersion
            | CfpaWriteField::ErrAuthFailCount
            | CfpaWriteField::ErrItrcCount => val.checked_add(1).ok_or(CfpaWriteError::CounterOverflow)?,
            _ => return Err(CfpaWriteError::ConfigError),
        };
        ptr::write_unaligned(ptr, next);
    }

    write_cfpa_page_to_scratch(&page)
}

fn update_rotk_revoke_in_scratch(config: RotkRevokeConfig) -> Result<(), CfpaWriteError> {
    let mut page = read_cfpa_page_for_update()?;

    unsafe {
        let upd_type_ptr = page.as_mut_ptr().add(CfpaWriteField::DevcfgUpdType.byte_offset()) as *mut u32;
        ptr::write_unaligned(upd_type_ptr, ScratchUpdateType::Cfpa as u32);

        let ptr = page.as_mut_ptr().add(CfpaWriteField::RotkRevoke.byte_offset()) as *mut u32;
        let current = ptr::read_unaligned(ptr);
        let next = config.apply_to_word(current);
        ptr::write_unaligned(ptr, next);
    }

    write_cfpa_page_to_scratch(&page)
}

fn stage_cfpa_lifecycle_advance_to_scratch(next_lc_state: NbootLifecycleState) -> Result<(), CfpaWriteError> {
    let mut page = read_cfpa_page_for_update()?;

    unsafe {
        let upd_type_ptr = page.as_mut_ptr().add(CfpaWriteField::DevcfgUpdType.byte_offset()) as *mut u32;
        ptr::write_unaligned(upd_type_ptr, ScratchUpdateType::Cfpa as u32);

        let header_ptr = page.as_mut_ptr().add(CfpaWriteField::Header.byte_offset()) as *mut u32;
        ptr::write_unaligned(header_ptr, next_lc_state as u32);
    }

    write_cfpa_page_to_scratch(&page)
}

pub fn cfpa_bump_auth_fail_count_and_reset() -> Result<Infallible, CfpaWriteError> {
    bump_cfpa_monotonic_ctr_in_scratch(CfpaWriteField::ErrAuthFailCount)?;
    cortex_m::peripheral::SCB::sys_reset()
}

pub fn cfpa_bump_firmware_version_and_reset() -> Result<Infallible, CfpaWriteError> {
    bump_cfpa_monotonic_ctr_in_scratch(CfpaWriteField::Ee0FirmwareVersion)?;
    cortex_m::peripheral::SCB::sys_reset()
}

pub fn update_rotk_revoke_in_scratch_and_reset(config: RotkRevokeConfig) -> Result<Infallible, CfpaWriteError> {
    update_rotk_revoke_in_scratch(config)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// Build a compile-time-checked advance token.
///
/// The `From` and `Next` type parameters must satisfy `CanAdvanceTo<Next>` — any
/// pair not listed in the `impl` table is a *compile error*. The runtime checks
/// (secure policy + rank) are still performed here, but the *valid combinations*
/// are enforced by the type system.
///
/// Example (valid at compile time, verified at runtime):
/// ```ignore
/// let token = verify_lifecycle_transition::<Develop, Develop2>()?;
/// cfpa_stage_lifecycle_advance_and_reset(token)?;
/// ```
/// This would NOT compile:
/// ```ignore
/// let token = verify_lifecycle_transition::<Develop, Bricked>(); // ← compile error
/// ```
/// WARNING: When bricking, the provisioner should run from SRAM, since flash will be erased (mid operation?)
/// WARNING: When MBC is enabled for INT_FLASH sectors, bricking will fail without unlocking the sectors first.
/// The ROM API flash_erase_sector does not support unlocking sectors, so the caller must ensure the flash is unlocked before calling this function.
/// Transitioning to Bricked LC is currently not in the pipeline.
pub fn verify_lifecycle_transition<From, Next>(
    next: NbootLifecycleState,
) -> Result<LifecycleAdvanceToken<Next>, CfpaWriteError>
where
    From: CanAdvanceTo<Next> + ActualLifecycleState,
{
    if let Some(current) = load_lifecycle_from_cfpa() {
        if current != From::STATE {
            return Err(CfpaWriteError::LifecycleStateMismatch);
        }
        if !current.can_advance_to(next) {
            return Err(CfpaWriteError::LifecycleRegression);
        }
    }
    if next == NbootLifecycleState::Bricked {
        // Bricking the device is a special case that doesn't require CMPA policy to be valid, since the device will be unusable after this action regardless.
        //erase all internal (todo: external) flash except the sticky locked SBL region.
        let drv = flash_driver();
        let mut cfg = flash_cfg_for_rom_api();
        let s = drv.flash_init(&mut cfg);

        if s != FlashStatus::Success {
            return Err(CfpaWriteError::FlashError(s));
        }

        const APP_START: u32 = 0x10000; // BL ends just before 0x10000, so this is the first address of the app region.
        const ERASE_SIZE: u32 = 0x0020_0000 - APP_START; // Erase from 0x10000 to 0x200000 (2MB flash size) to cover the app region.
        let s = drv.flash_erase_sector(&mut cfg, APP_START, ERASE_SIZE, FLASH_API_ERASE_KEY);

        if s != FlashStatus::Success {
            return Err(CfpaWriteError::FlashError(s));
        }

        return Ok(LifecycleAdvanceToken::new(next));
    }
    // CMPA must be provisioned and valid before any policy field can be trusted, unless bricking the device.
    if is_cmpa_erased() || !cmpa_header_marker_is_valid() {
        return Err(CfpaWriteError::SecurePolicyViolation);
    }
    if !hybrid_secure_boot_enforced() || !cnsa_enforced() || fast_boot_enabled() || !low_power_authentication_enforced()
    {
        return Err(CfpaWriteError::SecurePolicyViolation);
    }
    Ok(LifecycleAdvanceToken::new(next))
}

/// Commit a lifecycle transition that was verified via `verify_lifecycle_transition()`.
/// The token can only be constructed through that function, ensuring both compile-time
/// and runtime checks have passed before this point.
pub fn cfpa_stage_lifecycle_advance_and_reset<Next>(
    token: LifecycleAdvanceToken<Next>,
) -> Result<Infallible, CfpaWriteError> {
    stage_cfpa_lifecycle_advance_to_scratch(token.next)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// Typed write-side representation of CMPA.RoTK_USAGE.
///
/// Bit layout:
/// [2:0]   RoTK0_Usage      [5:3]  RoTK1_Usage
/// [8:6]   RoTK2_Usage      [11:9] RoTK3_Usage
/// [12]    SKIP_DICE
/// [13]    DICE_INC_NXP_CFG
/// [14]    DICE_INC_CUST_CFG
/// [15]    DICE_INC_NXP_FIELD_CFG
/// [31:16] Reserved
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmpaRotkUsage {
    pub rotk: [NbootRootKeyUsage; 4],
    pub skip_dice: bool,
    pub dice_inc_nxp_cfg: bool,
    pub dice_inc_cust_cfg: bool,
    pub dice_inc_nxp_field_cfg: bool,
}

impl CmpaRotkUsage {
    pub fn to_u32(self) -> u32 {
        (self.rotk[0] as u32)
            | ((self.rotk[1] as u32) << 3)
            | ((self.rotk[2] as u32) << 6)
            | ((self.rotk[3] as u32) << 9)
            | ((self.skip_dice as u32) << 12)
            | ((self.dice_inc_nxp_cfg as u32) << 13)
            | ((self.dice_inc_cust_cfg as u32) << 14)
            | ((self.dice_inc_nxp_field_cfg as u32) << 15)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmpaDefaultConfig {
    pub boot_cfg0: CmpaBootCfg0Write,
    pub boot_cfg1: CmpaBootCfg1Write,
    pub rotk_usage: CmpaRotkUsage,
    pub dice_csr_key_type: DiceCsrKeyType,
    pub enf_tzm_preset: TzmPreset,
    pub sbl_start_addr: Option<u32>,
    pub rotkh: [u8; 48],
    pub pqc_rotkh: [u8; 48],
}


// Reads the current CMPA page from CFG, validates state, and initializes the header for a
// first write if the page is erased. Returns the page buffer ready for field patching.
pub fn read_cmpa_page_for_update() -> Result<[u8; IFRPage::CmpaAll.byte_len()], CmpaWriteError> {
    let cmpa_is_erased = is_cmpa_erased();

    if cmpa_is_erased {
        // First-time provisioning: skip the flash read (would just give back 0xFF).
        // Start from a clean zeroed buffer with the header marker set.
        let mut cmpa_page = [0u8; IFRPage::CmpaAll.byte_len()];
        initialize_cmpa_page_for_first_write(&mut cmpa_page);
        return Ok(cmpa_page);
    } else if cmpa_header_marker_is_valid() {
        // Existing provisioned CMPA: only allow updates in Develop lifecycle.
        match load_lifecycle_from_cfpa() {
            Some(NbootLifecycleState::Develop) => {}
            Some(_) | None => return Err(CmpaWriteError::LCStateInvalid),
        }
    } else {
        // Non-erased but invalid header: corrupted or unknown state, refuse to write.
        return Err(CmpaWriteError::ConfigError);
    }

    let mut cmpa_page = [0u8; IFRPage::CmpaAll.byte_len()];
    // Use read_volatile instead of flash_read, the ROM API rejects CFG area addresses.
    for i in 0..(IFRPage::CmpaAll.byte_len() / 4) {
        let addr = IFRConfigAreaBase::Cmpa as u32 + (i as u32 * 4);
        let word = unsafe { core::ptr::read_volatile(addr as *const u32) };
        cmpa_page[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }

    Ok(cmpa_page)
}

// Stages the provided CMPA page image into SCRATCH. Reads the current CFPA page, sets
// UPD_TYPE to CMPA, erases scratch, programs both CFPA and CMPA scratch pages, then verifies.
pub fn write_cmpa_page_to_scratch(cmpa_page: &[u8; IFRPage::CmpaAll.byte_len()]) -> Result<(), CmpaWriteError> {
    if !IFRPage::Cfpa
        .byte_len()
        .is_multiple_of(IFRWriteGeometry::FlashPhraseBytes.as_usize())
        || !IFRPage::CmpaAll
            .byte_len()
            .is_multiple_of(IFRWriteGeometry::FlashPhraseBytes.as_usize())
    {
        return Err(CmpaWriteError::InvalidFlashGeometry);
    }

    let cfpa_page = build_cfpa_page_for_cmpa_update()?;

    let drv = flash_driver();
    let mut cfg = flash_cfg_for_rom_api();

    let status = drv.flash_init(&mut cfg);
    if status != FlashStatus::Success {
        error!("cmpa: flash_init failed");
        return Err(CmpaWriteError::FlashError(status));
    }

    // Erase the full 8 KB IFR scratch sector, then verify it is blank.
    // Start only needs to be phrase-aligned (16-byte); 0x11002000 qualifies.
    // 0x11002000 + 0x2000 - 1 = 0x110037FF, which is the last byte before NMPA at 0x11003800.
    let status = drv.flash_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
        FLASH_API_ERASE_KEY,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CmpaWriteError::FlashError(status));
    }
    let status = drv.ifr_verify_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: ifr_verify_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let status = drv.flash_program_phrase(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        cfpa_page.as_ptr(),
        cfpa_page.len() as u32,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_program_phrase(cfpa_scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            cfpa_page.len() as u32
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let status = drv.flash_program_phrase(
        &mut cfg,
        IFRScratchAreaBase::Cmpa as u32,
        cmpa_page.as_ptr(),
        cmpa_page.len() as u32,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_program_phrase(cmpa_scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cmpa as u32,
            cmpa_page.len() as u32
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let mut failed_address = 0u32;
    let mut failed_data = 0u32;
    let status = drv.flash_verify_program(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        cfpa_page.len() as u32,
        cfpa_page.as_ptr(),
        &mut failed_address,
        &mut failed_data,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_verify_program(cfpa_scratch) failed @ 0x{:08x} (data=0x{:08x})",
            failed_address, failed_data
        );
        return Err(CmpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        });
    }

    let status = drv.flash_verify_program(
        &mut cfg,
        IFRScratchAreaBase::Cmpa as u32,
        cmpa_page.len() as u32,
        cmpa_page.as_ptr(),
        &mut failed_address,
        &mut failed_data,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_verify_program(cmpa_scratch) failed @ 0x{:08x} (data=0x{:08x})",
            failed_address, failed_data
        );
        return Err(CmpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        });
    }

    Ok(())
}

// Reads only the 512-byte core CMPA page (IFRPage::Cmpa, no customer-defined area).
// Use this when EXT_CMPA_32B_SIZE is 0 (the default), so the ROM will copy the
// correct amount of data from scratch on reset.
pub fn read_cmpa_core_page_for_update() -> Result<[u8; IFRPage::Cmpa.byte_len()], CmpaWriteError> {
    let cmpa_is_erased = is_cmpa_erased();

    if cmpa_is_erased {
        let mut cmpa_page = [0u8; IFRPage::Cmpa.byte_len()];
        initialize_cmpa_page_for_first_write(&mut cmpa_page);
        return Ok(cmpa_page);
    } else if cmpa_header_marker_is_valid() {
        match load_lifecycle_from_cfpa() {
            Some(NbootLifecycleState::Develop) => {}
            Some(_) | None => return Err(CmpaWriteError::LCStateInvalid),
        }
    } else {
        return Err(CmpaWriteError::ConfigError);
    }

    let mut cmpa_page = [0u8; IFRPage::Cmpa.byte_len()];
    for i in 0..(IFRPage::Cmpa.byte_len() / 4) {
        let addr = IFRConfigAreaBase::Cmpa as u32 + (i as u32 * 4);
        let word = unsafe { core::ptr::read_volatile(addr as *const u32) };
        cmpa_page[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }

    Ok(cmpa_page)
}

/// Stages only the 512-byte core CMPA page into SCRATCH.
/// Matches read_cmpa_core_page_for_update; use when EXT_CMPA_32B_SIZE is 0.
pub fn write_cmpa_core_page_to_scratch(cmpa_page: &[u8; IFRPage::Cmpa.byte_len()]) -> Result<(), CmpaWriteError> {
    if !IFRPage::Cfpa
        .byte_len()
        .is_multiple_of(IFRWriteGeometry::FlashPhraseBytes.as_usize())
        || !IFRPage::Cmpa
            .byte_len()
            .is_multiple_of(IFRWriteGeometry::FlashPhraseBytes.as_usize())
    {
        return Err(CmpaWriteError::InvalidFlashGeometry);
    }

    let cfpa_page = build_cfpa_page_for_cmpa_update()?;

    let drv = flash_driver();
    let mut cfg = flash_cfg_for_rom_api();

    let status = drv.flash_init(&mut cfg);
    if status != FlashStatus::Success {
        error!("cmpa: flash_init failed");
        return Err(CmpaWriteError::FlashError(status));
    }

    let status = drv.flash_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
        FLASH_API_ERASE_KEY,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CmpaWriteError::FlashError(status));
    }
    let status = drv.ifr_verify_erase_sector(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        IFRWriteGeometry::ScratchSectorBytes.as_u32(),
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: ifr_verify_erase_sector(scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            IFRWriteGeometry::ScratchSectorBytes.as_u32()
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let status = drv.flash_program_phrase(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        cfpa_page.as_ptr(),
        cfpa_page.len() as u32,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_program_phrase(cfpa_scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cfpa as u32,
            cfpa_page.len() as u32
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let status = drv.flash_program_phrase(
        &mut cfg,
        IFRScratchAreaBase::Cmpa as u32,
        cmpa_page.as_ptr(),
        cmpa_page.len() as u32,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_program_phrase(cmpa_scratch=0x{:08x}, len=0x{:x}) failed",
            IFRScratchAreaBase::Cmpa as u32,
            cmpa_page.len() as u32
        );
        return Err(CmpaWriteError::FlashError(status));
    }

    let mut failed_address = 0u32;
    let mut failed_data = 0u32;
    let status = drv.flash_verify_program(
        &mut cfg,
        IFRScratchAreaBase::Cfpa as u32,
        cfpa_page.len() as u32,
        cfpa_page.as_ptr(),
        &mut failed_address,
        &mut failed_data,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_verify_program(cfpa_scratch) failed @ 0x{:08x} (data=0x{:08x})",
            failed_address, failed_data
        );
        return Err(CmpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        });
    }

    let status = drv.flash_verify_program(
        &mut cfg,
        IFRScratchAreaBase::Cmpa as u32,
        cmpa_page.len() as u32,
        cmpa_page.as_ptr(),
        &mut failed_address,
        &mut failed_data,
    );
    if status != FlashStatus::Success {
        error!(
            "cmpa: flash_verify_program(cmpa_scratch) failed @ 0x{:08x} (data=0x{:08x})",
            failed_address, failed_data
        );
        return Err(CmpaWriteError::FlashVerify {
            status,
            failed_address,
            failed_data,
        });
    }

    Ok(())
}

///  Allows a "default" secure Boot configuration to be written to CMPA and staged for the next reset. This is intended for first-time provisioning of a DEV unit, and should only be called in the Develop lifecycle state.
/// Not intended for production or factory use.
pub fn write_cmpa_default_config_to_scratch_and_reset(config: CmpaDefaultConfig) -> Result<Infallible, CmpaWriteError> {
    // Fixed policy fields — locked, not caller-configurable:
    // [1:0]   SEC_BOOT_EN  = 0b11 (EcdsaMldsaOnly: hybrid ECDSA+MLDSA only)
    // [4:3]   LP_SEC_BOOT  = 0b00 (FullAuthentication: full auth on LP wake)
    // [9:8]   ENF_CNSA     = 0b10 (CnsaTwo: CNSA 2.0 enforced)
    // [13:12] FAST_BOOT_EN = 0b11 (fast boot disabled: 0b00=enabled, any non-zero=disabled)
    // [15:14] ACTIVE_IMG_PROT = b01 GLBAC2 lock on active image (i.e. SBL)
    // [27:16] FIPS STEN; currently not used, set as 0.
    // [31:30] Disable NXP signed FW = b01 (Disable any non provisioned FW)
    // Configurable: [7:6] DICE_CSR_KEY_TYPE, [11:10] ENF_TZM_PRESET
    let secure_boot_cfg: u32 = (SecureBootLevel::EcdsaMldsaOnly as u32)            // [1:0]  = 0b11
        | ((LpWakePolicy::FullAuthentication as u32) << 3)  // [4:3]  = 0b00
        | ((config.dice_csr_key_type as u32) << 6)          // [7:6]  configurable
        | ((CnsaLevel::CnsaTwo as u32) << 8)                // [9:8]  = 0b10
        | ((config.enf_tzm_preset as u32) << 10)            // [11:10] configurable
        | (0x3 << 12)                                       // [13:12]= 0b11 fast boot disabled
        | ((XipImageProtect::WriteProtect as u32) << 14) // [15:14]= 0b10 write protect without sticky lock //TODO : Maybe XOM, but does that get in way of app authenticating the SBL?
        | (0x1 << 30); // [31:30] Disable NXP signed FW = b01 (Disable any non provisioned FW)

    let mut cmpa_page = read_cmpa_page_for_update()?;

    // Preserve the header marker in [31:16]; apply config bits to [15:0].
    let existing_boot_cfg0 =
        unsafe { ptr::read_unaligned(cmpa_page[CmpaUpdateConfigData::BootCfg0.byte_range()].as_ptr() as *const u32) };
    let boot_cfg0_word = (existing_boot_cfg0 & 0xFFFF_0000) | config.boot_cfg0.to_config_bits();

    // All the following copies are safe since the byte_ranges are defined via enums and impls to match the lengths of the respective fields.
    cmpa_page[CmpaUpdateConfigData::BootCfg0.byte_range()].copy_from_slice(&boot_cfg0_word.to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::BootCfg1.byte_range()]
        .copy_from_slice(&config.boot_cfg1.to_config_bits().to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::RotkUsage.byte_range()].copy_from_slice(&config.rotk_usage.to_u32().to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::SecureBootCfg.byte_range()].copy_from_slice(&secure_boot_cfg.to_le_bytes());
    if let Some(sbl_start_addr) = config.sbl_start_addr {
        cmpa_page[CmpaUpdateConfigData::SblStartAddr.byte_range()].copy_from_slice(&sbl_start_addr.to_le_bytes());
    }
    cmpa_page[CmpaUpdateConfigData::Rotkh.byte_range()].copy_from_slice(&config.rotkh);
    cmpa_page[CmpaUpdateConfigData::PqcRotkh.byte_range()].copy_from_slice(&config.pqc_rotkh);

    const CC_SOCU_PIN: u32 = 0x1FFFE000; // default value for CC_SOCU_PIN in CMPA, for breakdown, refer to NXP secure provisioning guide.
    const CC_SOCU_DFLT: u32 = 0xBFFF4000; // default value for CC_SOCU_DFLT in CMPA, for breakdown, refer to NXP secure provisioning guide.

    cmpa_page[CmpaUpdateConfigData::CcSocuPin.byte_range()].copy_from_slice(&CC_SOCU_PIN.to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::CcSocuDflt.byte_range()].copy_from_slice(&CC_SOCU_DFLT.to_le_bytes());

    write_cmpa_page_to_scratch(&cmpa_page)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// Individually addressable CMPA word-fields for generic patching.
/// Each variant maps to a single 32-bit word in the CMPA page.
/// Offsets are relative to IFRConfigAreaBase::Cmpa (0x0100_0200).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpaField {
    BootTimers,            // 0x0C  → 0x0100_020C
    LspiQflashCfg0,        // 0x10  → 0x0100_0210
    LspiQflashCfg1,        // 0x14  → 0x0100_0214
    LspiFlashCfg0,         // 0x18  → 0x0100_0218
    LspiFlashCfg1,         // 0x1C  → 0x0100_021C
    IspUartCfg,            // 0x20  → 0x0100_0220
    IspI2cCfg,             // 0x24  → 0x0100_0224
    IspCanCfg,             // 0x28  → 0x0100_0228
    IspSpiCfg0,            // 0x2C  → 0x0100_022C
    IspSpiCfg1,            // 0x30  → 0x0100_0230
    IspUsbId,              // 0x34  → 0x0100_0234
    IspUsbCfg,             // 0x38  → 0x0100_0238
    IspMiscCfg,            // 0x3C  → 0x0100_023C
    CcSocuPin,             // 0x40  → 0x0100_0240
    CcSocuDflt,            // 0x44  → 0x0100_0244
    VendorUsage,           // 0x48  → 0x0100_0248
    Iped0Start,            // 0x130 → 0x0100_0330
    Iped0End,              // 0x134 → 0x0100_0334
    Iped1Start,            // 0x138 → 0x0100_0338
    Iped1End,              // 0x13C → 0x0100_033C
    Iped2Start,            // 0x140 → 0x0100_0340
    Iped2End,              // 0x144 → 0x0100_0344
    Iped3Start,            // 0x148 → 0x0100_0348
    Iped3End,              // 0x14C → 0x0100_034C
    Iped4Start,            // 0x150 → 0x0100_0350
    Iped4End,              // 0x154 → 0x0100_0354
    Iped5Start,            // 0x158 → 0x0100_0358
    Iped5End,              // 0x15C → 0x0100_035C
    Iped6Start,            // 0x160 → 0x0100_0360
    Iped6End,              // 0x164 → 0x0100_0364
    Iped7Start,            // 0x168 → 0x0100_0368
    Iped7End,              // 0x16C → 0x0100_036C
    DiceX509SramBufLen,    // 0x1BC → 0x0100_03BC
    DiceX509EcdsaSramAddr, // 0x1C0 → 0x0100_03C0
    DiceX509MldsaSramAddr, // 0x1C4 → 0x0100_03C4
    DiceAliasKeySramAddr,  // 0x1C8 → 0x0100_03C8
    MldsaCertTempAddr,     // 0x1CC → 0x0100_03CC
    MldsaCertTempHash0,    // 0x1D0 → 0x0100_03D0
    MldsaCertTempHash1,    // 0x1D4 → 0x0100_03D4
    MldsaCertTempHash2,    // 0x1D8 → 0x0100_03D8
    MldsaCertTempHash3,    // 0x1DC → 0x0100_03DC
    MldsaCertTempHash4,    // 0x1E0 → 0x0100_03E0
    MldsaCertTempHash5,    // 0x1E4 → 0x0100_03E4
    MldsaCertTempHash6,    // 0x1E8 → 0x0100_03E8
    MldsaCertTempHash7,    // 0x1EC → 0x0100_03EC
    MldsaCertTempHash8,    // 0x1F0 → 0x0100_03F0
    MldsaCertTempHash9,    // 0x1F4 → 0x0100_03F4
    MldsaCertTempHash10,   // 0x1F8 → 0x0100_03F8
    MldsaCertTempHash11,   // 0x1FC → 0x0100_03FC
}

impl CmpaField {
    #[inline(always)]
    pub const fn byte_offset(self) -> usize {
        match self {
            Self::BootTimers => 0x0C,
            Self::LspiQflashCfg0 => 0x10,
            Self::LspiQflashCfg1 => 0x14,
            Self::LspiFlashCfg0 => 0x18,
            Self::LspiFlashCfg1 => 0x1C,
            Self::IspUartCfg => 0x20,
            Self::IspI2cCfg => 0x24,
            Self::IspCanCfg => 0x28,
            Self::IspSpiCfg0 => 0x2C,
            Self::IspSpiCfg1 => 0x30,
            Self::IspUsbId => 0x34,
            Self::IspUsbCfg => 0x38,
            Self::IspMiscCfg => 0x3C,
            Self::CcSocuPin => 0x40,
            Self::CcSocuDflt => 0x44,
            Self::VendorUsage => 0x48,
            Self::Iped0Start => 0x130,
            Self::Iped0End => 0x134,
            Self::Iped1Start => 0x138,
            Self::Iped1End => 0x13C,
            Self::Iped2Start => 0x140,
            Self::Iped2End => 0x144,
            Self::Iped3Start => 0x148,
            Self::Iped3End => 0x14C,
            Self::Iped4Start => 0x150,
            Self::Iped4End => 0x154,
            Self::Iped5Start => 0x158,
            Self::Iped5End => 0x15C,
            Self::Iped6Start => 0x160,
            Self::Iped6End => 0x164,
            Self::Iped7Start => 0x168,
            Self::Iped7End => 0x16C,
            Self::DiceX509SramBufLen => 0x1BC,
            Self::DiceX509EcdsaSramAddr => 0x1C0,
            Self::DiceX509MldsaSramAddr => 0x1C4,
            Self::DiceAliasKeySramAddr => 0x1C8,
            Self::MldsaCertTempAddr => 0x1CC,
            Self::MldsaCertTempHash0 => 0x1D0,
            Self::MldsaCertTempHash1 => 0x1D4,
            Self::MldsaCertTempHash2 => 0x1D8,
            Self::MldsaCertTempHash3 => 0x1DC,
            Self::MldsaCertTempHash4 => 0x1E0,
            Self::MldsaCertTempHash5 => 0x1E4,
            Self::MldsaCertTempHash6 => 0x1E8,
            Self::MldsaCertTempHash7 => 0x1EC,
            Self::MldsaCertTempHash8 => 0x1F0,
            Self::MldsaCertTempHash9 => 0x1F4,
            Self::MldsaCertTempHash10 => 0x1F8,
            Self::MldsaCertTempHash11 => 0x1FC,
        }
    }

    #[inline(always)]
    pub const fn byte_range(self) -> core::ops::Range<usize> {
        self.byte_offset()..(self.byte_offset() + mem::size_of::<u32>())
    }
}

/// Write arbitrary CMPA word-fields to scratch and reset.
/// `fields` is a slice of (field, value) pairs; each field is written as a little-endian u32.
/// Fields not listed are preserved from the existing CMPA page (or zeroed on first write).
pub fn write_cmpa_fields_to_scratch_and_reset(fields: &[(CmpaField, u32)]) -> Result<Infallible, CmpaWriteError> {
    let mut cmpa_page = read_cmpa_page_for_update()?;
    for (field, value) in fields {
        cmpa_page[field.byte_range()].copy_from_slice(&value.to_le_bytes());
    }
    write_cmpa_page_to_scratch(&cmpa_page)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// First boot IFR (CMPA / CFPA) provisioning for DEV and/or factory floor, non-security critical.
/// Keeps secure boot, DICE and TZM disabled, and sets a safe default for other fields.
pub fn set_ifr_initial_config_and_reset() -> Result<Infallible, CmpaWriteError> {
    // Have SecBoot disabled initially, TZM set to not configured. Eventually provisioning flow will set these to the desired values. The following is a safe default for first-time provisioning.
    let secure_boot_cfg: u32 = (SecureBootLevel::AllAllowed as u32)
        | ((LpWakePolicy::FullAuthentication as u32) << 3)
        | ((DiceCsrKeyType::EccP384AndMlDsa87 as u32) << 6) // DICE is turned off at this stage, this option is not relevant.
        | ((CnsaLevel::CnsaTwo as u32) << 8)
        | ((TzmPreset::Ignored as u32) << 10) // Ignore TZM for first stage.
        | (0x3 << 12)                                       // [13:12]= 0b11 fast boot disabled
        | ((XipImageProtect::FlashAclSettingCfpa as u32) << 14) // Set this to Flash ACL settings for now; Since CFPA is zeroed out, Flash sectors are all set to RW unlocked (0). Set to write-protect with sticky when SecBoot is enabled.
        | (0x1 << 30); // [31:30] Disable NXP signed FW = b01 (Disable any non provisioned FW)

    let mut cmpa_page = read_cmpa_page_for_update()?;

    // Preserve the header marker in [31:16]; apply config bits to [15:0].
    let existing_boot_cfg0 =
        unsafe { ptr::read_unaligned(cmpa_page[CmpaUpdateConfigData::BootCfg0.byte_range()].as_ptr() as *const u32) };
    let boot_cfg0_word = (existing_boot_cfg0 & 0xFFFF_0000) | CmpaBootCfg0Write::default().to_config_bits();

    // All the following copies are safe since the byte_ranges are defined via enums and impls to match the lengths of the respective fields.
    cmpa_page[CmpaUpdateConfigData::BootCfg0.byte_range()].copy_from_slice(&boot_cfg0_word.to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::BootCfg1.byte_range()]
        .copy_from_slice(&CmpaBootCfg1Write::default().to_config_bits().to_le_bytes());

    let rotk_usage = CmpaRotkUsage {
        rotk: [NbootRootKeyUsage::All; 4], //Leave as is OR quantify later (combinations possiible).
        skip_dice: true,                   // Skip DICE for first stage, since DICE is not yet provisioned.
        dice_inc_nxp_cfg: false,
        dice_inc_cust_cfg: false,
        dice_inc_nxp_field_cfg: false,
    };
    cmpa_page[CmpaUpdateConfigData::RotkUsage.byte_range()].copy_from_slice(&rotk_usage.to_u32().to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::SecureBootCfg.byte_range()].copy_from_slice(&secure_boot_cfg.to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::LSpiCfg0.byte_range()]
        .copy_from_slice(&CmpaLspiCfg0Write::default().to_config_bits().to_le_bytes());
    let sbl_start_addr: u32 = 0x1000_0000; // 0x0 Or secure alias?
    cmpa_page[CmpaUpdateConfigData::SblStartAddr.byte_range()].copy_from_slice(&sbl_start_addr.to_le_bytes());

    const CC_SOCU_PIN: u32 = 0x1FFFE000; // default value being used for CC_SOCU_PIN in CMPA, for breakdown, refer to NXP secure provisioning guide.
    const CC_SOCU_DFLT: u32 = 0xBFFF4000; // default value being used for CC_SOCU_DFLT in CMPA, for breakdown, refer to NXP secure provisioning guide.

    cmpa_page[CmpaUpdateConfigData::CcSocuPin.byte_range()].copy_from_slice(&CC_SOCU_PIN.to_le_bytes());
    cmpa_page[CmpaUpdateConfigData::CcSocuDflt.byte_range()].copy_from_slice(&CC_SOCU_DFLT.to_le_bytes());

    write_cmpa_page_to_scratch(&cmpa_page)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// Enables secure boot policies in CMPA, configures the provided Root of trust hashes and resets the device to apply the changes.
/// Must ensure that a signed SBL and signed application(s) are present along with the correct ROTK hashes (BOTH ECDSA and MLDSA) as the provided ROTKH values will be matched at least against the SBL, and more images
/// if configured by input 'starting_addresses' to do so. The first address must be the Secure Boot Loader (SBL) image, which is expected to be the first image in internal flash at 0x0. The caller must ensure that the SBL and application(s)
// are signed and present in flash and that the correct ROTKH set is provided as inputs before calling this function, otherwise hashes and secure boot will not be provisioned.
pub fn configure_rotkh_and_enable_secure_boot_policies_and_reset(
    mut peri: Peri<'_, peripherals::SGI0>,
    starting_addresses: &[u32],
    rotkh: &[u32; 12],
    pqc_rotkh: &[u32; 12],
) -> Result<Infallible, CmpaWriteError> {
    if is_cmpa_erased() || !cmpa_header_marker_is_valid() {
        return Err(CmpaWriteError::ConfigError);
    }
    if load_lifecycle_from_cfpa() != Some(NbootLifecycleState::Develop) {
        return Err(CmpaWriteError::LCStateInvalid);
    }
    if starting_addresses.is_empty() {
        return Err(CmpaWriteError::InvalidInput);
    }
    if starting_addresses[0] != 0x0000_0000 {
        return Err(CmpaWriteError::InvalidInput); //The first image must be the Secure Boot Loader (SBL) image, which is expected to be the first image in internal flash at 0x0.
    }

    let mut cmpa_page = read_cmpa_page_for_update()?;

    let rotkh_bytes = unsafe { core::slice::from_raw_parts(rotkh.as_ptr() as *const u8, 48) }; // Little endian representation of the 12 u32 words (4 bytes each) = 48 bytes
    let pqc_rotkh_bytes = unsafe { core::slice::from_raw_parts(pqc_rotkh.as_ptr() as *const u8, 48) };

    const MAX_PROVISIONED_IMAGE_SIZE: u32 = 2 * 1024 * 1024; // Max 2MB flash.

    for &starting_address in starting_addresses {
        let image_base = starting_address as *const u8;
        let image_header = unsafe { ImageHeader::from_ptr(image_base, MAX_PROVISIONED_IMAGE_SIZE) }
            .map_err(|_| CmpaWriteError::InvalidImageSlot)?;
        let (image_ecdsa_rkth, image_pqc_rkth) = derive_image_rkth_pair(
            peri.reborrow(),
            image_base,
            image_header.extended_header_offset(),
            image_header.image_length(),
        );
        if let Some(image_ecdsa_rkth) = image_ecdsa_rkth {
            if image_ecdsa_rkth.as_bytes() != rotkh_bytes {
                return Err(CmpaWriteError::RotkhMismatch);
            }
        } else {
            return Err(CmpaWriteError::HashError);
        }
        if let Some(image_pqc_rkth) = image_pqc_rkth {
            if image_pqc_rkth.as_bytes() != pqc_rotkh_bytes {
                return Err(CmpaWriteError::RotkhMismatch);
            }
        } else {
            return Err(CmpaWriteError::HashError);
        }
    }
    cmpa_page[CmpaUpdateConfigData::Rotkh.byte_range()].copy_from_slice(rotkh_bytes);
    cmpa_page[CmpaUpdateConfigData::PqcRotkh.byte_range()].copy_from_slice(pqc_rotkh_bytes);

    let secure_boot_cfg: u32 = (SecureBootLevel::EcdsaMldsaOnly as u32)            // [1:0]  = 0b11
        | ((LpWakePolicy::FullAuthentication as u32) << 3)          // [4:3]  = 0b00
        | ((DiceCsrKeyType::EccP384AndMlDsa87 as u32) << 6)         // [7:6]  Hybrid ECDSA+MLDSA DICE only.
        | ((CnsaLevel::CnsaTwo as u32) << 8)                        // [9:8]  = 0b10
        | ((TzmPreset::Enforce as u32) << 10)                       // [11:10] Enforce TZM preset (if present)
        | (0x3 << 12)                                               // [13:12]= 0b11 fast boot disabled
        | ((XipImageProtect::WriteProtect as u32) << 14)      // [15:14]= 0b10 write protect without sticky lock //TODO : Maybe XOM, but does that get in way of app authenticating the SBL?
        | (0x1 << 30); // [31:30] Disable NXP signed FW = b01 (Disable any non provisioned FW)

    cmpa_page[CmpaUpdateConfigData::SecureBootCfg.byte_range()].copy_from_slice(&secure_boot_cfg.to_le_bytes());
    write_cmpa_page_to_scratch(&cmpa_page)?;
    cortex_m::peripheral::SCB::sys_reset()
}

/// Combined lifecycle verification and advance in a single call.
/// Verifies the transition is valid, stages the new lifecycle state to scratch, and resets.
/// The `From` and `Next` type parameters enforce compile-time validation of allowed transitions.
pub fn advance_lifecycle_and_reset<From, Next>(next: NbootLifecycleState) -> Result<Infallible, CfpaWriteError>
where
    From: CanAdvanceTo<Next> + ActualLifecycleState,
{
    let token = verify_lifecycle_transition::<From, Next>(next)?;
    cfpa_stage_lifecycle_advance_and_reset(token)
}
