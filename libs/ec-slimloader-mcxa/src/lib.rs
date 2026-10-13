#![cfg_attr(not(test), no_std)]

pub mod board;
pub mod error;
pub mod jump;
pub mod verification;

pub use board::{ExternalStorage, Mcxa, McxaConfig, Partitions, SlotPartition, StatePartition};
pub use embassy_mcxa::sgi;
pub use embassy_mcxa::sgi::hash::{BlockingHasher, HashMode, HashOptions, HashSize, StreamingHasher};
pub use embassy_mcxa::sgi::{Async, Blocking, InterruptHandler, SetupError as SgiSetupError, Sgi, SgiError, hash};
