//! Bootloader HAL for MCXA microcontroller — slimloader backend.
//!
//! Provides MCXA-specific hardware abstraction for the ec-slimloader crate.
//! The MCXA5xx memory model (see `doc/memory.md`) is:
//!   * boot1 : internal flash `0x0000_0000` – `0x0001_0000` (64 KB, this bootloader)
//!   * app1  : internal flash `0x0001_0000` – `0x0010_8000` (992 KB)
//!   * app2  : internal flash `0x0010_8000` – `0x0020_0000` (992 KB)
//!   * boot journal + config/log partitions : external FlexSPI flash `0x0800_0000`
//!

use core::ops::Range;

use ec_slimloader::{Board, BootError, BootStatePolicy};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::Slot;
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use ec_slimloader_state::state::Status;
use embassy_mcxa::rom::NbootBoolValue;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::{ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash};
use heapless::Vec;
use mcxa_ifr::{LifeCycleState, SecBootEn};
use mcxa_security_provisioning::Provisioner;
use partition_manager::{Partition, PartitionManager, RO, RW};
use static_cell::StaticCell;

use crate::{jump, verification};

/// External flash total size (2 MB).
const EXTERNAL_FLASH_SIZE: usize = 0x0020_0000;
const READ_ALIGNMENT: usize = 1;
// 1-byte logical writes; write bridges to the ROM `partial_program`, split on
// page boundaries.
const WRITE_ALIGNMENT: usize = 1;
const ERASE_ALIGNMENT: usize = 4096;
const MAX_SLOT_COUNT: usize = 2;

/// Internal-flash base addresses of the two app banks (see `doc/memory.md`).
const APP_BASES: [usize; MAX_SLOT_COUNT] = [0x0001_0000, 0x0010_8000];

/// Emulate journal
#[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
const JOURNAL_EMULATION_WINDOW: usize = 0x4000;

#[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
static mut JOURNAL_EMULATION: [u8; JOURNAL_EMULATION_WINDOW] = [0u8; JOURNAL_EMULATION_WINDOW];

#[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
fn in_emulation_window(offset: usize, len: usize) -> bool {
    offset < JOURNAL_EMULATION_WINDOW
        && offset
            .checked_add(len)
            .is_some_and(|end| end <= JOURNAL_EMULATION_WINDOW)
}

// ─── FlexSPI NOR backend (MCXA hardware only) ─────────────────────────────
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::clocks::config::{FircConfig, FircFreqSel, VddDriveStrength, VddLevel};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::rom::{
    FlexspiNor, FlexspiNorPins, FlexspiSerialClockFrequency, SerialNorConfigOption, SerialNorDeviceType,
    SerialNorFlashConnection, SerialNorMiscMode, SerialNorOptionPadEncoding, SerialNorOptionSize,
    SerialNorQuadModeSetting, pack_serial_nor_option0, pack_serial_nor_option1,
};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::{Peri, peripherals};

/// External NOR page size; ROM reads/programs are chunked to at most one page.
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
const FLASH_PAGE_SIZE: usize = 256;

pub type StatePartition = Partition<'static, ExternalStorage, RW, NoopRawMutex>;
pub type SlotPartition = Partition<'static, ExternalStorage, RO, NoopRawMutex>;

/// MCXA slimloader backend trait — hardware-specific config for MCXA5xx.
pub trait McxaConfig: ec_slimloader::BootStatePolicy {
    /// Valid SRAM range for loading app slots (unused on MCXA XIP boot, kept
    /// for parity with other backends).
    const SLOT_SIZE_RANGE: Range<usize>;

    /// Valid SRAM address range for app execution (unused on MCXA XIP boot).
    const LOAD_RANGE: Range<*mut u32>;

    /// Provide partition map (app slots + boot state).
    fn partitions(&self, flash: &'static mut PartitionManager<ExternalStorage, NoopRawMutex>) -> Partitions;
}

/// Partitions view: app slots + boot state partition.
pub struct Partitions {
    pub state: StatePartition,
    pub slots: Vec<SlotPartition, MAX_SLOT_COUNT>,
}

/// MCXA external flash storage backend.
///
/// On MCXA hardware (`mcxa5xx`) this drives the external QSPI NOR through the
/// FlexSPI controller using the MCXA ROM FlexSPI NOR driver (read /
/// sector-erase / partial-program), so the boot journal is really persisted in
/// external flash. On host builds it falls back to a RAM shadow so the boot
/// state machine can be unit-tested without hardware.
pub struct ExternalStorage {
    #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
    nor: FlexspiNor<'static>,
}

impl ExternalStorage {
    /// Wrap an initialized ROM FlexSPI NOR handle (real external flash).
    #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
    pub fn new(nor: FlexspiNor<'static>) -> Self {
        Self { nor }
    }

