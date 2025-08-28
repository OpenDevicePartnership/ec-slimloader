#![no_std]
#![no_main]

use ec_slimloader;
use embassy_executor::Spawner;

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

struct Config;

impl ec_slimloader::imxrt::ImxrtConfig for Config {
    const SLOT_SIZE_RANGE: core::ops::Range<usize> = 64..1024 * 1024;
    const LOAD_RANGE: core::ops::Range<*mut u32> =
        (0x0002_0000 as *mut u32)..0x018_0000 as *mut u32;
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    ec_slimloader::start::<ec_slimloader::imxrt::Imxrt<Config>>(Config).await
}
