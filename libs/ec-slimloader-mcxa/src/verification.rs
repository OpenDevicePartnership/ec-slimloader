#![allow(clippy::not_unsafe_ptr_arg_deref)]

use core::mem::ManuallyDrop;

use ec_slimloader::BootError;
use embassy_mcxa::rom::{
    NbootBoolValue, NbootImgAuthParms, NbootRootKeyRevocation, NbootRootKeyType, NbootRootKeyUsage, NbootRotAuthParms,
};
use embassy_mcxa::{Peri, peripherals};
use mcxa_ifr::{IFR, RotkEn, RotkUsageVal};
use mcxa_security_provisioning::certificate::derive_image_rkth_pair;

use crate::error::map_nboot_status_to_boot_error;

fn load_nboot_auth_parms_from_ifr(ifr: &IFR) -> Result<NbootImgAuthParms, BootError> {
    fn from_rotk_en(rotk_en: RotkEn) -> NbootRootKeyRevocation {
        match rotk_en {
            RotkEn::Enabled1 | RotkEn::Enabled2 => NbootRootKeyRevocation::Enabled,
            RotkEn::Revoked1 | RotkEn::Revoked2 => NbootRootKeyRevocation::Revoked,
        }
    }

    fn from_rotk_usage_val(val: RotkUsageVal) -> NbootRootKeyUsage {
        match val {
            RotkUsageVal::All => NbootRootKeyUsage::All,
            RotkUsageVal::DebugCaOnly => NbootRootKeyUsage::DebugCa,
            RotkUsageVal::ImageCaOnly => NbootRootKeyUsage::ImageCaFwCa,
            RotkUsageVal::DebugAndImage => NbootRootKeyUsage::DebugCaImageCaFwCa,
            RotkUsageVal::KeyOnly => NbootRootKeyUsage::ImageKeyFwKey,
            RotkUsageVal::BootImageKeyOnly => NbootRootKeyUsage::ImageKey,
            RotkUsageVal::FwUpdateImageKeyOnly => NbootRootKeyUsage::FwKey,
            RotkUsageVal::Unused => NbootRootKeyUsage::Unused,
        }
    }

    Ok(NbootImgAuthParms {
        soc_rot_nvm: NbootRotAuthParms {
            soc_root_key_revocation: [
                from_rotk_en(ifr.cfpa.rotk_revoke.rotk_en(0)),
                from_rotk_en(ifr.cfpa.rotk_revoke.rotk_en(1)),
                from_rotk_en(ifr.cfpa.rotk_revoke.rotk_en(2)),
                from_rotk_en(ifr.cfpa.rotk_revoke.rotk_en(3)),
            ],
            soc_image_key_revocation: ifr.cfpa.image_key_revoke,
            soc_rkh: ifr.cmpa.rotkh.degrade_original(),
            soc_rkh_1: ifr.cmpa.pqc_rotkh.degrade_original(),
            soc_number_of_root_keys: 4,
            soc_root_key_usage: [
                from_rotk_usage_val(ifr.cmpa.rotk_usage.rotk_usage(0)),
                from_rotk_usage_val(ifr.cmpa.rotk_usage.rotk_usage(1)),
                from_rotk_usage_val(ifr.cmpa.rotk_usage.rotk_usage(2)),
                from_rotk_usage_val(ifr.cmpa.rotk_usage.rotk_usage(3)),
            ],
            soc_root_key_type_and_length: NbootRootKeyType::EcdsaP384Mldsa87,
            soc_lifecycle: {
                let raw_val = ifr
                    .cfpa
                    .header
                    .cfpa_lc_state()
                    .map_err(|_| BootError::Markers)?
                    .raw_value() as u32;
                let raw_inv_val = ifr
                    .cfpa
                    .header
                    .inv_cfpa_lc_state()
                    .map_err(|_| BootError::Markers)?
                    .raw_value() as u32;
                (raw_inv_val << 16) | raw_val
            },
        },
        soc_trusted_firmware_version: ifr.cfpa.ee0_fw_version,
    })
}

