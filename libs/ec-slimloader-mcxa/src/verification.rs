use crate::certificate::derive_image_rkth_pair;
use embassy_mcxa::{peripherals, Peri};

use crate::lifecycle::{
    cnsa_enforced, fast_boot_enabled, load_firmware_version_from_cfpa, load_image_key_revocation_from_cfpa,
    load_lifecycle_from_cfpa, load_pqc_rotkh_from_cmpa, load_root_key_revocation_from_cfpa, load_rotk_usage_from_cmpa,
    load_rotkh_from_cmpa, low_power_authentication_enforced, secure_boot_state, SecureBootState,
};
use crate::rom_api::{
    nboot, nboot_bool_is_true, NbootBool, NbootBoolValue, NbootCtx, NbootImgAuthParms, NbootLifecycleState,
    NbootRootKeyRevocation, NbootRootKeyType, NbootRootKeyUsage, NbootRotAuthParms,
};

macro_rules! verify_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "verification-logging")]
        {
            defmt_or_log::info!($($arg)*);
        }
    };
}

macro_rules! verify_trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "verification-logging")]
        {
            defmt_or_log::trace!($($arg)*);
        }
    };
}

macro_rules! verify_warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "verification-logging")]
        {
            defmt_or_log::warn!($($arg)*);
        }
    };
}

macro_rules! verify_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "verification-logging")]
        {
            defmt_or_log::error!($($arg)*);
        }
    };
}

fn is_dev_mode(secure_boot_state: SecureBootState) -> bool {
    matches!(secure_boot_state, SecureBootState::Disabled)
        && matches!(load_lifecycle_from_cfpa(), Some(NbootLifecycleState::Develop))
}

