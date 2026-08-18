#![no_std]
#![no_main]

#[cfg(any(feature = "defmt", feature = "log"))]
use defmt_or_log::info;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_executor::Spawner;
use mcxa_577app_bootloader::{Bootloader, Config, JOURNAL_BUFFER_SIZE};
use mcxa_security_provisioning::{
    is_cfpa_erased, is_cmpa_erased, log_cmpa_write_error, set_ifr_initial_config_and_reset,
};
use panic_probe as _;

#[cfg(feature = "defmt")]
defmt::timestamp!("{=u32}", 0);

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    if is_cmpa_erased() && is_cfpa_erased() {
        #[cfg(any(feature = "defmt", feature = "log"))]
        info!("CMPA and CFPA are erased, will provision with initial state, will reset");
        // causes a reset, so we will not return from this function. on next reset, IFR will have been provisioned and we will continue to bootloader.
        match set_ifr_initial_config_and_reset() {
            Ok(infallible) => match infallible {},
            Err(e) => {
                log_cmpa_write_error(e);
                // TODO: when integrating in ADO, add a counter and possibly take action based on counter (e.g. max attempts).
                loop {
                    cortex_m::asm::wfe();
                }
            }
        }
    } else {
        #[cfg(any(feature = "defmt", feature = "log"))]
        info!("CMPA and CFPA are already written, will continue bootloader");
    }
    #[cfg(any(feature = "defmt", feature = "log"))]
    info!("Starting MCXA bootloader");
    ec_slimloader::start::<Bootloader, { JOURNAL_BUFFER_SIZE }>(Config).await
}