pub fn verify_authenticity<'d>(
    mut peri: Peri<'d, peripherals::SGI0>,
    image_base: *const u8,
    dev_mode: bool,
) -> Result<bool, BootError> {
    let ifr = unsafe { mcxa_ifr::IFR::current() };

    let mut parms = load_nboot_auth_parms_from_ifr(ifr)?;

    defmt_or_log::trace!("Initializing NBOOT context");
    let mut n_boot_api = embassy_mcxa::rom::get().nboot().map_err(|_| BootError::Authenticate)?;

    protected_if::<{ LifeCycleState::Develop as u32 }, Result<(), BootError>, _, _>(
        || ifr.cfpa.header.cfpa_lc_state().map(|lc| lc as u32).unwrap_or_default(),
        || {
            // In dev mode, we're going to lie to nboot and derive the rkth's ourselves so they're always correct
            // This makes the provisioning process easier. Once no longer in dev mode, this branch is not taken and
            // only the flashed keys are used

            const MAX_FLASH_SLOT_SIZE: u32 = 2 * 1024 * 1024; // 2MB
            let image_header =
                unsafe { mcxa_security_provisioning::header::ImageHeader::from_ptr(image_base, MAX_FLASH_SLOT_SIZE) }
                    .map_err(|_| BootError::Integrity)?;

            // Parse AHAB container once and derive both RKTH values
            let (image_rkth, pqc_rkth) = derive_image_rkth_pair(
                peri.reborrow(),
                image_base,
                image_header.extended_header_offset(),
                image_header.image_length(),
            )
            .map_err(|_| BootError::Hash)?;

            parms.soc_rot_nvm.soc_rkh.copy_from_slice(&image_rkth.as_words());
            parms.soc_rot_nvm.soc_rkh_1.copy_from_slice(&pqc_rkth.as_words());

            Ok(())
        },
    )
    .transpose()?;

    defmt_or_log::trace!("begin auth");
    let status = n_boot_api
        .nboot_img_authenticate_romapi(image_base as u32, &mut parms)
        .map_err(map_nboot_status_to_boot_error)?;

    match status {
        NbootBoolValue::True => {
            defmt_or_log::info!("Hybrid Auth OK");
            Ok(true)
        }
        _ => Ok(false),
    }
}

unsafe extern "C" fn fn_once_trampoline<F: FnOnce() -> R, R>(ctx: *mut core::ffi::c_void, out: *mut core::ffi::c_void) {
    let returned = unsafe { ctx.cast::<F>().read()() };
    // Prevent drop here, we'll be dropping in the caller frame
    let returned = ManuallyDrop::new(returned);
    unsafe { out.cast::<ManuallyDrop<R>>().write(returned) };
}

#[must_use]
#[inline(always)]
#[cfg(feature = "mcxa5xx")]
fn protected_if<const TRUE: u32, R, E: FnOnce() -> u32, B: FnOnce() -> R>(expression: E, body: B) -> Option<R> {
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
            "mov r0, {exp}",
            "mov r1, {expv}",
            "bl {exp_tramp}",
            // The returned value is now in [bxed]. We must test if it's not 0
            // If it's not, we execute the body
            "movs r0, 1", // Clear the zero flag, so a skipped cmp cannot trigger the beq
            "mov r0, 0", // Clear the r0 value, so it cannot yet contain the true value
            "ldr r0, [{expv}]", // Get the expression value into r0
            "cmp r0, {true}", // Compare the loaded value with the 'true' value
            "beq 2f", // If the compare set the Z flag, we jump to the body. If the compare was skipped, Z is not set and we don't take the branch
            "b 3f", // If the beq was not taken or skipped, we branch to the exit
            "udf 0", // If the b was skipped, we crash the CPU

            "2:", // Execute the body
            "mov r0, {body}",
            "mov r1, {res}",
            "bl {body_tramp}",

            "3:", // Exit the asm block

            exp = in(reg) &mut expression as *mut _,
            body = in(reg) &mut body as *mut _,
            res = in(reg) &mut result as *mut _,
            expv = in(reg) &mut branch_taken as *mut _,
            true = const TRUE,
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

#[cfg(not(feature = "mcxa5xx"))]
fn protected_if<const TRUE: u32, R>(expression: impl FnOnce() -> u32, body: impl FnOnce() -> R) -> Option<R> {
    if expression() == TRUE { Some(body()) } else { None }
}
