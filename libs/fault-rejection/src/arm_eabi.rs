use core::mem::ManuallyDrop;

#[must_use]
#[inline(always)]
pub fn protected_if<const TRUE: u32, R, E: FnOnce() -> u32, B: FnOnce() -> R>(expression: E, body: B) -> Option<R> {
    use core::mem::{ManuallyDrop, MaybeUninit};

    // We're going to be handing the callbacks to the trampoline through a pointer
    // The drop will happen there, so we prevent it from dropping here using ManuallyDrop
    let mut expression = ManuallyDrop::new(expression);
    let mut body = ManuallyDrop::new(body);

    let mut result = MaybeUninit::uninit();
    let mut branch_taken = 0;

    unsafe {
        core::arch::asm!(
            // We need to evaluate `exp_tramp`, prepare the function call
            "mov r0, {exp}", // Load arg 0, pointer to the fnonce
            "mov r1, {expv}", // Load arg 1, pointer to the out parameter
            "bl {exp_tramp}", // Call the trampoline, which will execute th fnonce
            // The returned value is now in [bxed]. We must test if it's not 0
            // If it's not, we execute the body
            "movs r0, 1", // Clear the zero flag, so a skipped cmp cannot trigger the beq
            "mov r0, {false}", // Clear the r0 value, so it cannot yet contain the true value
            "ldr r0, [{expv}]", // Get the expression value into r0
            "cmp r0, {true}", // Compare the loaded value with the 'true' value
            "beq 2f", // If the compare set the Z flag, we jump to the body. If the compare was skipped, Z is not set and we don't take the branch
            "b 3f", // If the beq was not taken or skipped, we branch to the exit
            "udf 0", // If the b was skipped, we crash the CPU

            "2:", // Execute the body
            "mov r0, {body}", // Load arg 0, pointer to the fnonce
            "mov r1, {res}", // Load arg 1, pointer to the out parameter
            "bl {body_tramp}", // Call the trampoline, which will execute th fnonce

            "3:", // Exit the asm block

            exp = in(reg) &mut expression as *mut _,
            body = in(reg) &mut body as *mut _,
            res = in(reg) &mut result as *mut _,
            expv = in(reg) &mut branch_taken as *mut _,
            true = const TRUE,
            false = const !TRUE,
            out("r0") _,
            out("r1") _,
            exp_tramp = sym fn_once_trampoline::<E, u32>,
            body_tramp = sym fn_once_trampoline::<B, R>,
            clobber_abi("C"),
        )
    }

    if branch_taken != 0 {
        Some(unsafe { result.assume_init() })
    } else {
        None
    }
}

unsafe extern "C" fn fn_once_trampoline<F: FnOnce() -> R, R>(ctx: *mut core::ffi::c_void, out: *mut core::ffi::c_void) {
    let returned = unsafe { ctx.cast::<F>().read()() };
    // Prevent drop here, we'll be dropping in the caller frame
    let returned = ManuallyDrop::new(returned);
    unsafe { out.cast::<ManuallyDrop<R>>().write(returned) };
}
