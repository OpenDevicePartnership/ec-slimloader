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

use crate::{header, jump, verification};
use ec_slimloader::{Board, BootError, BootStatePolicy};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::Slot;
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use ec_slimloader_state::state::Status;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::{ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash};
use heapless::Vec;
use partition_manager::{Partition, PartitionManager, RO, RW};
use static_cell::StaticCell;

macro_rules! board_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "verification-logging")]
        {
            defmt_or_log::info!($($arg)*);
        }
    };
}

/// External flash total size (2 MB).
const EXTERNAL_FLASH_SIZE: usize = 0x0020_0000;
const READ_ALIGNMENT: usize = 1;
// 1-byte logical writes; write bridges to FlexSPI's 8-byte-aligned
// `page_program` with a read-modify-write.
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
use embassy_mcxa::clocks::config::{
    CoreSleep, Div8, FircConfig, FircFreqSel, MainClockConfig, MainClockSource, VddDriveStrength, VddLevel,
};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::clocks::PoweredClock;
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::flexspi::lookup::opcodes::sdr::{CMD, RADDR, READ, WRITE};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::flexspi::lookup::{Command, Instr, LookupTable, Pads, SequenceBuilder};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::flexspi::{Blocking, ClockConfig, FlashConfig, Flexspi, NorFlash as FlexspiNor};
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
use embassy_mcxa::{peripherals, Peri};

/// Single-lane (1-bit SPI) LUT + geometry for the Macronix MX25U.
/// Port A (EVK, `mcxa5xxevk` feature): flash starts at SFAR=0, use actual size.
/// Port B (custom board, default): A1 is disabled in post-init so B1 starts at SFAR=0.
#[cfg(all(target_os = "none", feature = "mcxa5xx"))]
const MX25U_FLASH_CONFIG: FlashConfig = FlashConfig {
    flash_size_kbytes: EXTERNAL_FLASH_SIZE as u32 / 1024,
    page_size: 256,
    busy_status_polarity: true,
    busy_status_offset: 0,
    lookup_table: LookupTable::new()
        .command(
            Command::Read,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x03))
                .instr(Instr::new(RADDR, Pads::One, 0x18))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::ReadStatus,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x05))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::WriteEnable,
            SequenceBuilder::new().instr(Instr::new(CMD, Pads::One, 0x06)).build(),
        )
        .command(
            Command::ReadId,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x9F))
                .instr(Instr::new(READ, Pads::One, 0x00))
                .build(),
        )
        .command(
            Command::EraseSector,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x20))
                .instr(Instr::new(RADDR, Pads::One, 0x18))
                .build(),
        )
        .command(
            Command::PageProgram,
            SequenceBuilder::new()
                .instr(Instr::new(CMD, Pads::One, 0x02))
                .instr(Instr::new(RADDR, Pads::One, 0x18))
                .instr(Instr::new(WRITE, Pads::One, 0x00))
                .build(),
        ),
    read_seq: Command::Read as u8,
    read_status_seq: Command::ReadStatus as u8,
    write_enable_seq: Command::WriteEnable as u8,
    read_id_seq: Command::ReadId as u8,
    erase_sector_seq: Command::EraseSector as u8,
    page_program_seq: Command::PageProgram as u8,
    reset_sequence: None,
    device_mode_command: None,
};

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
/// On MCXA hardware (`mcxa5xx`) this drives the external MX25U QSPI NOR through
/// the FlexSPI controller (Port B) with single-lane IP commands (read /
/// sector-erase / page-program), so the boot journal is really persisted in
/// external flash. On host builds it falls back to a RAM shadow so the boot
/// state machine can be unit-tested without hardware.
pub struct ExternalStorage {
    #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
    nor: FlexspiNor<'static, Blocking>,
}

