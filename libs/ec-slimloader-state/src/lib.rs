//! Journal for the EC Slimloader containing [state::State].
#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod flash;
pub mod state;
