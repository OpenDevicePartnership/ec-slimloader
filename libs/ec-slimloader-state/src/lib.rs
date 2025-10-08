//! Journal for the EC Slimloader containing [state::State].
#![cfg_attr(not(feature = "_test"), no_std)]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod flash;
pub mod state;
