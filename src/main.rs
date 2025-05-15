#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;
use panic_probe as _;

#[cfg(feature = "imxrt")]
mod imxrt;

#[cfg(feature = "imxrt")]
use imxrt::{raw_copy_to_ram, validate_crc};

#[cfg(not(feature = "imxrt"))]
mod unsupported {
    use super::*;

    pub unsafe fn raw_copy_to_ram(_from: *const u32, _to: *mut u32, _len_words: usize) {}
    pub fn validate_crc(_app_descriptor: &AppImageDescriptor) -> bool {
        true
    }
}
#[cfg(not(feature = "imxrt"))]
use unsupported::*;

fn copy_image(_app_descriptor: &AppImageDescriptor) {
    // TODO allow other scenarios supported from bootloader aside from copy to RAM
    unsafe {
        raw_copy_to_ram(
            _app_descriptor.stored_address as *const u32,
            _app_descriptor.execution_address as *mut u32,
            _app_descriptor.execution_copy_size_bytes as usize / core::mem::size_of::<u32>(),
        );
    }
}

#[cfg(feature = "defmt")]
fn dump_descriptor_header(descriptor_header_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<BootableRegionDescriptorHeader>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *descriptor_header_address.add(i) };
    }

    defmt::info!("Descriptor Header Bytes: {:x}", memory_copy);
}

#[cfg(feature = "defmt")]
fn dump_app_descriptor(app_descriptor_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<AppImageDescriptor>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *app_descriptor_address.add(i) };
    }

    defmt::info!("App Descriptor Bytes: {:x}", memory_copy);
}

use ec_slimloader_descriptors::*;

extern "C" {
    static __bootable_region_descriptors_address: u32;
}

#[cortex_m_rt::pre_init]
unsafe fn pre_init() {
    cortex_m::interrupt::disable();
}

fn copy_and_execute_image(app_descriptor: &AppImageDescriptor) -> ! {
    // note that use of this flag requires proper app image address backing for .text section-- ensure you have linked the app against RAM addresses! (Or other required adjustments)
    if app_descriptor.flags & APP_IMAGE_FLAG_COPY_TO_EXECUTION_ADDRESS != 0 {
        #[cfg(feature = "defmt")]
        defmt::info!("Bootloader: Performing image copy to execution location.");

        copy_image(&app_descriptor);
    }

    #[cfg(feature = "defmt")]
    {
        let xaddr = app_descriptor.execution_address;
        defmt::info!(
            "Bootloader: Validation complete and branching to application execution address {:x}.",
            xaddr
        );
    }

    // branch to location as described by descriptor
    unsafe {
        // set the VTOR
        cortex_m::Peripherals::steal()
            .SCB
            .vtor
            .write(app_descriptor.execution_address);

        // enable interrupts
        cortex_m::interrupt::enable();

        // perform the branch
        // NOTE: two separate steps are used here for easier debug inspection
        let p_sp = app_descriptor.execution_address as *const u32;
        let p_rv = (app_descriptor.execution_address as *const u32).add(1);

        let sp = *p_sp;
        let rv = *p_rv;

        cortex_m::asm::bootstrap(sp as *const u32, rv as *const u32);
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    #[cfg(feature = "defmt")]
    defmt::info!("Bootloader: Initializing Hardware.");

    let boot_descriptors_address = unsafe { &__bootable_region_descriptors_address as *const u32 };

    #[cfg(feature = "defmt")]
    defmt::info!(
        "Bootloader: Fetching App Descriptors from {:X}.",
        boot_descriptors_address
    );

    // TODO error handling
    //      ? should the bootloader be responsible for re-formatting? This may be a security decision
    let descriptors = match BootableRegionDescriptors::from_address(boot_descriptors_address) {
        Ok(descriptors) => descriptors,
        Err(_e) => {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "Invalid boot region descriptors: ParseError |{}|",
                match _e {
                    ParseError::InvalidSignature => "Invalid Header Signature",
                    ParseError::InvalidAppSlot => "Invalid App Slot",
                    ParseError::InvalidSlotCount => "Invalid Slot Count",
                    _ => "CRC Error",
                }
            );

            #[cfg(feature = "defmt")]
            match _e {
                ParseError::InvalidHeaderCrc { found, expected } => {
                    defmt::error!("Header CRC found = {:x}, expected = {:x}", found, expected);
                    dump_descriptor_header(boot_descriptors_address);
                }
                ParseError::InvalidAppCrc {
                    address,
                    found,
                    expected,
                } => {
                    defmt::error!("App @{:x} CRC found = {:x}, expected = {:x}", address, found, expected);
                    dump_app_descriptor(address);
                }
                _ => (),
            }

            loop {
                cortex_m::asm::wfi();
            }
        }
    };

    let active_app_descriptor = descriptors.get_active_slot();

    #[cfg(feature = "defmt")]
    {
        let slot = active_app_descriptor.app_slot_number;
        defmt::info!("Bootloader: Active App slot is {}", slot);
    }

    let boot_image = if active_app_descriptor.flags & APP_IMAGE_FLAG_SKIP_IMAGE_CRC_CHECK != 0 {
        #[cfg(feature = "defmt")]
        defmt::info!("Bootloader: skipping image CRC due to SKIP_IMAGE_CRC_CHECK flag.");

        true
    } else {
        #[cfg(feature = "defmt")]
        defmt::info!("Bootloader: validating image CRC.");

        validate_crc(&active_app_descriptor)
    };

    // validate integrity of app image
    if boot_image {
        copy_and_execute_image(&active_app_descriptor);
    } else {
        // Attempt to boot the backup image
        #[cfg(feature = "defmt")]
        defmt::error!("Active App CRC checksum failure!");

        let backup_slot = active_app_descriptor.app_slot_number ^ 1;
        if let Ok(backup_descriptor) = descriptors.get_app_at_slot(backup_slot) {
            #[cfg(feature = "defmt")]
            defmt::info!("Bootloader: Backup App slot is {}", backup_slot);

            let backup_boot_image = if backup_descriptor.flags & APP_IMAGE_FLAG_SKIP_IMAGE_CRC_CHECK != 0 {
                #[cfg(feature = "defmt")]
                defmt::info!("Bootloader: skipping backup image CRC due to SKIP_IMAGE_CRC_CHECK flag.");

                true
            } else {
                #[cfg(feature = "defmt")]
                defmt::info!("Bootloader: validating image CRC.");

                validate_crc(&backup_descriptor)
            };

            if backup_boot_image {
                #[cfg(feature = "defmt")]
                defmt::info!("Bootloader: Backup App CRC checksum success!");

                copy_and_execute_image(&backup_descriptor);
            } else {
                #[cfg(feature = "defmt")]
                defmt::error!("Backup App CRC checksum failure!");
            }
        }

        loop {
            cortex_m::asm::wfi();
        }
    }
}
