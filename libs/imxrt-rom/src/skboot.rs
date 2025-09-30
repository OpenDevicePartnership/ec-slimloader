//! Interface to the skboot ROM function 'skboot_authenticate'.

use core::ptr::{null, null_mut};

use defmt_or_log::error;
#[cfg(feature = "rt")]
use embassy_imxrt::pac::interrupt;

use crate::api::{api_table, BootStatus, KbAuthenticate, KbOperation, KbOptions, KbSettings, KbStatus, SecureBool};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AuthenticateError {
    /// Failed to verify signature.
    SignUnverified,
    /// Failed to verify signature with unknown error.
    SignUnknown,
    /// Failed to authenticate image when parsing certificate header, certificate chain RKH or signature verification fails.
    Fail,
    /// Found an unexpected value in image.
    UnexpectedValueInImage,
    /// The keystore marker on the image is invalid.
    KeyStoreMarkerInvalid,
    /// The function passed an undefined return value.
    BootStatusUnknown,
    /// The function passed an undefined value as `is_sign_verified`` value.
    IsSignVerifiedUnknown,
}

/// Perform ROM authentication of an image.
///
/// If RHK is provided it will use that hash to verify the certificate chain instead.
#[allow(dead_code)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn skboot_authenticate(
    start: *const u32,
    max_image_length: u32,
    rhk: Option<[u8; 32]>,
) -> Result<(), AuthenticateError> {
    // Note:
    // The ROM reserved space for global variables in RAM on this device is:
    // 0x1001_2000 to 0x1000_A000

    // 43.9 Secure ROM API page 1282 of RT6xx User manual

    let mut session_ref = null_mut();
    let mut user_buf = [0u32; 1024];

    let user_rhk = rhk.map(|rhk| rhk.as_ptr() as *const u32).unwrap_or(null());

    let options = KbOptions {
        version: 1,
        buffer: user_buf.as_mut_ptr() as *mut u8,
        buffer_length: core::mem::size_of_val(&user_buf) as u32,
        op: KbOperation::AuthenticateImage,
        settings: KbSettings {
            authenticate: KbAuthenticate {
                profile: 0,
                min_build_number: 0,
                max_image_length,
                user_rhk,
            },
        },
    };

    let status = unsafe { (api_table().iap_driver.init)(&mut session_ref, &options) };
    if status != KbStatus::Success as u32 {
        error!("kinit failed with {:?}", status);
        return Err(AuthenticateError::Fail);
    }

    // Placeholder value that will be mutated by skboot_authenticate.
    let mut is_sign_verified: u32 = 0xffffffff;
    let result = unsafe { (api_table().skboot.authenticate)(start, &mut is_sign_verified) };

    // ROM API keeps HASHCRYPT unmasked
    cortex_m::peripheral::NVIC::mask(embassy_imxrt::pac::Interrupt::HASHCRYPT);

    let status = unsafe { (api_table().iap_driver.deinit)(session_ref) };
    if status != KbStatus::Success as u32 {
        error!("kdeinit failed with {:?}", status);
        return Err(AuthenticateError::Fail);
    }

    let status = BootStatus::try_from(result).map_err(|()| AuthenticateError::BootStatusUnknown)?;
    let is_sign_verified =
        SecureBool::try_from(is_sign_verified).map_err(|()| AuthenticateError::IsSignVerifiedUnknown);

    match status {
        BootStatus::Success => match is_sign_verified {
            Ok(SecureBool::TrackerVerified) => Ok(()),
            Ok(SecureBool::False) => Err(AuthenticateError::SignUnverified),
            _ => Err(AuthenticateError::SignUnknown),
        },
        BootStatus::Fail => Err(AuthenticateError::Fail),
        BootStatus::InvalidArgument => Err(AuthenticateError::UnexpectedValueInImage),
        BootStatus::KeyStoreMarkerInvalid => Err(AuthenticateError::KeyStoreMarkerInvalid),
        _ => Err(AuthenticateError::BootStatusUnknown),
    }
}

#[cfg(feature = "rt")]
#[interrupt]
#[allow(non_snake_case)]
fn HASHCRYPT() {
    unsafe { (api_table().skboot.hashcrypt_irq_handler)() }
}
