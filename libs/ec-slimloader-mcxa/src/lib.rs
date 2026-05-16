#![no_std]

mod flash_internal;

#[cfg(not(feature = "internal-only"))]
use crate::rom_api::{flash_driver, run_bootloader_uart, FLASH_API_ERASE_KEY};

pub mod certificate;
pub mod error;
pub mod header;
pub mod jump;
pub mod lifecycle;
pub mod memory;
pub mod rom_api;
pub mod verification;

use ec_slimloader::{Board, BootStatePolicy};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::{Slot, State, Status};
use embassy_mcxa::clocks::config::{
    CoreSleep, Div8, FircConfig, FircFreqSel, FlashSleep, MainClockConfig, MainClockSource, VddDriveStrength, VddLevel,
};
use embassy_mcxa::clocks::PoweredClock;
use embassy_mcxa::{peripherals, Peri};
use embedded_storage_async::nor_flash::NorFlash;
use flash_internal::InternalFlash;

pub use embassy_mcxa::sgi::hash::{BlockingHasher, DmaHasher, HashMode, HashOptions, HashSize, StreamingHasher};
pub use embassy_mcxa::sgi::{Async, Blocking, InterruptHandler, SgiError, SetupError as SgiSetupError, Sgi};
pub use embassy_mcxa::sgi;
pub use embassy_mcxa::sgi::hash;

