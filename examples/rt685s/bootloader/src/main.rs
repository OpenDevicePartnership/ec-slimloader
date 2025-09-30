#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;
use ec_slimloader_imxrt::{ExternalStorage, Partitions};
use embassy_executor::Spawner;
use example_bsp::bootloader::{ExternalStorageConfig, ExternalStorageMap};
use heapless::Vec;
use panic_probe as _;

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

struct Config;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct TooManySlots;

const JOURNAL_BUFFER_SIZE: usize = 4096;

impl ec_slimloader_imxrt::ImxrtConfig for Config {
    const SLOT_SIZE_RANGE: core::ops::Range<usize> = 64..1024 * 1024;
    const LOAD_RANGE: core::ops::Range<*mut u32> = (0x1002_0000 as *mut u32)..0x1018_0000 as *mut u32;

    fn partitions(
        &self,
        flash: &'static mut partition_manager::PartitionManager<
            ExternalStorage,
            embassy_sync::blocking_mutex::raw::NoopRawMutex,
        >,
    ) -> Partitions {
        let ExternalStorageMap {
            app_slot0,
            app_slot1,
            bl_state,
        } = flash.map(ExternalStorageConfig::new());

        let mut slots = Vec::new();
        defmt_or_log::unwrap!(slots.push(app_slot0).map_err(|_| TooManySlots));
        defmt_or_log::unwrap!(slots.push(app_slot1).map_err(|_| TooManySlots));

        Partitions { state: bl_state, slots }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    ec_slimloader::start::<ec_slimloader_imxrt::Imxrt<Config>, JOURNAL_BUFFER_SIZE>(Config).await
}