    /// Host / non-hardware builds: prime the RAM-shadow journal emulation.
    #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
    pub fn init() -> Self {
        // The journal shadow lives in `.bss`; prime it to the NOR erased state
        // (0xFF) before first use.
        // SAFETY: called once during bootloader init, before any read/write.
        unsafe {
            let win = &mut *core::ptr::addr_of_mut!(JOURNAL_EMULATION);
            win.fill(0xFF);
        }
        Self {}
    }
}

impl ErrorType for ExternalStorage {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for ExternalStorage {
    const READ_SIZE: usize = READ_ALIGNMENT;

    // Host RAM-shadow slicing is bounds-checked by `in_emulation_window`.
    #[allow(clippy::indexing_slicing)]
    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let offset_usize = offset as usize;
        let end = offset_usize.saturating_add(bytes.len());
        if end > EXTERNAL_FLASH_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }

        // ROM read needs a 32-bit aligned destination; bounce through a word
        // buffer one page at a time.
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            let mut start: u32 = offset;
            let mut curr: usize = 0;
            let mut words = [0u32; FLASH_PAGE_SIZE / 4];
            while curr < bytes.len() {
                let read_len = core::cmp::min(FLASH_PAGE_SIZE, bytes.len() - curr);

                self.nor
                    .read(&mut words, start, read_len as u32)
                    .map_err(|_| NorFlashErrorKind::Other)?;
                for (dst, word) in bytes[curr..curr + read_len].chunks_mut(4).zip(words.iter()) {
                    dst.copy_from_slice(&word.to_le_bytes()[..dst.len()]);
                }

                start += read_len as u32;
                curr += read_len;
            }
        }

        // Host / non-hardware: RAM-shadow emulation.
        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        {
            bytes.fill(0xFF);
            if in_emulation_window(offset_usize, bytes.len()) {
                // SAFETY: bounds checked by in_emulation_window.
                unsafe {
                    let win = &*core::ptr::addr_of!(JOURNAL_EMULATION);
                    bytes.copy_from_slice(&win[offset_usize..offset_usize + bytes.len()]);
                }
            }
        }

        Ok(())
    }

    fn capacity(&self) -> usize {
        EXTERNAL_FLASH_SIZE
    }
}

impl NorFlash for ExternalStorage {
    const WRITE_SIZE: usize = WRITE_ALIGNMENT;
    const ERASE_SIZE: usize = ERASE_ALIGNMENT;

    // Host RAM-shadow slicing is bounds-checked by `in_emulation_window`.
    #[allow(clippy::indexing_slicing)]
    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let from_usize = from as usize;
        let to_usize = to as usize;
        if from_usize >= to_usize || to_usize > EXTERNAL_FLASH_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        if !from_usize.is_multiple_of(Self::ERASE_SIZE) || !to_usize.is_multiple_of(Self::ERASE_SIZE) {
            return Err(NorFlashErrorKind::NotAligned);
        }

        // Real FlexSPI: erase each 4 KB sector in [from, to).
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            let mut addr = from;
            while addr < to {
                self.nor.erase_sector(addr).map_err(|_| NorFlashErrorKind::Other)?;
                addr += Self::ERASE_SIZE as u32;
            }
        }

        // Host / non-hardware: RAM-shadow emulation.
        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        {
            if !in_emulation_window(from_usize, to_usize - from_usize) {
                return Err(NorFlashErrorKind::Other);
            }
            // SAFETY: bounds checked by in_emulation_window.
            unsafe {
                let win = &mut *core::ptr::addr_of_mut!(JOURNAL_EMULATION);
                win[from_usize..to_usize].fill(0xFF);
            }
        }

        Ok(())
    }

    // Slicing in the page-split and host RAM-shadow paths is bounds-guaranteed.
    #[allow(clippy::indexing_slicing)]
    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let offset_usize = offset as usize;
        let end = offset_usize.saturating_add(bytes.len());
        if end > EXTERNAL_FLASH_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        // WRITE_SIZE is 1, so every offset/length is aligned (no check needed).

        // ROM `partial_program` programs an arbitrary byte span from a 32-bit
        // aligned source. Bounce through a word buffer and never cross a page
        // boundary in a single program operation.
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            let mut start: u32 = offset;
            let mut curr: usize = 0;
            let mut words = [u32::MAX; FLASH_PAGE_SIZE / 4];
            while curr < bytes.len() {
                let page_remaining = FLASH_PAGE_SIZE - start as usize % FLASH_PAGE_SIZE;
                let write_len = core::cmp::min(page_remaining, bytes.len() - curr);

                for (word, chunk) in words.iter_mut().zip(bytes[curr..curr + write_len].chunks(4)) {
                    let mut bounce = [0xFF; 4];
                    bounce[..chunk.len()].copy_from_slice(chunk);
                    *word = u32::from_le_bytes(bounce);
                }
                self.nor
                    .partial_program(start, &words, write_len as u32)
                    .map_err(|_| NorFlashErrorKind::Other)?;

                start += write_len as u32;
                curr += write_len;
            }
        }

        // Host / non-hardware: RAM-shadow emulation (NOR 1 -> 0 semantics).
        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        {
            if !in_emulation_window(offset_usize, bytes.len()) {
                return Err(NorFlashErrorKind::Other);
            }
            // SAFETY: bounds checked by in_emulation_window.
            unsafe {
                let win = &mut *core::ptr::addr_of_mut!(JOURNAL_EMULATION);
                for (i, src) in bytes.iter().enumerate() {
                    win[offset_usize + i] &= *src;
                }
            }
        }

        Ok(())
    }
}

