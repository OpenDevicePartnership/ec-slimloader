#![allow(clippy::not_unsafe_ptr_arg_deref)]

use ec_slimloader::BootError;
use embassy_mcxa::rom::{
    NbootBoolValue, NbootImgAuthParms, NbootRootKeyRevocation, NbootRootKeyType, NbootRootKeyUsage, NbootRotAuthParms,
};
use embassy_mcxa::{Peri, peripherals};
use fault_rejection::protected_if;
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
) -> Result<NbootBoolValue, BootError> {
    let ifr = unsafe { mcxa_ifr::IFR::current() };

    let mut parms = load_nboot_auth_parms_from_ifr(ifr)?;

    defmt_or_log::trace!("Initializing NBOOT context");
    let mut n_boot_api = embassy_mcxa::rom::get().nboot().map_err(|_| BootError::Authenticate)?;

    let result = protected_if::<{ true as u32 }, Result<(), BootError>, _, _>(
        || dev_mode as u32,
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
    );

    match result {
        Ok(if_result) => if_result?,
        Err(_) => {
            // We did not take the if branch. Nothing to do
        }
    }

    defmt_or_log::trace!("begin auth");
    let status = n_boot_api
        .nboot_img_authenticate_romapi(image_base as u32, &mut parms)
        .map_err(map_nboot_status_to_boot_error)?;

    match status {
        NbootBoolValue::True => {
            defmt_or_log::info!("Hybrid Auth OK");
            Ok(status)
        }
        _ => Ok(status),
    }
}
