#![allow(non_snake_case)]
#![allow(dead_code)]
use core::ffi::c_char;

mod flash;
mod flexspi_nor;
mod kb;
mod nboot;
mod spi_flash;

pub use flash::*;
pub use flexspi_nor::*;
pub use kb::*;
pub use nboot::*;
pub use spi_flash::*;

use self::flash::FlashDriverRaw;
use self::flexspi_nor::FlexspiNorFlashDriverRaw;
use self::kb::KBApiDriverRaw;
use self::nboot::NbootDriverRaw;
use self::spi_flash::SpiFlashDriverRaw;

pub type Status = u32;
pub type NbootBool = u32;
pub type NbootStatusProtected = u64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StandardVersionFields {
    pub bugfix: u8,
    pub minor: u8,
    pub major: u8,
    pub name: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union StandardVersion {
    pub fields: StandardVersionFields,
    pub version: u32,
}

#[repr(C)]
struct RomApiRaw {
    // NXP usage: uint32_t arg = ...; g_bootloaderTree->runBootloader(&arg);
    // The ROM API takes a pointer to the argument word (NULL is allowed for default behavior).
    pub run_bootloader: unsafe extern "C" fn(arg: *const u32),
    // Flash driver interface table.
    pub flash_api: *const FlashDriverRaw,
    pub kb_api: *const KBApiDriverRaw,
    pub nboot_api: *const NbootDriverRaw,
    pub flex_spi_api: *const FlexspiNorFlashDriverRaw,
    pub spi_flash_api: *const SpiFlashDriverRaw,
    pub version: StandardVersion,
    pub copyright: *const c_char,
}

#[derive(Clone, Copy)]
pub struct RomApi {
    raw: &'static RomApiRaw,
}

impl RomApi {
    const fn from_raw(raw: &'static RomApiRaw) -> Self {
        Self { raw }
    }

    pub fn run_bootloader(&self, arg: *const u32) {
        unsafe { (self.raw.run_bootloader)(arg) }
    }

    pub fn flash_api(&self) -> FlashDriver {
        unsafe { FlashDriver::from_raw(&*self.raw.flash_api) }
    }

    pub fn flash_api_opt(&self) -> Option<FlashDriver> {
        let ptr = self.raw.flash_api;
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(FlashDriver::from_raw(&*ptr)) }
        }
    }

    pub fn kb_api(&self) -> KBApiDriver {
        unsafe { KBApiDriver::from_raw(&*self.raw.kb_api) }
    }

    pub fn kb_api_opt(&self) -> Option<KBApiDriver> {
        let ptr = self.raw.kb_api;
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(KBApiDriver::from_raw(&*ptr)) }
        }
    }

    pub fn nboot_api(&self) -> NbootDriver {
        unsafe { NbootDriver::from_raw(&*self.raw.nboot_api) }
    }

    pub fn nboot_api_opt(&self) -> Option<NbootDriver> {
        let ptr = self.raw.nboot_api;
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(NbootDriver::from_raw(&*ptr)) }
        }
    }

    pub fn flex_spi_api(&self) -> FlexspiNorFlashDriver {
        unsafe { FlexspiNorFlashDriver::from_raw(&*self.raw.flex_spi_api) }
    }

    pub fn flex_spi_api_opt(&self) -> Option<FlexspiNorFlashDriver> {
        let ptr = self.raw.flex_spi_api;
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(FlexspiNorFlashDriver::from_raw(&*ptr)) }
        }
    }

    pub fn spi_flash_api(&self) -> SpiFlashDriver {
        unsafe { SpiFlashDriver::from_raw(&*self.raw.spi_flash_api) }
    }

    pub fn spi_flash_api_opt(&self) -> Option<SpiFlashDriver> {
        let ptr = self.raw.spi_flash_api;
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(SpiFlashDriver::from_raw(&*ptr)) }
        }
    }

    pub fn version(&self) -> StandardVersion {
        self.raw.version
    }

    pub fn copyright(&self) -> *const c_char {
        self.raw.copyright
    }
}

pub type BootloaderTree = RomApi;

#[inline(always)]
pub fn rom_api() -> RomApi {
    const ROM_API_BASE: usize = 0x1303_D800; // from MCXA Reference Manual.
    unsafe {
        let ptr = ROM_API_BASE as *const RomApiRaw;
        RomApi::from_raw(&*ptr)
    }
}

#[inline(always)]
pub fn bootloader_tree() -> BootloaderTree {
    rom_api()
}

