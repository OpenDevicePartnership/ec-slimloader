#![no_std]
#![no_main]

use cortex_m_rt::exception;
#[cfg(feature = "defmt")]
use defmt_rtt as _;
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::{Slot, State, Status};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_executor::Spawner;
use embassy_imxrt::flexspi::embedded_storage::FlexSpiNorStorage;
use embassy_imxrt::flexspi::nor_flash::FlexSpiNorFlash;
use embassy_imxrt::gpio::{self, DriveMode, DriveStrength, Level, Output, SlewRate};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Instant, Timer};
use example_bsp::application::{ExternalStorageConfig, ExternalStorageMap};
use imxrt_rom::registers::field_sets::{BootCfg0, Rkth};
use imxrt_rom::registers::{OtpFuses, ShadowRegisters};
use partition_manager::PartitionManager;

#[allow(dead_code)]
struct Leds<'a> {
    pub red: Output<'a>,
    pub blue: Output<'a>,
    pub green: Output<'a>,
}

const JOURNAL_BUFFER_SIZE: usize = 1024;
const FUSE_DELAY: Duration = Duration::from_secs(5);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    defmt_or_log::info!("Example application");

    const SYSTEM_CORE_CLOCK_HZ: u32 = 500_000_000;
    let p = embassy_imxrt::init(Default::default());

    let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, 2, 2) } {
        Ok(ext_flash) => ext_flash,
        Err(e) => defmt_or_log::panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
    };

    let ext_flash = match unsafe { FlexSpiNorStorage::<2, 2, 4096>::new(ext_flash) } {
        Ok(ext_flash) => ext_flash,
        Err(e) => defmt_or_log::panic!("Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}", e),
    };

    let mut ext_flash_manager = PartitionManager::<_, NoopRawMutex>::new(BlockingAsync::new(ext_flash));

    let ExternalStorageMap { bl_state, .. } = ext_flash_manager.map(ExternalStorageConfig::new());

    let mut journal = match FlashJournal::new::<{ crate::JOURNAL_BUFFER_SIZE }>(bl_state).await {
        Ok(journal) => journal,
        Err(e) => defmt_or_log::panic!("Failed to initialize the flash state journal: {:?}", e),
    };

    let slot_a = Slot::S0;
    let slot_b = Slot::S1;

    let state = match journal.get() {
        Some(state) => {
            defmt_or_log::info!("Read state {}", state);
            *state
        }
        None => {
            defmt_or_log::info!("Initial state loaded");
            State::new(Status::Confirmed, slot_a, slot_b)
        }
    };

    let (slot, is_confirmed, is_backup) = match state.status() {
        Status::Initial => {
            defmt_or_log::warn!(
                "Booted into 'Initial' state, which should not be possible if the bootloader is flashed"
            );
            (state.target(), false, false)
        }
        Status::Attempting => (state.target(), false, false),
        Status::Failed => (state.backup(), false, true),
        Status::Confirmed => (state.target(), true, false),
    };

    let other_slot = if slot == slot_a { slot_b } else { slot_a };

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

    // Maps to user1 and user2 buttons on EVK.
    let mut button1 = gpio::Input::new(p.PIO1_1, gpio::Pull::None, gpio::Inverter::Disabled);
    let mut button2 = gpio::Input::new(p.PIO0_10, gpio::Pull::None, gpio::Inverter::Disabled);

    // Task to repeatedly blink the blue LED, once for Slot(0) and twice for Slot(1).
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

    // Task to blink the red LED if we are currently booted as the 'backup'.
    let backup_led_fut = async {
        if !is_backup {
            return;
        }
        loop {
            leds.red.toggle();
            Timer::after_millis(250).await;
        }
    };

    // Task to handle writing the state if we want to either attempt the other slot,
    // or want to confirm the current slot.
    let button1_fut = async move {
        // Potential new state used, but only if USER1 is pressed for a short period.
        let new_state = if is_confirmed {
            // Swap around
            State::new(Status::Initial, other_slot, slot)
        } else if is_backup {
            // Try main again
            state.with_status(Status::Initial)
        } else {
            // We were attempting so confirm
            state.with_status(Status::Confirmed)
        };

        loop {
            button1.wait_for_falling_edge().await;
            let start = Instant::now();
            button1.wait_for_rising_edge().await;
            defmt_or_log::info!("USER1");

            if start.elapsed() > FUSE_DELAY {
                let mut otp = imxrt_rom::otp::Otp::init(SYSTEM_CORE_CLOCK_HZ);
                let mut fuses = OtpFuses::writable(&mut otp, false);
                let mut shadow = ShadowRegisters::new();

                {
                    let rkth_shadow = defmt_or_log::unwrap!(shadow.rkth().read());
                    let rkth_otp = defmt_or_log::unwrap!(fuses.rkth().read());

                    if rkth_otp != rkth_shadow {
                        if rkth_shadow == Rkth::new_zero() {
                            defmt_or_log::error!("Requesting write of fuses, but RKTH is not set to something useful");
                        } else {
                            let rkth_shadow_arr: [u8; 32] = rkth_shadow.into();
                            defmt_or_log::info!("Writing RKTH fuses {:x}", rkth_shadow_arr);

                            defmt_or_log::unwrap!(fuses.rkth().write(|w| *w = rkth_shadow));
                        }
                    }
                }
                {
                    let boot0_otp = defmt_or_log::unwrap!(fuses.boot_cfg_0().read());
                    if boot0_otp != BootCfg0::new_zero() {
                        defmt_or_log::error!("Requesting write of fuses, but Boot0 seems to already be set");
                    } else {
                        defmt_or_log::info!("Writing boot0 fuse");

                        defmt_or_log::unwrap!(fuses.boot_cfg_0().write(|w| {
                            w.set_primary_boot_src(imxrt_rom::registers::BootSrc::QspiBBoot);
                            w.set_default_isp_mode(imxrt_rom::registers::DefaultIspMode::DisableIsp);
                            w.set_tzm_image_type(imxrt_rom::registers::TzmImageType::TzmEnable);
                            w.set_secure_boot_en(imxrt_rom::registers::SecureBoot::Enabled);
                            w.set_dice_skip(true);
                            w.set_boot_fail_pin_port(5);
                            w.set_boot_fail_pin_num(7);
                        }));
                    }
                }
            } else {
                defmt_or_log::info!("Writing new state: {}", new_state);
                defmt_or_log::unwrap!(journal.set::<JOURNAL_BUFFER_SIZE>(&new_state).await);
            }
        }
    };

    // Task to reboot.
    let button2_fut = async {
        button2.wait_for_falling_edge().await;
        defmt_or_log::info!("USER2");

        Timer::after_millis(100).await; // Await for defmt.
        cortex_m::peripheral::SCB::sys_reset()
    };

    embassy_futures::join::join4(led_fut, button1_fut, button2_fut, backup_led_fut).await;
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
