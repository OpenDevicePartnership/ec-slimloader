#[cfg(any(feature = "defmt", feature = "log"))]
macro_rules! jump_info {
    ($($arg:tt)*) => {
        defmt_or_log::info!($($arg)*);
    };
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! jump_info {
    ($($arg:tt)*) => {};
}

/// # Safety
///
/// `entry` must be a valid pointer to a loaded, authenticated image in flash.
pub unsafe fn jump_to_image(entry: *const u32) -> ! {
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

    let p = cortex_m::Peripherals::steal();
    p.SCB.vtor.write(entry as u32);

    // Load MSP/reset from the vector table and transfer control using the standard Cortex-M helper.
    jump_info!("jump: bootload to 0x{:08X}", entry as u32);
    cortex_m::asm::bootload(entry)
}
