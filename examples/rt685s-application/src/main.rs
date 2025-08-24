#![no_std]
#![no_main]

use cortex_m_rt::exception;
use defmt::info;
use defmt_rtt as _;
use ec_slimloader_descriptors::journal::{
    flash::FlashJournal,
    state::{Slot, State, Status},
};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_executor::Spawner;
use embassy_imxrt::interrupt;
use embassy_imxrt::{
    flexspi::{embedded_storage::FlexSpiNorStorage, nor_flash::FlexSpiNorFlash},
    gpio::{self, DriveMode, DriveStrength, Level, Output, SlewRate},
    interrupt::InterruptExt,
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::Timer;
use partition_manager::PartitionManager;

struct Leds<'a> {
    pub red: Output<'a>,
    pub blue: Output<'a>,
    pub green: Output<'a>,
}

partition_manager::macros::create_partition_map!(
    name: ExternalStorageConfig,
    map_name: ExternalStorageMap,
    variant: "bootloader",
    manifest: "src/ext-flash.toml"
);

const JOURNAL_BUFFER_SIZE: usize = 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Example application");

    let p = unsafe { cortex_m::Peripherals::steal() };
    let vtor = p.SCB.vtor.read() as *const u32;
    defmt::info!("VTOR: {}", vtor);

    let int = embassy_imxrt::interrupt::Interrupt::HASHCRYPT;
    unsafe {
        int.enable();
    }
    // int.pend();

    let p = embassy_imxrt::init(Default::default());

    let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, 2, 2) } {
        Ok(ext_flash) => ext_flash,
        Err(e) => defmt::panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
    };

    let ext_flash = match unsafe { FlexSpiNorStorage::<2, 2, 4096>::new(ext_flash) } {
        Ok(ext_flash) => ext_flash,
        Err(e) => defmt::panic!(
            "Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}",
            e
        ),
    };

    let mut ext_flash_manager =
        PartitionManager::<_, NoopRawMutex>::new(BlockingAsync::new(ext_flash));

    let ExternalStorageMap { bl_state } = ext_flash_manager.map(ExternalStorageConfig::new());

    let mut journal = match FlashJournal::new::<{ crate::JOURNAL_BUFFER_SIZE }>(bl_state).await {
        Ok(journal) => journal,
        Err(e) => defmt::panic!("Failed to initialize the flash state journal: {:?}", e),
    };

    let state = match journal.get() {
        Some(state) => {
            defmt::info!("Read state {}", state);
            *state
        }
        None => {
            defmt::info!("Initial state loaded");
            State::new(
                Status::Confirmed,
                defmt::unwrap!(Slot::try_from(0)),
                defmt::unwrap!(Slot::try_from(1)),
            )
        }
    };

    let (slot, is_confirmed, is_backup) = match state.status() {
        Status::Initial => {
            defmt::warn!("Booted into 'Initial' state, which should not be possible if the bootloader is flashed");
            (state.target(), false, false)
        }
        Status::Attempting => (state.target(), false, false),
        Status::Failed => (state.backup(), false, true),
        Status::Confirmed => (state.target(), true, false),
    };

    info!("Initializing GPIO");

    let mut leds = Leds {
        // Blue: blink number indicates active slot
        blue: Output::new(
            p.PIO0_26,
            Level::Low,
            DriveMode::PushPull,
            DriveStrength::Normal,
            SlewRate::Standard,
        ),
        // Red: is_backup (blinking)
        red: Output::new(
            p.PIO0_31,
            Level::Low,
            DriveMode::PushPull,
            DriveStrength::Normal,
            SlewRate::Standard,
        ),
        // Green: is_confirmed
        green: Output::new(
            p.PIO0_14,
            is_confirmed.into(),
            DriveMode::PushPull,
            DriveStrength::Normal,
            SlewRate::Standard,
        ),
    };

    let mut button1 = gpio::Input::new(p.PIO1_1, gpio::Pull::None, gpio::Inverter::Disabled);
    let mut button2 = gpio::Input::new(p.PIO0_10, gpio::Pull::None, gpio::Inverter::Disabled);

    let led_fut = async {
        let slot = u8::from(slot) + 1;
        loop {
            for _ in 0..slot {
                leds.blue.set_high();
                Timer::after_millis(200).await;
                leds.blue.set_low();
                Timer::after_millis(200).await;
            }

            Timer::after_millis(500).await;
        }
    };

    let backup_led_fut = async {
        if !is_backup {
            return;
        }
        loop {
            leds.red.toggle();
            Timer::after_millis(250).await;
        }
    };

    let button1_fut = async {
        button1.wait_for_falling_edge().await;
        info!("USER1");

        let new_state = if is_confirmed {
            // Swap around
            State::new(Status::Initial, state.backup(), state.target())
        } else if is_backup {
            // Try main again
            state.with_status(Status::Initial)
        } else {
            // We were attempting so confirm
            state.with_status(Status::Confirmed)
        };

        defmt::info!("Writing new state: {}", new_state);
        defmt::unwrap!(journal.set::<JOURNAL_BUFFER_SIZE>(&new_state).await);
    };

    let button2_fut = async {
        button2.wait_for_falling_edge().await;
        info!("USER2");

        Timer::after_millis(100).await; // Await for defmt.
        cortex_m::peripheral::SCB::sys_reset()
    };

    embassy_futures::join::join4(led_fut, button1_fut, button2_fut, backup_led_fut).await;
}

#[interrupt]
fn HASHCRYPT() {
    defmt::info!("test succeeded");
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    core::hint::black_box(&info);
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    let p = cortex_m::Peripherals::steal();
    let csfr = p.SCB.cfsr.read();
    let hfsr = p.SCB.hfsr.read();
    core::hint::black_box(&frame);
    core::hint::black_box(&csfr);
    core::hint::black_box(&hfsr);
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn NonMaskableInt() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn MemoryManagement() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn BusFault() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn UsageFault() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn SecureFault() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn SVCall() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn DebugMonitor() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn PendSV() {
    loop {
        cortex_m::asm::wfe();
    }
}

#[exception]
unsafe fn SysTick() {
    loop {
        cortex_m::asm::wfe();
    }
}
