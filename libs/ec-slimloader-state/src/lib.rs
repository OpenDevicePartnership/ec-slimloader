//! Descriptors and state for use with the ec-slimloader bootloader.
#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod journal;

/// re-export for matching software CRC32 checksum
pub use crc::{Crc, Digest, CRC_32_ISO_HDLC};

/// The App Image Descriptor for describing layout and usage of corresponding app image
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct AppImageDescriptor {
    /// Where the full, contiguous app image is stored
    pub slot_address: u32,

    /// The size of the app image stored at stored_address
    pub slot_size_bytes: u32,
}

impl AppImageDescriptor {
    #[allow(clippy::too_many_arguments)]
    /// Generate a copied to RAM app image descriptor with given parameters
    pub const fn new_ram_image(slot_address: u32, slot_size_bytes: u32) -> Self {
        Self {
            slot_address,
            slot_size_bytes,
        }
    }
}
