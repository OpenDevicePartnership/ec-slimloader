//! MCXA hardware glue: the external FlexSPI partition map and the
//! [`ec_slimloader_mcxa::McxaConfig`] impl for [`Config`].
//!

use defmt_or_log::{error, info};
use ec_slimloader_mcxa::{ExternalStorage as McxaExternalStorage, Partitions as McxaPartitions};
use heapless::Vec;

use crate::{Config, TooManySlots};

pub type Bootloader = ec_slimloader_mcxa::Mcxa<Config>;

partition_manager::macros::create_partition_map!(
    name: McxaExternalStorageConfig,
    map_name: McxaExternalStorageMap,
    variant: "bootloader",
    manifest: "flash-config-mcxa.toml"
);

impl ec_slimloader_mcxa::McxaConfig for Config {
    const SLOT_SIZE_RANGE: core::ops::Range<usize> = 64..(512 * 1024);

    const LOAD_RANGE: core::ops::Range<*mut u32> = (0x2000_0000 as *mut u32)..0x2008_0000 as *mut u32;

    fn partitions(
        &self,
        flash: &'static mut partition_manager::PartitionManager<
            McxaExternalStorage,
            embassy_sync::blocking_mutex::raw::NoopRawMutex,
        >,
    ) -> McxaPartitions {
        let McxaExternalStorageMap {
            app_one,
            app_two,
            boot_journal,
        } = flash.map(McxaExternalStorageConfig::new());

        let mut slots = Vec::new();

        let status = slots.push(app_one).map_err(|_| TooManySlots);
        if status.is_err() {
            error!("Failed to push App1 partition to slot map!");
            //panic!("Failed to push App1 partition to slot map!");
        }
        let status = slots.push(app_two).map_err(|_| TooManySlots);
        if status.is_err() {
            error!("Failed to push App2 partition to slot map!");
            //panic!("Failed to push App2 partition to slot map!");
        }

        info!("Slots in partition table: {}", slots.len());

        McxaPartitions {
            state: boot_journal,
            slots,
        }
    }
}