impl ExternalStorage {
    /// Wrap an initialized FlexSPI NOR driver (real external flash).
    #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
    pub fn new(nor: FlexspiNor<'static, Blocking>) -> Self {
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

        // Real FlexSPI IP read from external NOR.
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            self.nor
                .blocking_read(offset, bytes)
                .map_err(|_| NorFlashErrorKind::Other)?;
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
                self.nor
                    .blocking_erase_sector(addr)
                    .map_err(|_| NorFlashErrorKind::Other)?;
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

    // Slicing in the RMW and host RAM-shadow paths is bounds-guaranteed.
    #[allow(clippy::indexing_slicing)]
    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let offset_usize = offset as usize;
        let end = offset_usize.saturating_add(bytes.len());
        if end > EXTERNAL_FLASH_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        // WRITE_SIZE is 1, so every offset/length is aligned (no check needed).

        // page_program needs 8-byte-aligned address+length
        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        {
            const UNIT: usize = 8;
            let mut pos = offset_usize;
            let mut src = bytes;
            while !src.is_empty() {
                let block = pos & !(UNIT - 1);
                let in_block = pos - block;
                let n = core::cmp::min(UNIT - in_block, src.len());

                let mut buf = [0u8; UNIT];
                self.nor
                    .blocking_read(block as u32, &mut buf)
                    .map_err(|_| NorFlashErrorKind::Other)?;
                buf[in_block..in_block + n].copy_from_slice(&src[..n]);
                self.nor
                    .blocking_page_program(block as u32, &buf)
                    .map_err(|_| NorFlashErrorKind::Other)?;

                pos += n;
                src = &src[n..];
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

/// MCXA slimloader runtime — orchestrates boot sequence with MCXA HAL.
///
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
        board_info!("Initializing MCXA slimloader backend");

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let p = {
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

            // Do NOT use DeepSleep: the bootloader has no custom executor capable of
            // handling deep sleep wakeup, and DeepSleep gates peripheral clocks (including
            // FlexSPI) on WFE, which crashes the async journal read. WfeGated (the
            // default) only gates the CPU clock domain, leaving FlexSPI alive.
            bl_cfg.clock_cfg.vdd_power.core_sleep = CoreSleep::WfeGated;

            embassy_mcxa::init(bl_cfg)
        };

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let sgi = p.SGI0;

        #[cfg(all(target_os = "none", feature = "mcxa5xx"))]
        let external = {
            // FIRC is 192 MHz; use div=16 → 12 MHz serial clock, matching the
            // known-good speed from the original 48 MHz FIRC + div=4 default.
            // The MX25U standard-SPI READ (0x03) max is ~50 MHz but board
            // capacitance probably makes 48 MHz (192/4) unreliable.
            use embassy_mcxa::clocks::periph_helpers::{Div4, FlexspiClockSel};
            const FLEXSPI_DIV: Div4 = match Div4::from_divisor(16) {
                Some(d) => d,
                None => panic!("invalid FlexSPI divisor"),
            };
            const FLEXSPI_CLOCK: ClockConfig = ClockConfig {
                power: PoweredClock::NormalEnabledDeepSleepDisabled,
                source: FlexspiClockSel::FroHf,
                div: FLEXSPI_DIV,
            };

            #[cfg(not(feature = "mcxa5xxevk"))]
            let flexspi = Flexspi::new_blocking(
                p.FLEXSPI0,
                p.P3_17, // Port B: B_SS0 (CS0)
                p.P3_16, // Port B: B_SCLK
                p.P3_1,  // Port B: B_DQS
                p.P3_15, // Port B: B_DATA0
                p.P3_14, // Port B: B_DATA1
                p.P3_13, // Port B: B_DATA2
                p.P3_12, // Port B: B_DATA3
                FLEXSPI_CLOCK,
                MX25U_FLASH_CONFIG,
            )
            .unwrap_or_else(|_| panic!("FlexSPI init failed"));

            #[cfg(feature = "mcxa5xxevk")]
            let flexspi = Flexspi::new_blocking(
                p.FLEXSPI0,
                p.P3_0,  // Port A EVK: A_SS0 (CS0)
                p.P3_7,  // Port A EVK: A_SCLK
                p.P3_6,  // Port A EVK: A_DQS
                p.P3_8,  // Port A EVK: A_DATA0
                p.P3_9,  // Port A EVK: A_DATA1
                p.P3_10, // Port A EVK: A_DATA2
                p.P3_11, // Port A EVK: A_DATA3
                FLEXSPI_CLOCK,
                MX25U_FLASH_CONFIG,
            )
            .unwrap_or_else(|_| panic!("FlexSPI init failed"));
            let external = ExternalStorage::new(FlexspiNor::new(flexspi));
            // Port B only: disable A1 (size=0) so B1 starts at SFAR=0,
            // matching the driver's write_enable/read_status which use SFAR=0.
            // Port A (EVK) needs no post-init — A1 is chip_index=0 and SFAR=0
            // already targets it natively.
            #[cfg(not(feature = "mcxa5xxevk"))]
            {
                use embassy_mcxa::pac;
                pac::FLEXSPI0.flshcr1(2).write_value(pac::FLEXSPI0.flshcr1(1).read());
                pac::FLEXSPI0.flshcr2(2).write_value(pac::FLEXSPI0.flshcr2(1).read());
                pac::FLEXSPI0.dllcr(1).write_value(pac::FLEXSPI0.dllcr(0).read());
                pac::FLEXSPI0.flshcr0(0).write(|r| r.set_flshsz(0));
                pac::FLEXSPI0.mcr0().modify(|r| r.set_mdis(pac::flexspi::Mdis::Val1));
                pac::FLEXSPI0
                    .mcr0()
                    .modify(|r| r.set_rxclksrc(pac::flexspi::Rxclksrc::Val0));
                pac::FLEXSPI0.mcr0().modify(|r| r.set_mdis(pac::flexspi::Mdis::Val0));
                pac::FLEXSPI0
                    .mcr0()
                    .modify(|r| r.set_swreset(pac::flexspi::Swreset::Val1));
                while pac::FLEXSPI0.mcr0().read().swreset() == pac::flexspi::Swreset::Val1 {}
            }
            external
        };
        #[cfg(not(all(target_os = "none", feature = "mcxa5xx")))]
        let external = ExternalStorage::init();

        // FlexSPI external-flash READ probe (read-only, modifies nothing): dumps
        // the JEDEC id + boot-journal bytes so you can compare the journal
        // before/after an external write done by another tool (e.g. SurfDbg
        // 0xFF'ing the journal). If the bytes change, that tool really wrote to
        // external flash; if they don't, it didn't.
        #[cfg(all(target_os = "none", feature = "mcxa5xx", feature = "flexspi-selftest"))]
        let external =
            {
                let mut external = external;
                use embassy_mcxa::pac;
                // JEDEC/vendor id: proves the MCU can actually reach the external NOR.
                flexspi_ip_command(0, Command::ReadId as u8, 3);
                let jedec = pac::FLEXSPI0.rfdr(0).read().rxdata();
                board_info!(
                    "flexspi-probe: JEDEC id = {:#010x} (MX25U low byte should be 0xc2)",
                    jedec
                );
                // Boot-journal bytes at external offset 0x1000 (read-only).
                let mut j = [0u8; 16];
                let _ = external.read(0x1000, &mut j).await;
                board_info!(
                "flexspi-probe: journal @0x1000 = [{:#04x} {:#04x} {:#04x} {:#04x} {:#04x} {:#04x} {:#04x} {:#04x}]",
                j[0], j[1], j[2], j[3], j[4], j[5], j[6], j[7]
            );
                external
            };

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
                        #[cfg(feature = "verification-logging")]
                        board_info!("Initialized boot journal: target=S0 backup=S1");
                    }
                    Err(_e) => {
                        #[cfg(feature = "verification-logging")]
                        board_info!("Boot-journal initialization write failed");
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
            if !slot_has_valid_app(app_base) {
                return BootError::Markers;
            }

            const SLOT_SIZE: u32 = 0x000F_8000; // 992 KB
            let image_base = app_base as *const u8;
            let jump_address = image_base as *const u32;
            let image_header = match unsafe { header::ImageHeader::from_ptr(image_base, SLOT_SIZE) } {
                Ok(header) => header,
                Err(_) => return ec_slimloader::BootError::Markers,
            };

            let image_len = image_header.image_length();
            let cert_offset = image_header.cert_block_offset();
            if !(0x40..=SLOT_SIZE).contains(&image_len) || (cert_offset & 0x3) != 0 || cert_offset >= image_len {
                return ec_slimloader::BootError::Markers;
            }

            match verification::verify_authenticity(self.sgi.reborrow(), image_base) {
                Ok(()) => unsafe {
                    jump::jump_to_image(jump_address);
                },
                Err(error) => error,
            }
        }
    }

    fn abort(&mut self) -> ! {
        #[cfg(target_os = "none")]
        loop {
            cortex_m::asm::wfi();
        }

        #[cfg(not(target_os = "none"))]
        loop {
            core::hint::spin_loop();
        }
    }
}

impl<C: McxaConfig + BootStatePolicy> Mcxa<C> {
    pub async fn start<const JOURNAL_BUFFER_SIZE: usize>(config: C) -> ! {
        ec_slimloader::start::<Self, JOURNAL_BUFFER_SIZE>(config).await
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
