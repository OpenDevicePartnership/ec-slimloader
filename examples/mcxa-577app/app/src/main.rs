#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use hal::bind_interrupts;
use hal::dma::DmaChannel;
use hal::gpio::{DriveStrength, Level, Output, SlewRate};
use hal::peripherals::SGI0;
use hal::sgi::hash::{DmaHasher, HashSize};
use hal::sgi::{InterruptHandler, Sgi};
use {defmt_rtt as _, embassy_mcxa as hal, panic_probe as _};

bind_interrupts!(struct Irqs {
    SGI => InterruptHandler<SGI0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut p = hal::init(hal::config::Config::default());

    defmt::info!("Blinky example with a sprinkle of SGI hashing");

    let mut dma_ch0 = DmaChannel::new(p.DMA0_CH0.reborrow());
    let mut hash_result = [0u8; 48];
    let mut input_data = [0u8; 256];

    for (index, byte) in input_data.iter_mut().enumerate() {
        *byte = index as u8;
    }

    let sgi = Sgi::new(p.SGI0.reborrow(), Irqs).unwrap();
    match DmaHasher::start_and_finalize(sgi, &mut dma_ch0, HashSize::Sha384, &input_data, &mut hash_result)
    .await
    {
        Ok(()) => defmt::info!("DMA hash: {=[u8]:x}", &hash_result),
        Err(e) => defmt::error!("DMA hash failed: {:?}", defmt::Debug2Format(&e)),
    }

    let mut red = Output::new(p.P2_14, Level::High, DriveStrength::Normal, SlewRate::Fast);
    let mut green = Output::new(p.P2_22, Level::High, DriveStrength::Normal, SlewRate::Fast);
    let mut blue = Output::new(p.P2_23, Level::High, DriveStrength::Normal, SlewRate::Fast);

    let mut rate = 250;

    defmt::info!("It's showtime...");

    loop {
        if rate > 1000 {
            rate = 250; // wrap rate to avoid overflow and excessively long timers.
        }
        red.toggle();
        Timer::after_millis(rate).await;

        red.toggle();
        green.toggle();
        Timer::after_millis(rate).await;

        green.toggle();
        blue.toggle();
        Timer::after_millis(rate).await;
        blue.toggle();

        Timer::after_millis(rate).await;
        rate = rate.wrapping_add(100);
    }
}
