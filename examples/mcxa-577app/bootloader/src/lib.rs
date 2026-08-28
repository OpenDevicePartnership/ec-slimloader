//! Bootloader library — testable logic extracted from the embedded binary.
//!
//! All code here must compile on both host (x86_64, for `cargo test`) and on
//! the embedded ARM target.

#![cfg_attr(not(test), no_std)]

use ec_slimloader_state::state::{Slot, State, Status};

#[cfg(all(target_os = "none", feature = "mcxa"))]
mod mcxa;

#[cfg(all(target_os = "none", feature = "mcxa"))]
pub use mcxa::Bootloader;

#[cfg(target_os = "none")]
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct TooManySlots;

/// Size of the boot journal buffer in bytes.
#[cfg(target_os = "none")]
pub const JOURNAL_BUFFER_SIZE: usize = 4096;

/// Bootloader configuration type
pub struct Config;

impl ec_slimloader::BootStatePolicy for Config {
    fn default_state() -> State {
        State::new(Status::Initial, Slot::S0, Slot::S1)
    }

    /// Validate a boot state entry using the following rules:
    /// 1. Target and backup slots must differ.
    /// 2. Both slots must be within the valid range [0, MAX_SLOT].
    fn is_valid_state(state: &State) -> bool {
        const MAX_SLOT: u8 = 1;

        let target = state.target();
        let backup = state.backup();

        if target == backup {
            return false;
        }

        if u8::from(target) > MAX_SLOT {
            return false;
        }

        if u8::from(backup) > MAX_SLOT {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use ec_slimloader::BootStatePolicy as _;

    use super::*;

    // ── is_valid_state ──────────────────────────────────────────────────────

    #[test]
    fn valid_state_s0_target_s1_backup() {
        let state = State::new(Status::Initial, Slot::S0, Slot::S1);
        assert!(Config::is_valid_state(&state));
    }

    #[test]
    fn valid_state_s1_target_s0_backup() {
        let state = State::new(Status::Initial, Slot::S1, Slot::S0);
        assert!(Config::is_valid_state(&state));
    }

    #[test]
    fn invalid_same_slot_s0() {
        let state = State::new(Status::Initial, Slot::S0, Slot::S0);
        assert!(!Config::is_valid_state(&state));
    }

    #[test]
    fn invalid_same_slot_s1() {
        let state = State::new(Status::Initial, Slot::S1, Slot::S1);
        assert!(!Config::is_valid_state(&state));
    }

    // ── default_state ───────────────────────────────────────────────────────

    #[test]
    fn default_state_is_itself_valid() {
        let state = Config::default_state();
        assert!(Config::is_valid_state(&state));
    }

    #[test]
    fn default_state_target_is_s0() {
        let state = Config::default_state();
        assert_eq!(u8::from(state.target()), 0);
    }

    #[test]
    fn default_state_backup_is_s1() {
        let state = Config::default_state();
        assert_eq!(u8::from(state.backup()), 1);
    }
}
