use defmt_or_log::info;

/// Boot an application from memory.
///
/// It should follow the standard ARM Cortex M image format:
/// initial stack pointer, vector table, program data.
///
/// # SAFETY
/// The loaded application must be a valid firmware image for the platform,
/// and it must not return control to the caller.
pub unsafe fn boot_application(boot_address: *const u32) -> ! {
    unsafe {
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

        info!("Invalidating SCB icache, overwriting vector table and jumping to boot address");

        let mut p = cortex_m::Peripherals::steal();
        p.SCB.invalidate_icache();
        p.SCB.vtor.write(boot_address as u32);

        // Ensure that all previous steps have been executed.
        cortex_m::asm::dmb();
        cortex_m::asm::dsb();
        cortex_m::asm::isb();

        cortex_m::asm::bootload(boot_address)
    }
}
