use core::mem::ManuallyDrop;

pub fn protected_if<const TRUE: u32, R, E: FnOnce() -> u32, B: FnOnce() -> R>(
    expression: E,
    body: B,
) -> Result<R, u32> {
    use core::mem::{ManuallyDrop, MaybeUninit};

    // We're going to be handing the callbacks to the trampoline through a pointer
    // The drop will happen there, so we prevent it from dropping here using ManuallyDrop
    let mut expression = ManuallyDrop::new(expression);
    let mut body = ManuallyDrop::new(body);

    let mut result = MaybeUninit::uninit();
    let mut expression_value = !TRUE;

    unsafe {
        core::arch::asm!(
            // Make sure our expression value is false
            "movw r0, #:lower16:{false}", // Clear the r0 value, so it cannot yet contain the true value
            "movt r0, #:upper16:{false}",
            "str r0, [{expv}]",

            // We need to evaluate `exp_tramp`, prepare the function call
            "mov r0, {exp}", // Load arg 0, pointer to the fnonce
            "mov r1, {expv}", // Load arg 1, pointer to the out parameter
            "bl {exp_tramp}", // Call the trampoline, which will execute th fnonce
            
            // The returned value is now in [expv]. We must test if it's not 0
            // If it's not, we execute the body
            "movs r0, 1", // Clear the zero flag, so a skipped cmp cannot trigger the beq
            "movw r0, #:lower16:{false}", // Clear the r0 value, so it cannot yet contain the true value
            "movt r0, #:upper16:{false}",
            "ldr r0, [{expv}]", // Get the expression value into r0

            "movw r3, #:lower16:{true}", // Load the true value
            "movt r3, #:upper16:{true}",
            "cmp r0, r3", // Compare the loaded value with the loaded 'true' value
            
            "beq 2f", // If the compare set the Z flag, we jump to the body. If the compare was skipped, Z is not set and we don't take the branch
            "b 3f", // If the beq was not taken or skipped, we branch to the exit
            "udf 0", // If the b was skipped, we crash the CPU

            "2:", // Execute the body
            "mov r0, {body}", // Load arg 0, pointer to the fnonce
            "mov r1, {res}", // Load arg 1, pointer to the out parameter
            "bl {body_tramp}", // Call the trampoline, which will execute th fnonce

            "3:", // Exit the asm block

            exp = in(reg) &mut expression as *mut _,
            expv = in(reg) &mut expression_value as *mut _,
            body = in(reg) &mut body as *mut _,
            res = in(reg) &mut result as *mut _,
            true = const TRUE,
            false = const !TRUE,
            in("r0") 0, // Reserve so we don't have to manually save named registers
            in("r1") 0, // Reserve so we don't have to manually save named registers
            in("r3") 0, // Reserve so we don't have to manually save named registers
            in("r4") 0, // Reserve so we don't have to manually save named registers
            in("r12") 0, // Reserve so we don't have to manually save named registers
            exp_tramp = sym fn_once_trampoline::<E, u32>,
            body_tramp = sym fn_once_trampoline::<B, R>,
            clobber_abi("C"),
        )
    }

    if expression_value == TRUE {
        Ok(unsafe { result.assume_init() })
    } else {
        // We did not run the body, so we should drop it here
        unsafe { ManuallyDrop::drop(&mut body) };
        Err(expression_value)
    }
}

unsafe extern "C" fn fn_once_trampoline<F: FnOnce() -> R, R>(ctx: *mut core::ffi::c_void, out: *mut core::ffi::c_void) {
    let callback = unsafe { ctx.cast::<F>().read() };

    // Running the callback also consumes (and thus drops) it
    let returned = callback();

    // Prevent drop here, we'll be dropping in the caller frame
    let returned = ManuallyDrop::new(returned);
    unsafe { out.cast::<ManuallyDrop<R>>().write(returned) };
}