/// Entry point called from bootloader main(). Handles:
/// 1. External-flash (FlexSPI) journal read
/// 2. Boot state validation
/// 3. App bank selection (with rollback fallback)
/// 4. Jump to selected app in internal flash once hybrid ECDSA+MLDSA signature is verified.
pub struct Mcxa<C: McxaConfig> {
    journal: FlashJournal<StatePartition>,
    slots: Vec<SlotPartition, MAX_SLOT_COUNT>,
    _config: C,
    #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
    sgi: Peri<'static, peripherals::SGI0>,
}

/// Erased NOR flash reads as `0xFFFF_FFFF`, which fails both checks, so an empty
/// app slot is rejected
#[cfg(any(test, all(target_os = "none", feature = "mcxa5xx")))]
fn vector_table_looks_valid(initial_sp: u32, reset_handler: u32) -> bool {
    const SRAM_START: u32 = 0x2000_0000;
    const SRAM_END: u32 = 0x200A_0000;
    const FLASH_END: u32 = 0x0020_0000;

    let sp_ok = (SRAM_START..=SRAM_END).contains(&initial_sp);
    let reset_ok = (reset_handler & 1) == 1 && reset_handler < FLASH_END;

    defmt_or_log::trace!(
        "initial_sp: {:X} (ok: {}), reset_handler: {:X} (ok: {})",
        initial_sp,
        sp_ok,
        reset_handler,
        reset_ok
    );

    sp_ok && reset_ok
}

/// Validate that an internal-flash app slot actually holds an application image
/// before jumping to it
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
fn slot_has_valid_app(app_base: usize) -> bool {
    let initial_sp = unsafe { core::ptr::read_volatile(app_base as *const u32) };
    let reset_handler = unsafe { core::ptr::read_volatile((app_base + 4) as *const u32) };
    vector_table_looks_valid(initial_sp, reset_handler)
}

impl<C: McxaConfig + BootStatePolicy> Board for Mcxa<C> {
    type Config = C;

