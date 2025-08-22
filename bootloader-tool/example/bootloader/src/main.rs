#![no_std]
#![no_main]

mod rom;

use cortex_m_rt::entry;
use defmt_rtt as _;

pub const MAX_IMAGE_SIZE: u32 = 0x80000;

#[entry]
fn main() -> ! {
    do_main().ok();

    defmt::warn!("Bootloader fell through main");

    loop {
        cortex_m::asm::wfe();
    }
}

fn do_main() -> Result<(), ()> {
    defmt::info!("Minimal bootloader example");

    // TODO set watchdog

    let rkth = &unsafe { *(0x401301E0 as *const [u8; 32]) };
    defmt::info!("Shadow RKTH: {:x}", rkth);

    if rkth[0] == 0x00 {
        defmt::warn!("Shadow RKTH not set, do not agitate the ROM bootloader further");
        return Ok(());
    }

    let image_container_ptr = 0x08020000 as *const u32;
    let target_ptr = 0x00090000 as *mut u32;

    let image_len = unsafe { *image_container_ptr.byte_add(0x20) };
    defmt::info!("Image size is 0x{:x}", image_len);
    if image_len > MAX_IMAGE_SIZE {
        defmt::panic!("Image too big");
    }

    // Before we check anything more, we need to make sure the external NOR flash is not man-in-the-middle'd.
    // We therefor copy everything to RAM.
    let target_slice = unsafe { core::slice::from_raw_parts_mut(target_ptr, image_len as usize) };
    let image_container_slice =
        unsafe { core::slice::from_raw_parts(image_container_ptr, image_len as usize) };

    target_slice.copy_from_slice(image_container_slice);
    defmt::info!("Copy done");

    let image_type = unsafe { *target_ptr.byte_add(0x24) };
    defmt::info!("Image type is 0x{:x}", image_type);
    defmt::assert_eq!(image_type & 0xff, 0x04); // Xip Signed.

    let load_addr = unsafe { *target_ptr.byte_add(0x34) };
    defmt::info!("Load address is 0x{:x}", load_addr);

    let verified = rom::skboot_authenticate(target_ptr);
    if let Err(e) = verified {
        defmt::error!("Failed to verify image: {}", e);
        return Err(());
    }
    defmt::info!("Verified image");

    // TODO set trustzone
    // TODO set MPU
    let vector_table_ptr = target_ptr;

    defmt::info!("Booting into application");
    unsafe {
        cortex_m::Peripherals::steal()
            .SCB
            .vtor
            .write(vector_table_ptr as u32);

        cortex_m::asm::bootload(vector_table_ptr)
    };
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {
        cortex_m::asm::wfe();
    }
}
