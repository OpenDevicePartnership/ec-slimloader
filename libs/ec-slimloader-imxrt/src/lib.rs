#![no_std]

use core::ops::Range;

use defmt_or_log::{info, warn};
use ec_slimloader_state::journal::flash::FlashJournal;
use ec_slimloader_state::journal::state::Slot;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_imxrt::clocks::MainClkSrc;
use embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage;
use embassy_imxrt::flexspi::nor_flash::FlexSpiNorFlash;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use heapless::Vec;
use partition_manager::{Partition, PartitionManager, RO, RW};
use static_cell::StaticCell;

use ec_slimloader::{Board, BootError};

mod bootload;
mod fcb;
mod rom;

const IMAGE_TYPE_XIP_SIGNED: u32 = 0x0004;
const READ_ALIGNMENT: u32 = 2;
const WRITE_ALIGNMENT: u32 = 2;
const ERASE_SIZE: u32 = 4096;
const MAX_SLOT_COUNT: usize = 7;

#[cfg(feature = "imxrt")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

pub type ExternalStorage = BlockingAsync<FlexSpiNorStorage<'static, READ_ALIGNMENT, WRITE_ALIGNMENT, ERASE_SIZE>>;

#[derive(Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
struct IVT {
    pub image_len: usize,
    pub image_type: u32,
    pub target_ptr: *mut u32,
}

struct BufferTooSmall;

impl IVT {
    pub async fn read<F: ReadNorFlash>(slot: &mut F) -> Result<Self, F::Error> {
        let mut buf = [0u8; 64];
        slot.read(0, &mut buf).await?;

        // Note(unsafe): our buffer is 64 bytes large.
        Ok(unsafe { Self::read_from_slice(&buf).unwrap_unchecked() })
    }

    pub fn read_from_slice(data: &[u8]) -> Result<Self, BufferTooSmall> {
        if data.len() < 64 {
            return Err(BufferTooSmall);
        }

        // Note(unsafe): we are taking byte slices 4 bytes long, so they should map perfectly to 4 byte arrays.
        Ok(Self {
            image_len: u32::from_le_bytes(unsafe { data[0x20..0x24].try_into().unwrap_unchecked() }) as usize,
            image_type: u32::from_le_bytes(unsafe { data[0x24..0x28].try_into().unwrap_unchecked() }),
            target_ptr: u32::from_le_bytes(unsafe { data[0x34..0x38].try_into().unwrap_unchecked() }) as *mut u32,
        })
    }

    pub fn target_end_ptr(&self) -> Option<*mut u32> {
        (self.target_ptr as usize)
            .checked_add(self.image_len)
            .map(|ptr| ptr as *mut u32)
    }
}

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

pub struct Imxrt<C> {
    journal: FlashJournal<Partition<'static, ExternalStorage, RW>>,
    slots: Vec<Partition<'static, ExternalStorage, RO, NoopRawMutex>, MAX_SLOT_COUNT>,
    _config: C,
}

impl<C: ImxrtConfig> Board for Imxrt<C> {
    type Config = C;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        // Set clock to Pll but with a larger divider, otherwise
        // we get nondeterministic behaviour from the ROM API.
        let mut hal_config = embassy_imxrt::config::Config::default();
        hal_config.clocks.main_clk.src = MainClkSrc::PllMain;
        hal_config.clocks.main_clk.div_int = 4.into();
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
            _config: config,
        }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    async fn check_and_boot(&mut self, slot: &Slot) -> BootError {
        let slot_partition = match self.slots.get_mut(u8::from(*slot) as usize) {
            Some(slot) => slot,
            None => return BootError::SlotUnknown,
        };

        // Copy the image to RAM from flash, and ensure that everything from flash is no longer available.
        let ram_ivt = {
            let slot_size = slot_partition.capacity();

            // Check if the image_len fits within the slot.
            if slot_size >= C::SLOT_SIZE_RANGE.end {
                return BootError::TooLarge;
            }

            // Verify IVT fields.
            let ivt = match IVT::read(slot_partition).await {
                Ok(ivt) => ivt,
                Err(_) => return BootError::IO,
            };

            // Note: skboot_authenticate only supports checking XIP_SIGNED, even though we are loading it to RAM here.
            if ivt.image_type & 0xFF != IMAGE_TYPE_XIP_SIGNED {
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
            let image_target_end_ptr = match ivt.target_end_ptr() {
                Some(ptr) => ptr,
                None => return BootError::TooLarge,
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

            let ram_ivt = match IVT::read_from_slice(target_slice) {
                Ok(ram_ivt) => ram_ivt,
                Err(BufferTooSmall) => return BootError::TooSmall,
            };

            if ivt != ram_ivt {
                return BootError::ChangeAfterRead;
            }

            ram_ivt
        };

        info!("Starting authenticate");

        // Call the ROM API to ensure that the image is signed and not broken or tampered with.
        // Note: skboot_authenticate will show false-negatives if your clock jitter is too high.
        // We noticed this with FFROdiv2 and MainClk > 475MHz.
        match rom::skboot_authenticate(ram_ivt.target_ptr, ram_ivt.image_len as u32) {
            Ok(()) => {}
            Err(e) => {
                warn!("Failed to authenticate {:?}", e);
                return BootError::Authenticate;
            }
        }
        info!("Booting into application...");

        // Boot to application, and we do not return from this function.
        unsafe { bootload::boot_application(ram_ivt.target_ptr) }
    }

    fn abort(&mut self) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
}
