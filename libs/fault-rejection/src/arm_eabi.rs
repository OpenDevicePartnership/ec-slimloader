use core::mem::ManuallyDrop;

/// A very convoluted if statement.
///
/// It consists of 3 parts:
/// - The expression callback
/// - The `TRUE` const
/// - The body callback
///
/// The expression will be evaluated. If it returns the same value as `TRUE`, the body will be called.
///
/// ## Claims
///
/// The goal is to harden against fault injections. Specifically, this function protects against single(!) instruction skips.
/// If any instruction is skipped, either the body is not executed or a `udf` instruction is executed instead.
///
/// This function does not protect against saturation faults (where a register is set to all-0 or all-1).
/// The `TRUE` value should be chosen so it's not all-0 or all-1 to mitigate this type of fault injection somewhat.
/// If that's the case, a saturation can in practise only affect some pointers which will cause UB when it's all-0 or a memfault when it's all-1.
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
            "str r0, [r5]",

            // We need to evaluate `exp_tramp`, prepare the function call
            "mov r0, r4", // Load arg 0, pointer to the fnonce
            "mov r1, r5", // Load arg 1, pointer to the out parameter
            "bl {exp_tramp}", // Call the trampoline, which will execute th fnonce

            // The returned value is now in [expv]. We must test if it's not 0
            // If it's not, we execute the body
            "movs r0, 1", // Clear the zero flag, so a skipped cmp cannot trigger the beq
            "movw r0, #:lower16:{false}", // Clear the r0 value, so it cannot yet contain the true value
            "movt r0, #:upper16:{false}",
            "ldr r0, [r5]", // Get the expression value into r0

            "movw r1, #:lower16:{true}", // Load the true value
            "movt r1, #:upper16:{true}",
            "cmp r0, r1", // Compare the loaded value with the loaded 'true' value

            "beq 2f", // If the compare set the Z flag, we jump to the body. If the compare was skipped, Z is not set and we don't take the branch
            "b 3f", // If the beq was not taken or skipped, we branch to the exit
            "udf 0", // If the b was skipped, we crash the CPU

            "2:", // Execute the body
            "mov r0, r10", // Load arg 0, pointer to the fnonce
            "mov r1, r11", // Load arg 1, pointer to the out parameter
            "bl {body_tramp}", // Call the trampoline, which will execute th fnonce

            "3:", // Exit the asm block

            in("r4") &mut expression,
            in("r5") &mut expression_value,
            in("r10") &mut body,
            in("r11") &mut result,
            true = const TRUE,
            false = const !TRUE,
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
