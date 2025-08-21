use ec_slimloader_descriptors::{AppImageDescriptor, BootableRegionDescriptors, ParseError};

use crate::{error, info};

extern "C" {
    static __bootable_region_descriptors_address: u32;
}

fn dump_descriptor_header(descriptor_header_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<BootableRegionDescriptors>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *descriptor_header_address.add(i) };
    }

    info!("Descriptor Header Bytes: {:x}", memory_copy);
}

fn dump_app_descriptor(app_descriptor_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<AppImageDescriptor>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *app_descriptor_address.add(i) };
    }

    info!("App Descriptor Bytes: {:x}", memory_copy);
}

pub fn load() -> BootableRegionDescriptors {
    let boot_descriptors_address = unsafe { &__bootable_region_descriptors_address as *const u32 };

    info!(
        "Bootloader: Fetching App Descriptors from {:X}.",
        boot_descriptors_address
    );

    // TODO error handling
    //      ? should the bootloader be responsible for re-formatting? This may be a security decision
    match BootableRegionDescriptors::from_address(boot_descriptors_address) {
        Ok(descriptors) => descriptors,
        Err(_e) => {
            error!(
                "Invalid boot region descriptors: ParseError |{}|",
                match _e {
                    ParseError::InvalidSignature => "Invalid Header Signature",
                    ParseError::InvalidAppSlot => "Invalid App Slot",
                    ParseError::InvalidSlotCount => "Invalid Slot Count",
                    _ => "CRC Error",
                }
            );

            match _e {
                ParseError::InvalidHeaderCrc { found, expected } => {
                    error!("Header CRC found = {:x}, expected = {:x}", found, expected);
                    dump_descriptor_header(boot_descriptors_address);
                }
                ParseError::InvalidAppCrc {
                    address,
                    found,
                    expected,
                } => {
                    error!("App @{:x} CRC found = {:x}, expected = {:x}", address, found, expected);
                    dump_app_descriptor(address);
                }
                _ => (),
            }

            loop {
                cortex_m::asm::wfi();
            }
        }
    }
}