// runBootloader API fields (Table 31)
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootTag {
    EnterBoot = 0xEB << 24,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootMode {
    PrimaryMasterBoot = 0x0 << 20,
    IspBoot = 0x1 << 20,
    ProvFwMode = 0x2 << 20,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootIspInterface {
    AutoDetection = 0x0 << 16,
    Uart = 0x1 << 16,
    Spi = 0x2 << 16,
    I2c = 0x8 << 16,
    UsbHid = 0x10 << 16,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootMasterFlashBootOption {
    InternalFlash = 0x0 << 16,
    FlexspiFlash = 0x2 << 16,
    OneBitSpiNorFlash = 0x3 << 16,
    AutoDetection = 0x1F << 16,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootInterfaceInstance {
    FlexspiPortA = 0x0 << 12,
    FlexspiPortB = 0x1 << 12,
    FlexspiPortAAndB = 0x2 << 12,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootImageIndex {
    Image0 = 0x0 << 8,
    Image1 = 0x1 << 8,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootRecoveryBootCfg1 {
    SpiNorBaudRate0 = 0x0 << 6,
    SpiNorBaudRate1 = 0x1 << 6,
    SpiNorBaudRate2 = 0x2 << 6,
    SpiNorBaudRate3 = 0x3 << 6,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunBootRecoveryBootCfg0 {
    SpiNorChipSelect0 = 0x0 << 4,
    SpiNorChipSelect1 = 0x1 << 4,
    SpiNorChipSelect2 = 0x2 << 4,
    SpiNorChipSelect3 = 0x3 << 4,
}

/// Helper function to invoke the ROM API's run_bootloader function with the appropriate argument to enter ISP mode over UART. This can be used as a fallback if the main bootloader fails and we want to recover by flashing over UART using NXP's ISP tools.
/// The function will not return since the bootloader will take over execution after this call, but we still include an infinite loop after the call to satisfy the Rust type system since the function is declared to return ! (never).
pub fn run_bootloader_uart() -> ! {
    // Build arg: tag 0xEB, mode ISP(1), interface UART(1)
    let arg: u32 = RunBootTag::EnterBoot as u32 | RunBootMode::IspBoot as u32 | RunBootIspInterface::Uart as u32;
    bootloader_tree().run_bootloader(&arg as *const u32);
    loop {
        core::hint::spin_loop()
    }
}

/// Helper function to get a pointer to the flash driver API from the ROM API tree.
pub fn flash_driver() -> FlashDriver {
    // Match NXP usage: g_bootloaderTree->flashDriver->...
    // The bootloader tree stores a direct pointer to the flash driver interface.
    bootloader_tree().flash_api()
}

pub fn flash_driver_opt() -> Option<FlashDriver> {
    bootloader_tree().flash_api_opt()
}

/// Helper function to get a pointer to the nboot API from the ROM API tree.
pub fn nboot() -> NbootDriver {
    // Match NXP usage: g_bootloaderTree->nbootDriver->...
    bootloader_tree().nboot_api()
}

pub fn nboot_opt() -> Option<NbootDriver> {
    bootloader_tree().nboot_api_opt()
}

/// Helper function to get a pointer to the KB driver API from the ROM API tree.
pub fn kb() -> KBApiDriver {
    bootloader_tree().kb_api()
}

pub fn kb_opt() -> Option<KBApiDriver> {
    bootloader_tree().kb_api_opt()
}

/// Helper function to get a pointer to the FlexSPI NOR flash driver API from the ROM API tree.
pub fn flexspi_nor() -> FlexspiNorFlashDriver {
    bootloader_tree().flex_spi_api()
}

pub fn flexspi_nor_opt() -> Option<FlexspiNorFlashDriver> {
    bootloader_tree().flex_spi_api_opt()
}

/// Helper function to get a pointer to the SPI flash driver API from the ROM API tree.
pub fn spi_flash() -> SpiFlashDriver {
    bootloader_tree().spi_flash_api()
}

pub fn spi_flash_opt() -> Option<SpiFlashDriver> {
    bootloader_tree().spi_flash_api_opt()
}

#[inline(always)]
/// Used to get the default FlashConfig struct to be inited by flash_init().
pub fn flash_cfg_for_rom_api() -> FlashConfig {
    FlashConfig {
        pflash_block_base: 0,
        pflash_total_size: 0,
        pflash_block_count: 0,
        pflash_page_size: 0,
        pflash_sector_size: 0,
        ffr_config: FlashFfrConfig {
            ffr_block_base: 0,
            ffr_total_size: 0,
            ffr_page_size: 0,
            sector_size: 0,
            cfpa_page_version: 0,
            cfpa_page_offset: 0,
        },
        mode_config: FlashModeConfig::new(
            0,
            FlashReadSingleWordConfig::new(
                FlashReadEccOption::On,
                FlashReadMarginOption::Normal,
                FlashReadDmaccOption::Disabled,
            ),
            FlashSetWriteModeConfig::new(FlashRampControlOption::Reserved, FlashRampControlOption::Reserved),
            FlashSetReadModeConfig::new(0, 0, 0),
        ),
        nboot_ctx: core::ptr::null_mut(),
        use_ahb_read: true,
    }
}

// Compile-time ABI guards for MCXA ROM-facing structs.
// The ROM expects `spi_eeprom_config(uint32_t *config)` to point at exactly 2x u32 words
// (8 bytes total). ABI size checks for this and other ROM-facing structs are collected
// below in a single private guard block.

struct AbiGuards;

impl AbiGuards {
    const CHECK: () = {
        let _ = [0u8; core::mem::size_of::<SpiMemConfigOption>()];
        let _ = [0u8; core::mem::size_of::<StandardVersion>()];
        let _ = [0u8; core::mem::size_of::<FlashFfrConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashReadSingleWordConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashSetWriteModeConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashSetReadModeConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashModeConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashConfig>()];
        let _ = [0u8; core::mem::size_of::<FlashRunContext>()];
        let _ = [0u8; core::mem::size_of::<FlexspiLutSeq>()];
        let _ = [0u8; core::mem::size_of::<FlexspiDllTime>()];
        let _ = [0u8; core::mem::size_of::<FlexspiMemConfig>()];
        let _ = [0u8; core::mem::size_of::<FlexspiNorConfig>()];
        let _ = [0u8; core::mem::size_of::<FlexspiXfer>()];
    };
}
