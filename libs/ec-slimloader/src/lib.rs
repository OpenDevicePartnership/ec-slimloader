#![no_std]

use defmt_or_log::{debug, error, info, unwrap, warn};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::{Slot, State, Status};
use embedded_storage_async::nor_flash::NorFlash;

/// A trait for application specific configurations.
pub trait BootStatePolicy {
    /// Get the application specific default boot state.
    fn default_state() -> State {
        State::new(Status::Initial, unwrap!(Slot::try_from(0)), unwrap!(Slot::try_from(0)))
    }

    /// Allows application specific validation of the boot state.
    fn is_valid_state(_state: &State) -> bool {
        true
    }
}

/// A board that can boot an application image.
///
/// Typically a board needs to support the intrinsics for some microcontroller and
/// contain non volatile memory that stores the multiple images and bootloading state.
#[allow(async_fn_in_trait)]
pub trait Board {
    /// Type used to instantiate a [Board] implementation.
    type Config: BootStatePolicy;

    /// Initialize the [Board], can only be called once.
    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self;

    /// Give a mutable reference to the [FlashJournal].
    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash>;

    /// Check the application image for integrity, and try to boot.
    ///
    /// Does not return if the boot is successful.
    /// Yields [BootError] if at any stage the boot is aborted.
    async fn check_and_boot(&mut self, slot: &Slot) -> BootError;

    /// Give up booting into an application.
    ///
    /// Either shut down the device or go into an infinite loop.
    fn abort(&mut self) -> !;
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BootError {
    /// Slot is not defined.
    SlotUnknown,
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
    /// The underlying NVM threw an error.
    IO,
}

/// Intent which denotes which [Slot] should be booted.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum BootIntent {
    Target,
    Backup,
}

/// Set a new valid [State] as the latest in the [FlashJournal].
async fn set_status<B: Board, const JOURNAL_BUFFER_SIZE: usize>(board: &mut B, state: &mut State, status: Status) {
    *state = state.with_status(status);
    if let Err(_e) = board.journal().set::<JOURNAL_BUFFER_SIZE>(state).await {
        panic!("Failed to update state"); // TODO print e, but requirements for defmt are in the way.
    }

    debug!("Stored new state in journal: {:?}", state);
}

pub async fn start<B: Board, const JOURNAL_BUFFER_SIZE: usize>(config: B::Config) -> ! {
    let mut board = B::init::<JOURNAL_BUFFER_SIZE>(config).await;

    let state = board.journal().get();

    // Fetch state or set initial state.
    let mut state: State = match state {
        Some(state) => {
            info!("Latest state fetched from journal: {:?}", state);
            if B::Config::is_valid_state(state) {
                *state
            } else {
                let default_state = B::Config::default_state();
                warn!(
                    "State {:?} is invalid per application policy, using default state {:?}",
                    state, default_state
                );
                default_state
            }
        }
        None => {
            let default_state = B::Config::default_state();
            warn!(
                "Initial bootup and no state was loaded into the journal, attempting {:?}",
                default_state
            );
            default_state
        }
    };

    // Determine our intended slot to boot.
    let intent = match state.status() {
        Status::Initial => {
            // Mark the status to [Attempting], so that the app can mark the status to [Confirmed].
            set_status::<_, JOURNAL_BUFFER_SIZE>(&mut board, &mut state, Status::Attempting).await;
            BootIntent::Target
        }
        Status::Attempting => {
            // When the bootloader starts with the state [Attempting],
            // it implies that an attempt was made to start the application in the slot,
            // but the application failed to mark the slot as [Confirmed].
            set_status::<_, JOURNAL_BUFFER_SIZE>(&mut board, &mut state, Status::Failed).await;
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
    let error = board.check_and_boot(&slot).await; // If this function returns, it implies that the boot has failed.
    warn!("Failed to boot {:?} in {:?} because {:?}", intent, slot, error);

    // Mark our state as [Failed] if it was not set to be so already.
    if state.status() != Status::Failed {
        set_status::<_, JOURNAL_BUFFER_SIZE>(&mut board, &mut state, Status::Failed).await;
    }

    if slot != state.backup() {
        // There exists a separate backup slot.
        // That implies that we were in either [Initial] or [Confirmed], and now are in [Failed].
        // So attempt to boot the backup for now.

        info!("Attempting to boot backup in {:?}", slot);
        let error = board.check_and_boot(&state.backup()).await; // If this function returns, it implies that the boot has failed.
        warn!("Failed to boot backup in {:?} because {:?}", slot, error);
    }

    error!("No candidates booted successfully, giving up...");
    board.abort()
}
