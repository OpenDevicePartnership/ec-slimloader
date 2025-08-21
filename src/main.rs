#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;

use ec_slimloader_descriptors::AppImageDescriptor;
use embassy_executor::Spawner;
use panic_probe as _;

mod descriptors;
#[cfg(feature = "imxrt")]
mod imxrt;

#[cfg(feature = "imxrt")]
use imxrt::{init, raw_copy_to_ram};

mod bootload;
mod log;

trait Board {
    async fn init() -> Self;
}

#[cfg(not(feature = "imxrt"))]
mod unsupported {
    use super::*;

    pub unsafe fn raw_copy_to_ram(_from: *const u32, _to: *mut u32, _len_words: usize) {}
    pub fn validate_crc(_app_descriptor: &AppImageDescriptor) -> bool {
        true
    }
}
use partition_manager::PartitionManager;
#[cfg(not(feature = "imxrt"))]
use unsupported::*;

fn copy_image(_app_descriptor: &AppImageDescriptor) {
    todo!()
    // TODO allow other scenarios supported from bootloader aside from copy to RAM
    // unsafe {
    //     raw_copy_to_ram(
    //         _app_descriptor.slot_address as *const u32,
    //         _app_descriptor.execution_address as *mut u32,
    //         _app_descriptor.execution_copy_size_bytes as usize / core::mem::size_of::<u32>(),
    //     );
    // }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    info!("Bootloader: Initializing Hardware.");

    // Load descriptors, if flashed at all.
    let descriptors = descriptors::load();

    let board = init().await;

    let active_slot = 1; // TODO

    let active_app_descriptor = descriptors.get_app_at_slot(active_slot);

    info!("Bootloader: Performing image copy to execution location.");
    // copy_image(&active_app_descriptor);

    // branch to location as described by descriptor
    // unsafe { bootload::boot_application(active_app_descriptor.execution_address as *const u32) };
    loop {}
}