#[cfg(any(feature = "defmt", feature = "log"))]
macro_rules! mcxa_error {
    ($($arg:tt)*) => {
        defmt_or_log::error!($($arg)*);
    };
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! mcxa_error {
    ($($arg:tt)*) => {};
}

pub struct Config;

impl BootStatePolicy for Config {
    fn default_state() -> State {
        #[cfg(feature = "internal-only")]
        {
            State::new(Status::Initial, Slot::S0, Slot::S0)
        }

        #[cfg(not(feature = "internal-only"))]
        {
            State::new(Status::Initial, Slot::S0, Slot::S1)
        }
    }

    fn is_valid_state(state: &State) -> bool {
        let target: u8 = state.target().into();
        let backup: u8 = state.backup().into();

        #[cfg(feature = "internal-only")]
        {
            target == 0 && backup == 0
        }

        #[cfg(not(feature = "internal-only"))]
        {
            (target == 0 && backup == 1) || (target == 1 && backup == 0)
        }
    }
}

pub struct McxaBoard {
    journal: FlashJournal<InternalFlash>,
    sgi: Peri<'static, peripherals::SGI0>,
}

impl Board for McxaBoard {
    type Config = Config;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(_config: Self::Config) -> Self {
        let mut bl_cfg = embassy_mcxa::config::Config::default();

        // Enable 192M FIRC, NOTE that this following configuration is intended for MCXA5xx family of MCUs.
        // Feature-gate as needed for other family of MCXA MCUs.

        let mut fcfg = FircConfig::default();
        fcfg.frequency = FircFreqSel::Mhz192;
        fcfg.power = PoweredClock::NormalEnabledDeepSleepDisabled;
        fcfg.fro_hf_enabled = true;
        fcfg.clk_hf_fundamental_enabled = false;
        fcfg.fro_hf_div = None; // Not sure what we would need the hf_div clock for here.
        bl_cfg.clock_cfg.firc = Some(fcfg);

        // Enable 12M osc to use as ostimer clock
        bl_cfg.clock_cfg.sirc.fro_12m_enabled = true;
        bl_cfg.clock_cfg.sirc.fro_lf_div = None;
        bl_cfg.clock_cfg.sirc.power = PoweredClock::AlwaysEnabled;

        // Disable 16K osc
        bl_cfg.clock_cfg.fro16k = None;

        // Disable external osc
        bl_cfg.clock_cfg.sosc = None;

        // Disable PLL
        bl_cfg.clock_cfg.spll = None;

        // Feed core from 192M osc
        bl_cfg.clock_cfg.main_clock = MainClockConfig {
            source: MainClockSource::FircHfRoot,
            power: PoweredClock::NormalEnabledDeepSleepDisabled,
            ahb_clk_div: Div8::no_div(),
        };

        // Set the core in high power active mode
        bl_cfg.clock_cfg.vdd_power.active_mode.level = VddLevel::OverDriveMode;
        bl_cfg.clock_cfg.vdd_power.active_mode.drive = VddDriveStrength::Normal;
        // Set the core in low power sleep mode
        bl_cfg.clock_cfg.vdd_power.low_power_mode.level = VddLevel::MidDriveMode;
        bl_cfg.clock_cfg.vdd_power.low_power_mode.drive = VddDriveStrength::Low { enable_bandgap: false };

        // Set "deep sleep" mode
        bl_cfg.clock_cfg.vdd_power.core_sleep = CoreSleep::DeepSleep;

        // Set flash doze, allowing internal flash clocks to be gated on sleep
        bl_cfg.clock_cfg.vdd_power.flash_sleep = FlashSleep::FlashDoze;

        let p = embassy_mcxa::init(bl_cfg);

        let flash = InternalFlash::new();
        let journal = match FlashJournal::new::<JOURNAL_BUFFER_SIZE>(flash).await {
            Ok(journal) => journal,
            Err(_) => {
                mcxa_error!("Critical: failed to initialize flash journal");
                loop {
                    cortex_m::asm::wfi();
                }
            }
        };

        Self { journal, sgi: p.SGI0 }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    #[cfg(feature = "internal-only")]
    async fn check_and_boot<const JOURNAL_BUFFER_SIZE: usize>(&mut self, slot: &Slot) -> ec_slimloader::BootError {
        let slot_i: u8 = (*slot).into();
        let (base_addr, slot_size) = match slot_i {
            0 => (memory::SLOT_A_START, memory::SLOT_A_SIZE),
            _ => return ec_slimloader::BootError::SlotUnknown,
        };

        let base = base_addr as *const u8;
        let image_header = match unsafe { header::ImageHeader::from_ptr(base, slot_size) } {
            Ok(header) => header,
            Err(_) => return ec_slimloader::BootError::Markers,
        };

        let image_len = image_header.image_length();
        let cert_offset = image_header.cert_block_offset();
        if image_len < 0x40 || image_len > slot_size || (cert_offset & 0x3) != 0 || cert_offset >= image_len {
            return ec_slimloader::BootError::Markers;
        }

        match verification::verify_authenticity(self.sgi.reborrow(), base) {
            Ok(()) => unsafe {
                jump::jump_to_image(base_addr);
            },
            Err(error) => error,
        }
    }

    #[cfg(not(feature = "internal-only"))]
    async fn check_and_boot<const JOURNAL_BUFFER_SIZE: usize>(&mut self, slot: &Slot) -> ec_slimloader::BootError {
        let slot_i: u8 = (*slot).into();
        match slot_i {
            0 => {
                let base = memory::SLOT_A_START as *const u8;
                let image_header = match unsafe { header::ImageHeader::from_ptr(base, memory::SLOT_A_SIZE) } {
                    Ok(header) => header,
                    Err(_) => return ec_slimloader::BootError::Markers,
                };

                let image_len = image_header.image_length();
                let cert_offset = image_header.cert_block_offset();
                if image_len < 0x40
                    || image_len > memory::SLOT_A_SIZE
                    || (cert_offset & 0x3) != 0
                    || cert_offset >= image_len
                {
                    return ec_slimloader::BootError::Markers;
                }

                match verification::verify_authenticity(self.sgi.reborrow(), base) {
                    Ok(()) => unsafe {
                        jump::jump_to_image(memory::SLOT_A_START);
                    },
                    Err(error) => error,
                }
            }
            1 => {
                let base_ext = memory::SLOT_B_START as *const u8;
                let image_header_ext = match unsafe { header::ImageHeader::from_ptr(base_ext, memory::SLOT_B_SIZE) } {
                    Ok(header) => header,
                    Err(_) => return ec_slimloader::BootError::Markers,
                };

                let image_len_ext = image_header_ext.image_length();
                let cert_offset_ext = image_header_ext.cert_block_offset();
                if image_len_ext < 0x40
                    || image_len_ext > memory::SLOT_B_SIZE
                    || (cert_offset_ext & 0x3) != 0
                    || cert_offset_ext >= image_len_ext
                {
                    return ec_slimloader::BootError::Markers;
                }

                // This verify call assumes memory mapped flash access for Slot B, which is true for the internal flash part built into MCXA,
                // but may not be true for all future use cases of Slot B, so may need to be revisited if we want to support more flexible loading scenarios in the future.
                if verification::verify_authenticity(self.sgi.reborrow(), base_ext).is_err() {
                    // unsafe { run_bootloader_uart() } // TODO: well, what should we actually do? entering ISP isn't necessarily best idea.
                    return ec_slimloader::BootError::Authenticate;
                }

                let mut internal = InternalFlash::new(); // Will use this to access the flash config.
                let flash = flash_driver();
                let flash_init_status = flash.flash_init(&mut internal.cfg);
                if flash_init_status != crate::error::FlashStatus::Success {
                    return crate::error::map_flash_status_to_boot_error(flash_init_status);
                }

                // Erase the whole destination slot so data from a previous larger image cannot survive
                // beyond the end of the newly copied image.
                // TODO: eventually other persistent states may need to be preserved across updates, in which case we would want to be more surgical with our erases.
                // For now just wipe the whole slot.
                let erase_len = memory::SLOT_A_SIZE;
                let erase_status =
                    flash.flash_erase_sector(&mut internal.cfg, memory::SLOT_A_START, erase_len, FLASH_API_ERASE_KEY);
                if erase_status != crate::error::FlashStatus::Success {
                    return crate::error::map_flash_status_to_boot_error(erase_status);
                }

                let aligned_len = match image_header_ext.aligned_copy_length(memory::SLOT_B_SIZE) {
                    Ok(len) => len,
                    Err(_) => return ec_slimloader::BootError::Markers,
                };
                let mut offset = 0u32;
                while offset < aligned_len {
                    let remaining_image_len = image_len_ext.saturating_sub(offset) as usize;
                    let chunk_len = remaining_image_len.min(memory::INTERNAL_FLASH_PAGE_SIZE as usize);
                    let mut page_buf = [0xffu8; memory::INTERNAL_FLASH_PAGE_SIZE as usize];
                    // only used for bounds checking.
                    let _src_end = match offset.checked_add(chunk_len as u32) {
                        Some(end) if end <= memory::SLOT_B_SIZE => end,
                        _ => return ec_slimloader::BootError::Markers,
                    };
                    let src = unsafe {
                        // The BIG assumption here is that the external flash is memory-mapped and can be read via normal pointers.
                        // This is true for the Internal flash part built into MCXA, but may not be true for all future use cases of Slot B,
                        // so may need to be revisited if we want to support more flexible loading scenarios in the future.
                        // TODO: Ideally we would have a buffer of memory::INTERNAL_FLASH_PAGE_SIZE bytes and read directly into that via FLEX SPI flash ROM API.
                        core::slice::from_raw_parts((memory::SLOT_B_START + offset) as *const u8, chunk_len)
                    };
                    let dst = match page_buf.get_mut(..chunk_len) {
                        Some(dst) => dst,
                        None => return ec_slimloader::BootError::Markers,
                    };
                    dst.copy_from_slice(src);

                    let program_status = flash.flash_program_page(
                        &mut internal.cfg,
                        memory::SLOT_A_START + offset,
                        page_buf.as_ptr(),
                        memory::INTERNAL_FLASH_PAGE_SIZE,
                    );
                    if program_status != crate::error::FlashStatus::Success {
                        return crate::error::map_flash_status_to_boot_error(program_status);
                    }

                    offset += memory::INTERNAL_FLASH_PAGE_SIZE;
                }

                let new_state = State::new(Status::Initial, Slot::S0, Slot::S1);
                if self.journal.set::<JOURNAL_BUFFER_SIZE>(&new_state).await.is_err() {
                    run_bootloader_uart();
                    // If we can't update the journal, we have no idea what state the bootloader will be in on next boot,
                    // so just enter ISP to be safe.
                }

                ec_slimloader::BootError::SlotRetryRequired
            }
            _ => ec_slimloader::BootError::SlotUnknown,
        }
    }

    fn abort(&mut self) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    fn arm_mcu_reset(&mut self) -> ! {
        const AIRCR: *mut u32 = 0xE000ED0C as *mut u32;
        const AIRCR_VECTKEY: u32 = 0x5FA << 16;
        const AIRCR_SYSRESETREQ: u32 = 1 << 2;

        unsafe {
            core::ptr::write_volatile(AIRCR, AIRCR_VECTKEY | AIRCR_SYSRESETREQ);
        }

        loop {
            cortex_m::asm::wfi();
        }
    }
}
