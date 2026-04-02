#![no_std]
#![no_main]

#[cfg(any(feature = "defmt", feature = "log"))]
use defmt_or_log::info;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_executor::Spawner;
use panic_halt as _;

const JOURNAL_BUFFER_SIZE: usize = 4096;

#[cfg(feature = "defmt")]
defmt::timestamp!("{=u32}", 0);

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    #[cfg(any(feature = "defmt", feature = "log"))]
    info!("Starting MCXA bootloader");
    ec_slimloader::start::<ec_slimloader_mcxa::McxaBoard, JOURNAL_BUFFER_SIZE>(ec_slimloader_mcxa::Config).await
}