    #[allow(clippy::panic)]
    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        defmt_or_log::info!("Initializing MCXA slimloader backend");

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let p = {
            let mut bl_cfg = embassy_mcxa::config::Config::default();

            // Enable 96Mhz FIRC.
            let mut fcfg = FircConfig::default();
            fcfg.frequency = FircFreqSel::Mhz96;
            bl_cfg.clock_cfg.firc = Some(fcfg);

            // 96 MHz exceeds the MidDrive clock limit; use NormalMode.
            bl_cfg.clock_cfg.vdd_power.active_mode.level = VddLevel::NormalMode;
            bl_cfg.clock_cfg.vdd_power.active_mode.drive = VddDriveStrength::Normal;
            bl_cfg.clock_cfg.vdd_power.low_power_mode.level = VddLevel::NormalMode;
            bl_cfg.clock_cfg.vdd_power.low_power_mode.drive = VddDriveStrength::Normal;

            embassy_mcxa::init(bl_cfg)
        };

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let sgi = p.SGI0;

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let external = {
            // Reset FlexSPI so the ROM get_config/init start from the hardware
            // default rather than inheriting leftover controller state.
            // SAFETY: FLEXSPI0 is not otherwise in use at this point in boot.
            unsafe {
                embassy_mcxa::clocks::pulse_reset::<peripherals::FLEXSPI0>();
            }

            #[cfg(not(feature = "mcxa5xxevk"))]
            let (pins, connection) = (
                FlexspiNorPins::new(
                    p.P3_17, // Port B: B_SS0 (CS0)
                    p.P3_16, // Port B: B_SCLK
                    p.P3_1,  // Port B: B_DQS
                    p.P3_15, // Port B: B_DATA0
                    p.P3_14, // Port B: B_DATA1
                    p.P3_13, // Port B: B_DATA2
                    p.P3_12, // Port B: B_DATA3
                ),
                SerialNorFlashConnection::SinglePortB,
            );

            #[cfg(feature = "mcxa5xxevk")]
            let (pins, connection) = (
                FlexspiNorPins::new(
                    p.P3_0,  // Port A EVK: A_SS0 (CS0)
                    p.P3_7,  // Port A EVK: A_SCLK
                    p.P3_6,  // Port A EVK: A_DQS
                    p.P3_8,  // Port A EVK: A_DATA0
                    p.P3_9,  // Port A EVK: A_DATA1
                    p.P3_10, // Port A EVK: A_DATA2
                    p.P3_11, // Port A EVK: A_DATA3
                ),
                SerialNorFlashConnection::SinglePortA,
            );

            // SFDP SDR auto-probe (non-destructive; DDR probing would write test
            // patterns), 1-pad query/cmd, quad mode left to SFDP, 30 MHz.
            let option = SerialNorConfigOption {
                option0: pack_serial_nor_option0(
                    SerialNorOptionSize::Option0AndOption1,
                    SerialNorDeviceType::ReadSfdpSdr,
                    SerialNorOptionPadEncoding::One,
                    SerialNorOptionPadEncoding::One,
                    SerialNorQuadModeSetting::NotConfigured,
                    SerialNorMiscMode::Disabled,
                    FlexspiSerialClockFrequency::MHz30,
                ),
                option1: pack_serial_nor_option1(connection, 0, 0, 0, 0, 0),
            };

            // Keep the ROM-probed LUT; override only what the ROM can't infer:
            // - read_sample_clk_src = 0: internal dummy-read strobe (no-DQS board).
            // - Port B: disable A1/A2 so B1 sits at SFAR 0, and size B1.
            let nor = embassy_mcxa::rom::get()
                .flex_spi_with_options(p.FLEXSPI0, pins, option, |cfg| {
                    cfg.mem_config.read_sample_clk_src = 0;
                    #[cfg(not(feature = "mcxa5xxevk"))]
                    {
                        cfg.mem_config.sflash_a1_size = 0;
                        cfg.mem_config.sflash_a2_size = 0;
                        cfg.mem_config.sflash_b1_size = EXTERNAL_FLASH_SIZE as u32;
                        cfg.mem_config.sflash_b2_size = 0;
                    }
                })
                .unwrap_or_else(|_| panic!("FlexSPI init failed"));

            ExternalStorage::new(nor)
        };
        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        let external = ExternalStorage::init();

        let ext_flash_manager = EXT_FLASH.init_with(|| PartitionManager::new(external));

        let Partitions { state, slots } = config.partitions(ext_flash_manager);

        let journal = match FlashJournal::new::<JOURNAL_BUFFER_SIZE>(state).await {
            Ok(journal) => journal,
            Err(_e) => panic!("Failed to initialize MCXA journal"),
        };
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let mut journal = journal;

