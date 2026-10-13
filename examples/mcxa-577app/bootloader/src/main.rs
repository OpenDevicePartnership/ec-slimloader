#![no_std]
#![no_main]

#[cfg(any(feature = "defmt", feature = "log"))]
use defmt_or_log::info;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use embassy_executor::Spawner;
use mcxa_577app_bootloader::{Bootloader, Config, JOURNAL_BUFFER_SIZE};
use mcxa_security_provisioning::Provisioner;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    let mut provisioner = unsafe { Provisioner::new_from_page(true) };

    if provisioner.is_erased() {
        #[cfg(any(feature = "defmt", feature = "log"))]
        info!("IFR is erased, will provision with initial state, will reset");

        // causes a reset, so we will not return from this function. on next reset, IFR will have been provisioned and we will continue to bootloader.
        let Err(e) = provisioner.with_initial_config().commit_and_reboot();

        // If we got here we didn't reboot for some reason

        #[cfg(any(feature = "defmt", feature = "log"))]
        defmt_or_log::error!("{}", e);
        // TODO: when integrating in ADO, add a counter and possibly take action based on counter (e.g. max attempts).
        loop {
            cortex_m::asm::wfe();
        }
    } else {
        #[cfg(any(feature = "defmt", feature = "log"))]
        info!("IFR is already written, will continue bootloader");
    }

    #[cfg(any(feature = "defmt", feature = "log"))]
    info!("Starting MCXA bootloader");
    ec_slimloader::start::<Bootloader, { JOURNAL_BUFFER_SIZE }>(Config).await
}
