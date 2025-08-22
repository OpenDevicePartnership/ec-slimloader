#![no_std]
#![no_main]

mod descriptors;
mod log;

use ec_slimloader_descriptors::journal::flash::FlashJournal;
use ec_slimloader_descriptors::journal::state::{Slot, State, Status};
use ec_slimloader_descriptors::{AppImageDescriptor, BootableRegionDescriptors};
use embassy_executor::Spawner;
use embedded_storage_async::nor_flash::NorFlash;
use panic_probe as _;

#[cfg(feature = "defmt")]
use defmt_rtt as _;

#[cfg(feature = "imxrt")]
mod imxrt;

#[cfg(feature = "imxrt")]
use imxrt::init;

/// Maximum buffer size on stack that is used by the bootloader.
const JOURNAL_BUFFER_SIZE: usize = 1024;

/// A board that can boot an application image.
///
/// Typically a board needs to support the intrinsics for some microcontroller and
/// contain non volatile memory that stores the multiple images and bootloading state.
trait Board {
    /// Initialize the [Board], can only be called once.
    async fn init() -> Self;

    /// Give a mutable reference to the [FlashJournal].
    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash>;

    /// Check the application image for integrity, and try to boot.
    ///
    /// Does not return if the boot is successful.
    /// Yields [BootError] if at any stage the boot is aborted.
    async fn check_and_boot(&mut self, descriptor: &AppImageDescriptor) -> BootError;

    /// Give up booting into an application.
    ///
    /// Either shut down the device or go into an infinite loop.
    fn abort(&mut self) -> !;
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum BootError {
    /// Image is too large to fit.
    TooLarge,
    /// Image cannot not possible be this small.
    TooSmall,
    /// Image did not contain the correct markers,
    Markers,
    /// Image requested to be copied into a disallowed memory region.
    MemoryRegion,
    /// What we copied from the NVM seems to have changed after initial read.
    ///
    /// Indicates a possible Man-in-the-Middle attack on the NVM.
    ChangeAfterRead,
    /// Image failed to authenticate.
    Authenticate,
    /// Failed to read the descriptor.
    #[allow(unused)]
    Descriptor(ec_slimloader_descriptors::ParseError),
}

impl From<ec_slimloader_descriptors::ParseError> for BootError {
    fn from(value: ec_slimloader_descriptors::ParseError) -> Self {
        BootError::Descriptor(value)
    }
}

/// Intent which denotes which [Slot] should be booted.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum BootIntent {
    Target,
    Backup,
}

/// Attempt booting a specific slot.
///
/// Will not return if boot is successfull, and yield [BootError] in any other case.
async fn attempt_slot(slot: Slot, board: &mut impl Board, descriptors: &BootableRegionDescriptors) -> BootError {
    match descriptors.get_app_at_slot(u8::from(slot) as u32) {
        Ok(active_app_descriptor) => board.check_and_boot(&active_app_descriptor).await,
        Err(e) => e.into(),
    }
}

/// Set a new valid [State] as the latest in the [FlashJournal].
async fn set_status<B: Board>(board: &mut B, state: &mut State, status: Status) {
    *state = state.with_status(status);
    if let Err(_e) = board.journal().set::<JOURNAL_BUFFER_SIZE>(state).await {
        panic!("Failed to update state"); // TODO print e, but requirements for defmt are in the way.
    }

    debug!("Stored new state in journal: {}", state);
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) -> ! {
    info!("Bootloader: Initializing Hardware.");

    // Load descriptors, if flashed at all.
    let descriptors = descriptors::load();

    let mut board = init().await;
    let state = board.journal().get();

    // Fetch state or set initial state.
    let mut state: State = match state {
        Some(state) => {
            info!("Latest state fetched from journal: {:?}", state);
            *state
        }
        None => {
            let slot = unwrap!(Slot::try_from(0));
            warn!(
                "Initial bootup and no state was loaded into the journal, attempting {:?}",
                slot
            );
            State::new(Status::Initial, slot, slot)
        }
    };

    // Determine our intended slot to boot.
    let intent = match state.status() {
        Status::Initial => {
            // Mark the status to [Attempting], so that the app can mark the status to [Confirmed].
            set_status(&mut board, &mut state, Status::Attempting).await;
            BootIntent::Target
        }
        Status::Attempting => {
            // When the bootloader starts with the state [Attempting],
            // it implies that an attempt was made to start the application in the slot,
            // but the application failed to mark the slot as [Confirmed].
            set_status(&mut board, &mut state, Status::Failed).await;
            BootIntent::Backup
        }
        Status::Failed => BootIntent::Backup,
        Status::Confirmed => BootIntent::Target,
    };

    // Translate the abstract intention to a concrete slot.
    let slot = match intent {
        BootIntent::Target => state.target(),
        BootIntent::Backup => state.backup(),
    };

    info!("Attempting to boot {:?} in {:?}", intent, slot);
    let error = attempt_slot(slot, &mut board, &descriptors).await; // If this function returns, it implies that the boot has failed.
    warn!("Failed to boot {:?} in {:?} because {:?}", intent, slot, error);

    // Mark our state as [Failed] if it was not set to be so already.
    if state.status() != Status::Failed {
        set_status(&mut board, &mut state, Status::Failed).await;
    }

    if slot != state.backup() {
        // There exists a separate backup slot.
        // That implies that we were in either [Initial] or [Confirmed], and now are in [Failed].
        // So attempt to boot the backup for now.

        info!("Attempting to boot backup in {:?}", slot);
        let error = attempt_slot(state.backup(), &mut board, &descriptors).await; // If this function returns, it implies that the boot has failed.
        warn!("Failed to boot backup in {:?} because {:?}", slot, error);
    }

    error!("No candidates booted successfully, giving up...");
    board.abort()
}
