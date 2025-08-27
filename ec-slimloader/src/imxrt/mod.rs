use core::ops::Range;

use ec_slimloader_state::journal::flash::FlashJournal;
use ec_slimloader_state::journal::state::Slot;
use ec_slimloader_state::AppImageDescriptor;
use embassy_imxrt::clocks::MainClkSrc;
use embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage;
use embassy_imxrt::flexspi::nor_flash::FlexSpiNorFlash;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::NorFlash;
use partition_manager::{Partition, PartitionManager, RW};
use static_cell::StaticCell;

use crate::imxrt::storage_async::AsyncWrapper;
use crate::{info, panic, warn, Board, BootError};

mod bootload;
mod fcb;
mod rom;
mod storage_async;

const MAXIMUM_SLOT_SIZE: usize = 1024 * 1024;
const MINIMUM_IMAGE_SIZE: usize = 64; // Should at least contain an IVT.
const ALLOWED_APP_RANGE: Range<*mut u32> = (0x0002_0000 as *mut u32)..0x018_0000 as *mut u32;
const IMAGE_TYPE_XIP_SIGNED: u32 = 0x0004;

static DESCRIPTOR_SLOTS: &'static [AppImageDescriptor] = &[
    AppImageDescriptor::new_ram_image(0x800_D000, 1024 * 944),
    AppImageDescriptor::new_ram_image(0x80F_9000, 1024 * 944),
];

// auto-generated version information from Cargo.toml
#[cfg(feature = "imxrt")]
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[cfg(feature = "imxrt")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

pub unsafe fn raw_copy_to_ram(from: *const u32, to: *mut u32, len_words: usize) {
    core::ptr::copy_nonoverlapping(from, to, len_words);
}

type ExternalStorage = AsyncWrapper<FlexSpiNorStorage<'static, 2, 2, 4096>>;

partition_manager::macros::create_partition_map!(
    name: ExternalStorageConfig,
    map_name: ExternalStorageMap,
    variant: "bootloader",
    manifest: "src/imxrt/ext-flash.toml"
);

#[derive(Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
struct IVT {
    pub image_len: usize,
    pub image_type: u32,
    pub target_ptr: *mut u32,
}

impl IVT {
    pub unsafe fn read(image_ptr: *const u32) -> Self {
        Self {
            image_len: *image_ptr.byte_add(0x20) as usize,
            image_type: *image_ptr.byte_add(0x24),
            target_ptr: *image_ptr.byte_add(0x34) as *mut u32,
        }
    }

    pub fn target_end_ptr(&self) -> Option<*mut u32> {
        (self.target_ptr as usize)
            .checked_add(self.image_len)
            .map(|ptr| ptr as *mut u32)
    }
}

struct Imxrt {
    journal: FlashJournal<Partition<'static, ExternalStorage, RW>>,
}

impl Board for Imxrt {
    async fn init() -> Self {
        // Set clock to Pll but with a larger divider, otherwise
        // we get nondeterministic behaviour from the ROM API.
        let mut config = embassy_imxrt::config::Config::default();
        config.clocks.main_clk.src = MainClkSrc::PllMain;
        config.clocks.main_clk.div_int = 4.into();
        let p = embassy_imxrt::init(config);

        let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, 2, 2) } {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
        };

        let ext_flash = match unsafe { FlexSpiNorStorage::<2, 2, 4096>::new(ext_flash) } {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}", e),
        };

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();
        let ext_flash_manager =
            EXT_FLASH.init_with(|| PartitionManager::<_, NoopRawMutex>::new(AsyncWrapper(ext_flash)));

        let ExternalStorageMap { bl_state } = ext_flash_manager.map(ExternalStorageConfig::new());

        let journal = match FlashJournal::new::<{ crate::JOURNAL_BUFFER_SIZE }>(bl_state).await {
            Ok(journal) => journal,
            Err(e) => panic!("Failed to initialize the flash state journal: {:?}", e),
        };

        Self { journal }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    async fn check_and_boot(&mut self, slot: &Slot) -> BootError {
        let descriptor = match DESCRIPTOR_SLOTS.get(u8::from(*slot) as usize) {
            Some(descriptor) => descriptor,
            None => return BootError::SlotUnknown,
        };

        // Copy the image to RAM from flash, and ensure that everything from flash is no longer available.
        let ram_ivt = {
            // Fetch image size, which in MBI is located in 0x20 of IVT.
            let image_ptr = descriptor.slot_address as *const u32;
            let slot_size = descriptor.slot_size_bytes as usize;

            // Check if the image_len fits within the slot.
            if slot_size > MAXIMUM_SLOT_SIZE {
                return BootError::TooLarge;
            }

            // Verify IVT fields.
            let ivt = unsafe { IVT::read(image_ptr) };
            if ivt.image_type & 0xFF != IMAGE_TYPE_XIP_SIGNED {
                return BootError::Markers;
            }
            if ivt.image_len > slot_size {
                return BootError::TooLarge;
            }
            if ivt.image_len < MINIMUM_IMAGE_SIZE {
                return BootError::TooSmall;
            }

            // Check if the target_ptr is within the allowed range.
            // In MBI this is called the 'load_addr', which is located in 0x34 of IVT.
            let image_target_end_ptr = match ivt.target_end_ptr() {
                Some(ptr) => ptr,
                None => return BootError::TooLarge,
            };

            if !ALLOWED_APP_RANGE.contains(&ivt.target_ptr) || !ALLOWED_APP_RANGE.contains(&image_target_end_ptr) {
                return BootError::MemoryRegion;
            }

            info!("Starting copy");
            unsafe {
                raw_copy_to_ram(
                    image_ptr,
                    ivt.target_ptr,
                    ivt.image_len.div_ceil(core::mem::size_of::<u32>()),
                );
            }

            unsafe {
                let mut p = cortex_m::Peripherals::steal();
                p.SCB.invalidate_icache();
            }
            info!("Copy done");

            let ram_ivt = unsafe { IVT::read(ivt.target_ptr) };
            if ivt != ram_ivt {
                return BootError::ChangeAfterRead;
            }

            ram_ivt
        };

        info!("Starting authenticate");

        // Call the ROM API to ensure that the image is signed and not broken or tampered with.
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

pub async fn init() -> impl Board {
    Imxrt::init().await
}
