use core::arch::asm;
use cortex_m::asm::{dsb, isb};

#[cfg(any(feature = "defmt", feature = "log"))]
macro_rules! jump_error {
    ($($arg:tt)*) => {
        defmt_or_log::error!($($arg)*);
    };
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
macro_rules! jump_error {
    ($($arg:tt)*) => {};
}

pub unsafe fn jump_to_image(entry: u32) -> ! {
    // entry points to vector table base
    let initial_sp = *(entry as *const u32);
    let reset = *((entry + 4) as *const u32);
    // info!(
    //     "jump: entry=0x{:08X}, initial_sp=0x{:08X}, reset=0x{:08X}",
    //     entry, initial_sp, reset
    // );

    // Guards: validate image header fields (Table 204 Nx4x security reference manual)

    let image_len = *((entry + 0x20) as *const u32);
    let cert_off = *((entry + 0x28) as *const u32);

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
    // (similar to imxrt): disable interrupts & timer
    // Disable all maskable interrupts
    // info!("jump: disabling interrupts");
    #[cfg(target_arch = "arm")]
    asm!("cpsid i", options(nostack, preserves_flags));

    // Disable SysTick (if previously configured by loader)
    const SYST_CSR: *mut u32 = 0xE000E010 as *mut u32; // SysTick Control and Status Register
    core::ptr::write_volatile(SYST_CSR, 0);
    // info!("jump: SysTick disabled");

    // Disable NVIC interrupts & clear any pending bits (MCXN556s up to IRQ 155 → 5 * 32 blocks)
    const NVIC_ICER_BASE: *mut u32 = 0xE000E180 as *mut u32; // Interrupt Clear/Enable Registers
    const NVIC_ICPR_BASE: *mut u32 = 0xE000E280 as *mut u32; // Interrupt Clear/Pending Registers
    for i in 0..5 {
        core::ptr::write_volatile(NVIC_ICER_BASE.add(i), 0xFFFF_FFFF);
        core::ptr::write_volatile(NVIC_ICPR_BASE.add(i), 0xFFFF_FFFF);
    }
    // info!("jump: NVIC interrupts disabled & pending cleared");

    // Clear selected system handler pending bits (SecureFault, PendSV) in SHCSR
    const SCB_SHCSR: *mut u32 = 0xE000ED24 as *mut u32; // System Handler Control and State Register
                                                        // Write-1-to-clear for pending bits is not supported; instead, clear enable bits to avoid servicing.
                                                        // Ensure SVC/Debug/PendSV/SysTick not enabled by loader.
    core::ptr::write_volatile(SCB_SHCSR, 0);
    // info!("jump: SHCSR cleared");

    // Ensure privileged thread mode & use MSP (clear CONTROL.nPRIV & CONTROL.SPSEL)
    #[cfg(target_arch = "arm")]
    asm!("msr CONTROL, {0}", in(reg) 0u32, options(nostack, preserves_flags));
    #[cfg(target_arch = "arm")]
    asm!("isb", options(nostack, preserves_flags));

    // Set MSP to application's initial stack pointer
    #[cfg(target_arch = "arm")]
    asm!("msr MSP, {0}", in(reg) initial_sp, options(nostack, preserves_flags));
    // info!("jump: MSP set to 0x{:08X}", initial_sp);

    // Set VTOR to application's vector table
    const SCB_VTOR: *mut u32 = 0xE000ED08 as *mut u32;
    core::ptr::write_volatile(SCB_VTOR, entry);
    // info!("jump: VTOR set to 0x{:08X}", entry);

    // Data / instruction sync barriers before branch
    dsb();
    isb();

    // Branch to application's reset handler
    let reset_fn: extern "C" fn() = core::mem::transmute(reset as usize);
    // info!("jump: branching to reset handler 0x{:08X}", reset);
    reset_fn();
    loop {} // should not return
}
