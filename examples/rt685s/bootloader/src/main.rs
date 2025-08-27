#![no_std]
#![no_main]

use ec_slimloader;
use embassy_executor::Spawner;

// auto-generated version information from Cargo.toml
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    ec_slimloader::start().await
}
