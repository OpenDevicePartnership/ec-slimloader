use ec_slimloader_descriptors::journal::flash::FlashJournal;
use embassy_imxrt::flexspi::nor_flash::FlexSpiNorFlash;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use partition_manager::{Partition, PartitionManager, RW};
use static_cell::StaticCell;

use crate::{imxrt::storage_async::AsyncWrapper, panic, Board};

mod fcb;
mod rom;
mod storage_async;

const JOURNAL_BUFFER_SIZE: usize = 1024;

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

type ExternalStorage = AsyncWrapper<embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage<'static, 2, 2, 4096>>;

struct Imxrt {
    journal: FlashJournal<Partition<'static, ExternalStorage, RW>>,
}

impl Board for Imxrt {
    async fn init() -> Self {
        let p = embassy_imxrt::init(Default::default());

        let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, 2, 2) } {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
        };

        let ext_flash = match unsafe {
            embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage::<2, 2, 4096>::new(ext_flash)
        } {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}", e),
        };

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();
        let ext_flash_manager =
            EXT_FLASH.init_with(|| PartitionManager::<_, NoopRawMutex>::new(AsyncWrapper(ext_flash)));

        let ExternalStorageMap { bl_state } = ext_flash_manager.map(ExternalStorageConfig::new());

        let journal = match FlashJournal::new::<JOURNAL_BUFFER_SIZE>(bl_state).await {
            Ok(journal) => journal,
            Err(e) => panic!("Failed to initialize the flash state journal: {:?}", e),
        };

        Self { journal }
    }
}

pub async fn init() -> impl Board {
    Imxrt::init().await
}

partition_manager::macros::create_partition_map!(
    name: ExternalStorageConfig,
    map_name: ExternalStorageMap,
    variant: "bootloader",
    manifest: "src/imxrt/ext-flash.toml"
);