/// Verify the authenticity of the image at the given base address using the NBOOT ROM API. This includes initializing the NBOOT context, loading lifecycle and root of trust information from CFPA/CMPA,
/// deriving the image RKTH from the AHAB container, and calling nboot_img_authenticate_romapi. Returns Ok(()) if authentication is successful, or an appropriate BootError if any step fails or if authentication fails.
/// Will ONLY authenticate if CMPA secure boot settings is configured correctly, correct key set (as established by the ROTKH values) is used for signing, and the image is properly signed as an HYBRID (ECDSA + ML-DSA) image.
/// In dev mode, if the RKTH derived from the image does not match the ROTKH in CMPA, it will be copied to the ROTKH to allow authentication to proceed (this allows flexibility in dev mode since keys may not be provisioned yet),
/// but in production mode, a mismatch will cause authentication to fail (to prevent unauthorized images from being authenticated).
pub fn verify_authenticity<'d>(
    mut peri: Peri<'d, peripherals::SGI0>,
    image_base: *const u8,
) -> Result<(), ec_slimloader::BootError> {
    let n_boot_api = nboot();
    let mut ctx: NbootCtx = unsafe { core::mem::zeroed() };
    let mut sig_ok: NbootBool = NbootBoolValue::False as u32;

    verify_trace!("Initializing NBOOT context");
    let context_init_status = n_boot_api.nboot_context_init(&mut ctx);
    if context_init_status != crate::error::NbootStatus::Success {
        return Err(ec_slimloader::BootError::Authenticate);
    }

    let mut parms = NbootImgAuthParms {
        soc_RoTNVM: NbootRotAuthParms {
            soc_rootKeyRevocation: [
                NbootRootKeyRevocation::Revoked as u32,
                NbootRootKeyRevocation::Revoked as u32,
                NbootRootKeyRevocation::Revoked as u32,
                NbootRootKeyRevocation::Revoked as u32,
                //Start as revoked by default for safety; will be updated with real values from CFPA if read is successful. 
                // This way if CFPA read fails for some reason, we won't accidentally treat revoked keys as valid.
            ],
            soc_imageKeyRevocation: 0, //Image key revoocation use case: None?
            soc_rkh: [0; 12],
            soc_rkh_1: [0; 12],      // PQC hash for hybrid keys
            soc_numberOfRootKeys: 4, // TODO: Must equal 4 per NXP example code.
            soc_rootKeyUsage: [
                NbootRootKeyUsage::Unused as u32,
                NbootRootKeyUsage::Unused as u32,
                NbootRootKeyUsage::Unused as u32,
                NbootRootKeyUsage::Unused as u32,
                // Start as unused by default for safety; will be updated with real values from CMPA if read is successful.
            ],
            soc_rootKeyTypeAndLength: NbootRootKeyType::EcdsaP384Mldsa87 as u32, //FIXED TO THIS because we are CNSA 2.0 compliant.
            soc_lifecycle: NbootLifecycleState::InField.nboot_soc_lifecycle(), // default to INFIELD (strict start), gets updated with real one further below.
        },
        soc_trustedFirmwareVersion: 0xFFFF_FFFF, // default to max version to be safe (any real version should be lower), gets updated with real one from CFPA further below
    };

    if let Some(cmpa_rotkh) = load_rotkh_from_cmpa() {
        parms.soc_RoTNVM.soc_rkh = cmpa_rotkh;
        verify_trace!("RKTH loaded from CMPA");
    } else {
        verify_warn!("CMPA ROTKH read failed");
        return Err(ec_slimloader::BootError::RootOfTrust);
    }

    // Load PQC ROTKH for hybrid keys
    if let Some(cmpa_pqc_rotkh) = load_pqc_rotkh_from_cmpa() {
        parms.soc_RoTNVM.soc_rkh_1 = cmpa_pqc_rotkh;
        verify_trace!("PQC RKTH loaded from CMPA");
    } else {
        verify_warn!("CMPA PQC ROTKH read failed");
        return Err(ec_slimloader::BootError::RootOfTrust);
    }

    //Load additional lifecycle state from CFPA/CMPA
    if let Some(cfpa_img_key_revocation) = load_image_key_revocation_from_cfpa() {
        parms.soc_RoTNVM.soc_imageKeyRevocation = cfpa_img_key_revocation;
    }

    if let Some(cfpa_root_key_revocation) = load_root_key_revocation_from_cfpa() {
        parms.soc_RoTNVM.soc_rootKeyRevocation = cfpa_root_key_revocation.map(|r| r as u32);
    }

    if let Some(cfpa_fw_version) = load_firmware_version_from_cfpa() {
        parms.soc_trustedFirmwareVersion = cfpa_fw_version;
    }

    if let Some(cmpa_root_key_usage) = load_rotk_usage_from_cmpa() {
        parms.soc_RoTNVM.soc_rootKeyUsage = cmpa_root_key_usage.map(|u| u as u32);
    }

    if let Some(cfpa_lifecycle) = load_lifecycle_from_cfpa() {
        parms.soc_RoTNVM.soc_lifecycle = cfpa_lifecycle.nboot_soc_lifecycle();
    }

    let secure_boot_state = secure_boot_state();
    if matches!(secure_boot_state, SecureBootState::Unknown) {
        verify_error!("Secure boot state could not be validated");
        n_boot_api.nboot_context_deinit(&mut ctx);
        return Err(ec_slimloader::BootError::Integrity);
    }

    let dev_mode = is_dev_mode(secure_boot_state);

    if !dev_mode {
        if !matches!(secure_boot_state, SecureBootState::HybridEnforced)
            || !cnsa_enforced()
            || fast_boot_enabled()
            || !low_power_authentication_enforced()
        {
            verify_error!(
                "Secure Boot policy violation: secure boot state={:?}, CNSA enforced={}, fast boot enabled={}, low power auth enforced={}",
                secure_boot_state,
                cnsa_enforced(),
                fast_boot_enabled(),
                low_power_authentication_enforced()
            );
            n_boot_api.nboot_context_deinit(&mut ctx);
            return Err(ec_slimloader::BootError::Integrity);
        }
    }

    const MAX_FLASH_SLOT_SIZE: u32 = 2 * 1024 * 1024; // 2MB, TODO: make this configurable or derive from flash size
    let image_header = unsafe { crate::header::ImageHeader::from_ptr(image_base, MAX_FLASH_SLOT_SIZE) }
        .map_err(|_| ec_slimloader::BootError::Integrity)?;
    // Parse AHAB container once and derive both RKTH values
    let (image_rkth, pqc_rkth) = derive_image_rkth_pair(
        peri.reborrow(),
        image_base,
        image_header.extended_header_offset(),
        image_header.image_length(),
    );

    // Process ECDSA RKTH (traditional)
    if let Some(image_rkth) = image_rkth {
        let image_rkth_words = image_rkth.as_le_words();

        verify_info!("Derived image RKTH: {:x}", image_rkth_words);
        if image_rkth_words != parms.soc_RoTNVM.soc_rkh { // non-const time is okay, these are public key hashes.
            if dev_mode {
                verify_warn!("Dev mode: copying from image RKTH");
                parms.soc_RoTNVM.soc_rkh.copy_from_slice(&image_rkth_words);
            } else {
                verify_warn!("Production: image RKTH differs; not copying, will call ecdsa_verify anyway");
                n_boot_api.nboot_context_deinit(&mut ctx);
                return Err(ec_slimloader::BootError::RootOfTrust);
            }
        } else {
            verify_trace!("RKTH match");
        }
    } else {
        verify_warn!("Failed to derive image RKTH");
        n_boot_api.nboot_context_deinit(&mut ctx);
        return Err(ec_slimloader::BootError::RootOfTrust);
    }

    // Process PQC RKTH (ML-DSA) for hybrid keys
    if let Some(pqc_rkth) = pqc_rkth {
        let pqc_rkth_words = pqc_rkth.as_le_words();

        verify_info!("Derived image PQC RKTH: {:x}", pqc_rkth_words);
        if pqc_rkth_words != parms.soc_RoTNVM.soc_rkh_1 { //non-const time comparison is okay, these are public key hashes
            if dev_mode {
                verify_warn!("Dev mode: copying from image PQC RKTH");
                parms.soc_RoTNVM.soc_rkh_1.copy_from_slice(&pqc_rkth_words);
            } else {
                verify_warn!("Production: image PQC RKTH differs; not copying");
                //TODO: just return Err() here?
                n_boot_api.nboot_context_deinit(&mut ctx);
                return Err(ec_slimloader::BootError::RootOfTrust);
            }
        } else {
            verify_trace!("PQC RKTH match");
        }
    } else {
        verify_warn!("Failed to derive image PQC RKTH (ML-DSA not found or error)");
        n_boot_api.nboot_context_deinit(&mut ctx);
        return Err(ec_slimloader::BootError::RootOfTrust);
    }
    verify_trace!("begin auth");
    let status = n_boot_api.nboot_img_authenticate_romapi(&mut ctx, image_base, &mut sig_ok, &mut parms);

    for w in parms.soc_RoTNVM.soc_rkh.iter_mut() {
        *w = 0;
    }
    for w in parms.soc_RoTNVM.soc_rkh_1.iter_mut() {
        *w = 0;
    }
    for w in parms.soc_RoTNVM.soc_rootKeyRevocation.iter_mut() {
        *w = 0;
    }

    n_boot_api.nboot_context_deinit(&mut ctx);
    //TODO: does de-init zeroize the context or do we need to do that manually for security?

    match (status, sig_ok) {
        (crate::error::NbootStatus::Success, s) if nboot_bool_is_true(s) => {
            verify_info!("Hybrid Auth OK");
            Ok(())
        }
        (status, _) => {
            let boot_error = crate::error::map_nboot_status_to_boot_error(status);

            verify_error!("Auth failed with status {:?}: {:?}", status, boot_error);
            Err(boot_error)
        }
    }
}
