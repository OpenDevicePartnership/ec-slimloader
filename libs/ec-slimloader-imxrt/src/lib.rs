#![no_std]

#[cfg(feature = "fcb")]
mod fcb;

#[cfg(not(feature = "non-secure"))]
mod verification;

#[cfg(feature = "empty-otfad")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

mod bootload;
mod mbi;

use core::ops::Range;

use defmt_or_log::{error, info, panic};
use ec_slimloader::{Board, BootError};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::Slot;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_imxrt::clocks::MainClkSrc;
use embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage;
use embassy_imxrt::flexspi::nor_flash::FlexSpiNorFlash;
use embassy_imxrt::peripherals::HASHCRYPT;
use embassy_imxrt::Peri;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use heapless::Vec;
use partition_manager::{Partition, PartitionManager, RO, RW};
use static_cell::StaticCell;

use crate::mbi::Ivt;

const IMAGE_TYPE_TZ_XIP_SIGNED: u32 = 0x0004;
const READ_ALIGNMENT: u32 = 2;
const WRITE_ALIGNMENT: u32 = 2;
const ERASE_SIZE: u32 = 4096;
const MAX_SLOT_COUNT: usize = 7;

pub type ExternalStorage = BlockingAsync<FlexSpiNorStorage<'static, READ_ALIGNMENT, WRITE_ALIGNMENT, ERASE_SIZE>>;

pub struct Partitions {
    pub state: Partition<'static, ExternalStorage, RW, NoopRawMutex>,
    pub slots: Vec<Partition<'static, ExternalStorage, RO, NoopRawMutex>, MAX_SLOT_COUNT>,
}

pub trait ImxrtConfig {
    /// Minimum and maximum image size contained within a slot.
    const SLOT_SIZE_RANGE: Range<usize>;

    /// The memory range an image is allowed to be copied to.
    const LOAD_RANGE: Range<*mut u32>;

    fn partitions(&self, flash: &'static mut PartitionManager<ExternalStorage, NoopRawMutex>) -> Partitions;
}

#[allow(dead_code)]
pub struct Imxrt<C> {
    journal: FlashJournal<Partition<'static, ExternalStorage, RW>>,
    slots: Vec<Partition<'static, ExternalStorage, RO, NoopRawMutex>, MAX_SLOT_COUNT>,
    hashcrypt: Peri<'static, HASHCRYPT>,
    _config: C,
}

trait CheckImage {
    fn check_image(&mut self, _ram_ivt: &Ivt) -> Result<(), BootError>;
}

#[cfg(feature = "non-secure")]
impl<C: ImxrtConfig> CheckImage for Imxrt<C> {
    fn check_image(&mut self, _ram_ivt: &Ivt) -> Result<(), BootError> {
        defmt_or_log::warn!("Skipped authentication because non-secure mode is set");
        Ok(())
    }
}

impl<C: ImxrtConfig> Board for Imxrt<C> {
    type Config = C;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        // Set clock to Pll but with a larger divider, otherwise
        // we get nondeterministic behaviour from the ROM API.
        let mut hal_config = embassy_imxrt::config::Config::default();
        hal_config.clocks.main_clk.src = MainClkSrc::PllMain;
        hal_config.clocks.main_clk.div_int = 4.into();
        hal_config.clocks.main_pll_clk.pfd0 = 20;
        let p = embassy_imxrt::init(hal_config);

        let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, READ_ALIGNMENT, WRITE_ALIGNMENT) }
        {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
        };

        let ext_flash =
            match unsafe { FlexSpiNorStorage::<READ_ALIGNMENT, WRITE_ALIGNMENT, ERASE_SIZE>::new(ext_flash) } {
                Ok(ext_flash) => ext_flash,
                Err(e) => panic!("Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}", e),
            };

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();
        let ext_flash_manager =
            EXT_FLASH.init_with(|| PartitionManager::<_, NoopRawMutex>::new(BlockingAsync::new(ext_flash)));

        let Partitions { state, slots } = config.partitions(ext_flash_manager);

        let journal = match FlashJournal::new::<JOURNAL_BUFFER_SIZE>(state).await {
            Ok(journal) => journal,
            Err(e) => panic!("Failed to initialize the flash state journal: {:?}", e),
        };

        Self {
            journal,
            slots,
            hashcrypt: p.HASHCRYPT,
            _config: config,
        }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    async fn check_and_boot(&mut self, slot: &Slot) -> BootError {
        let Some(slot_partition) = self.slots.get_mut(u8::from(*slot) as usize) else {
            return BootError::SlotUnknown;
        };

        // Copy the image to RAM from flash, and ensure that everything from flash is no longer available.
        let ram_ivt = {
            let slot_size = slot_partition.capacity();

            // Check if the image_len fits within the slot.
            if slot_size >= C::SLOT_SIZE_RANGE.end {
                return BootError::TooLarge;
            }

            // Verify IVT fields.
            let Ok(ivt) = mbi::Ivt::read(slot_partition).await else {
                return BootError::IO;
            };

            // Note: skboot_authenticate only supports checking XIP_SIGNED, even though we are loading it to RAM here.
            if ivt.image_type != IMAGE_TYPE_TZ_XIP_SIGNED {
                return BootError::Markers;
            }
            if ivt.image_len > slot_size {
                return BootError::TooLarge;
            }
            if ivt.image_len < C::SLOT_SIZE_RANGE.start {
                return BootError::TooSmall;
            }

            // Check if the target_ptr is within the allowed range.
            // In MBI this is called the 'load_addr', which is located in 0x34 of IVT.
            let Some(image_target_end_ptr) = ivt.target_end_ptr() else {
                return BootError::TooLarge;
            };

            if !C::LOAD_RANGE.contains(&ivt.target_ptr) || !C::LOAD_RANGE.contains(&image_target_end_ptr) {
                return BootError::MemoryRegion;
            }

            info!("Starting copy");
            let target_slice = unsafe { core::slice::from_raw_parts_mut(ivt.target_ptr as *mut u8, ivt.image_len) };
            if let Err(_e) = slot_partition.read(0, target_slice).await {
                return BootError::IO;
            }

            // Invalidate icache as we are writing to Code RAM, which is cached.
            unsafe {
                let mut p = cortex_m::Peripherals::steal();
                p.SCB.invalidate_icache();
            }
            info!("Copy done");

            let Ok(ram_ivt) = mbi::Ivt::read_from_slice(target_slice) else {
                return BootError::TooSmall;
            };

            if ivt != ram_ivt {
                return BootError::ChangeAfterRead;
            }

            ram_ivt
        };

        if let Err(e) = self.check_image(&ram_ivt) {
            error!("Failed to boot image @ {}", slot);
            return e;
        }

        info!("Booting into application @ {:?}...", ram_ivt.target_ptr);

        // Boot to application, and we do not return from this function.
        unsafe { bootload::boot_application(ram_ivt.target_ptr) }
    }

    fn abort(&mut self) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
}
