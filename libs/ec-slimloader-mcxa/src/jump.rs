#[cfg(any(feature = "defmt", feature = "log"))]
macro_rules! jump_error {
    ($($arg:tt)*) => {
        defmt_or_log::error!($($arg)*);
    };
}

#[cfg(any(feature = "defmt", feature = "log"))]
macro_rules! jump_info {
    ($($arg:tt)*) => {
        defmt_or_log::info!($($arg)*);
    };
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! jump_error {
    ($($arg:tt)*) => {};
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! jump_info {
    ($($arg:tt)*) => {};
}

/// # Safety
///
/// `entry` must be a valid pointer to a loaded, authenticated image in flash.
pub unsafe fn jump_to_image(entry: *const u32) -> ! {
    // Guards: validate image header fields (Table 204 Nx4x security reference manual)

    let entry_bytes = entry as *const u8;
    let image_len = *(entry_bytes.add(0x20) as *const u32);
    let cert_off = *(entry_bytes.add(0x28) as *const u32);

    // Basic sanity: image length should be at least a vector table (>= 0x40),
    // cert header offset must be 4-byte aligned and within image length.
    if image_len < 0x40 || (cert_off & 0x3) != 0 || cert_off >= image_len {
        // Invalid header; halt to avoid jumping to a potentially corrupt image.
        jump_error!(
            "jump: invalid header image_len=0x{:X}, cert_off=0x{:X}",
            image_len,
            cert_off
        );
        loop {
            core::hint::spin_loop()
        }
    }

    jump_info!(
        "jump: handoff entry=0x{:08X} image_len=0x{:X} cert_off=0x{:X}",
        entry as u32,
        image_len,
        cert_off
    );

    // The following code is replicated from IMXRT bootloader.
    // Disable interrupts globally while we reset the NVIC.
    cortex_m::interrupt::disable();

    let nvic = &*cortex_m::peripheral::NVIC::PTR;

    // Disable all configurable interrupts.
    for clear_enable in &nvic.icer {
        clear_enable.write(u32::MAX);
    }

    // Clear all interrupt-pending bits.
    for clear_pending in &nvic.icpr {
        clear_pending.write(u32::MAX);
    }

    // Reset all interrupt priorities.
    for priority in &nvic.ipr {
        priority.write(0);
    }

    // Re-enable interrupts globally to match boot-up environment.
    cortex_m::interrupt::enable();

    let mut p = cortex_m::Peripherals::steal();
    p.SCB.invalidate_icache();
    p.SCB.vtor.write(entry as u32);

    // Ensure that all previous steps have been executed.
    cortex_m::asm::dmb();
    cortex_m::asm::dsb();
    cortex_m::asm::isb();

    // Load MSP/reset from the vector table and transfer control using the standard Cortex-M helper.
    jump_info!("jump: bootload to 0x{:08X}", entry as u32);
    cortex_m::asm::bootload(entry)
}