        // Seed the boot journal when:
        // - Journal is blank (first boot), OR
        // - Journal has a stale Failed state but S0 has a valid app (dev workflow reset).
        // Always seed toward S0 as the target when S0 is valid.
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            let needs_seed = journal.get().is_none()
                || (cfg!(feature = "mcxa5xxevk")
                    && journal.get().is_some_and(|s| matches!(s.status(), Status::Failed)));
            if needs_seed && slot_has_valid_app(APP_BASES[0]) {
                let initial_state = ec_slimloader_state::state::State::new(Status::Confirmed, Slot::S0, Slot::S1);
                match journal.set::<JOURNAL_BUFFER_SIZE>(&initial_state).await {
                    Ok(()) => {
                        defmt_or_log::info!("Initialized boot journal: target=S0 backup=S1");
                    }
                    Err(_e) => {
                        defmt_or_log::info!("Boot-journal initialization write failed");
                    }
                }
            }
        }

        Self {
            journal,
            slots,
            _config: config,
            #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
            sgi,
        }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    async fn check_and_boot(&mut self, slot: &Slot) -> ec_slimloader::BootError {
        let index = usize::from(u8::from(*slot));
        if self.slots.get_mut(index).is_none() {
            return BootError::SlotUnknown;
        }

        let Some(&app_base) = APP_BASES.get(index) else {
            return BootError::SlotUnknown;
        };

        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        {
            let _ = app_base;
            return BootError::Integrity;
        }

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            use fault_rejection::protected_if;

            if !slot_has_valid_app(app_base) {
                return BootError::Markers;
            }

            const SLOT_SIZE: u32 = 0x000F_8000; // 992 KB
            let image_base = app_base as *const u8;
            let jump_address = image_base as *const u32;
            let image_header =
                match unsafe { mcxa_security_provisioning::header::ImageHeader::from_ptr(image_base, SLOT_SIZE) } {
                    Ok(val) => val,
                    Err(e) => {
                        defmt_or_log::error!("Image header not ok: {:?}", e);
                        return ec_slimloader::BootError::Markers;
                    }
                };

            let image_len = image_header.image_length();
            let cert_offset = image_header.cert_block_offset();
            if !(0x40..=SLOT_SIZE).contains(&image_len) || (cert_offset & 0x3) != 0 || cert_offset >= image_len {
                defmt_or_log::error!("Image len or cert offset not ok");
                return ec_slimloader::BootError::Markers;
            }

            if !Self::follows_policy() {
                return BootError::Integrity;
            }

            let Err(if_result) = protected_if::<{ NbootBoolValue::True as u32 }, _, _, _>(
                || match verification::verify_authenticity(self.sgi.reborrow(), image_base, || Self::dev_mode() as u32)
                {
                    Ok(result) => result as u32,
                    Err(error) => error as u32, // Error has no overlap in value with Nboot bool, so this is safe to do
                },
                || unsafe { jump::jump_to_image(jump_address) },
            );

            match BootError::try_from(if_result) {
                Ok(err) => err,
                Err(_) => BootError::Authenticate,
            }
        }
    }

    fn abort(&mut self) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}

impl<C: McxaConfig + BootStatePolicy> Mcxa<C> {
    pub async fn start<const JOURNAL_BUFFER_SIZE: usize>(config: C) -> ! {
        ec_slimloader::start::<Self, JOURNAL_BUFFER_SIZE>(config).await
    }

    /// Check if the device authenticity should be checked in dev mode (where the keys are self-generated instead of taken from the programmed value)
    fn dev_mode() -> bool {
        let ifr = unsafe { mcxa_ifr::IFR::current() };
        let secure_boot_enabled = ifr.cmpa.secure_boot_cfg.sec_boot_en() == SecBootEn::OnlyPki;
        let develop_lifecycle = ifr.cfpa.header.cfpa_lc_state() == Ok(LifeCycleState::Develop);
        // Don't enable dev mode if secure boot is active. That way a flashed key can be tested
        develop_lifecycle && !secure_boot_enabled
    }

    /// Check if the secure boot settings are good
    fn follows_policy() -> bool {
        if Self::dev_mode() {
            true
        } else {
            unsafe { Provisioner::new_from_page(true) }
                .check_production_settings()
                .is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_bases_match_memory_md() {
        assert_eq!(APP_BASES[0], 0x0001_0000);
        assert_eq!(APP_BASES[1], 0x0010_8000);
    }

    #[test]
    fn emulation_window_bounds() {
        assert!(in_emulation_window(0, JOURNAL_EMULATION_WINDOW));
        assert!(!in_emulation_window(0, JOURNAL_EMULATION_WINDOW + 1));
        assert!(!in_emulation_window(JOURNAL_EMULATION_WINDOW, 1));
    }

    #[test]
    fn vector_table_validation_accepts_real_rejects_empty() {
        assert!(!vector_table_looks_valid(0xFFFF_FFFF, 0xFFFF_FFFF));
        assert!(!vector_table_looks_valid(0x0000_0000, 0x0000_0000));
        assert!(vector_table_looks_valid(0x2004_58B0, 0x0001_0289)); // app1 @ 0x10000
        assert!(vector_table_looks_valid(0x2009_FFF0, 0x0010_8001)); // app2 @ 0x108000
        assert!(!vector_table_looks_valid(0x0801_0000, 0x0001_0289));
        assert!(!vector_table_looks_valid(0x2004_58B0, 0x0001_0288));
        assert!(!vector_table_looks_valid(0x2004_58B0, 0x0801_0001));
    }
}
