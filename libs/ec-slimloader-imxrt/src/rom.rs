use core::ptr::{null, null_mut};
use defmt_or_log::{error, warn};
use mimxrt685s_pac::interrupt;

#[repr(C)]
#[derive(Default, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct Version {
    bugfix: u8,
    minor: u8,
    major: u8,
    name: u8,
}

#[repr(C)]
struct SKBoot {
    pub authenticate: unsafe extern "C" fn(start_addr: *const u32, is_verified: *mut u32) -> u32,
    pub hashcrypt_irq_handler: unsafe extern "C" fn() -> (),
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KbAuthenticate {
    profile: u32,
    min_build_number: u32,
    max_image_length: u32,
    user_rhk: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KbRegion {
    address: u32,
    length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct KbLoadSB {
    profile: u32,
    min_build_number: u32,
    override_sbboot_section_id: u32,
    user_sbkek: *const u32,
    region_count: u32,
    regions: *const KbRegion,
}

#[repr(C)]
union KbSettings {
    authenticate: KbAuthenticate,
    load_sb: KbLoadSB,
}

#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(unused)]
enum KbOperation {
    AuthenticateImage = 1,
    LoadImage = 2,
}

#[repr(C)]
struct KbOptions {
    version: u32,
    buffer: *mut u8,
    buffer_length: u32,
    op: KbOperation,
    settings: KbSettings,
}

#[repr(C)]
struct KbSessionRef {
    context: KbOptions,
    cau_3_initialized: bool,
    memory_map: *const u8,
}

#[repr(C)]
#[derive(PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(unused)]
enum KbStatus {
    Success = 0,
    Fail = 1,
    ReadOnly = 2,
    OutOfRange = 3,
    InvalidArgument = 4,
    Timeout = 5,
    NoTransferInProgress = 6,
    /// Undocumented status code when passing insufficient memory.
    UnknownInsufficientMemory = 10,
    /// Incorrect SB2.1 loader signature.
    Signature = 10101,
    /// The SB state machine is waiting for more data.
    DataUnderrun = 10109,
    /// An image version rollback event has been detected.
    RollbackBlocked = 10115,
    Unknown,
}

#[repr(C)]
struct IAPDriver {
    pub init: unsafe extern "C" fn(*mut *mut KbSessionRef, *const KbOptions) -> u32,
    pub deinit: unsafe extern "C" fn(*mut KbSessionRef) -> u32,
    pub execute: unsafe extern "C" fn(*mut KbSessionRef, *const u8, u32) -> u32,
}

/// ROM API layout 42.9.3.1, RT6xx user manual UM11147.
#[repr(C)]
struct ApiTable {
    bootloader_fn: unsafe extern "C" fn(*const u8),
    version: Version,
    copyright: &'static [u8; 0],
    reserved: u32,
    iap_driver: &'static IAPDriver,
    reserved1: u32,
    reserved2: u32,
    flash_driver: &'static [u8; 0], // stubbed
    otp_driver: &'static [u8; 0],   // stubbed
    pub skboot: &'static SKBoot,
}

extern "C" {
    static API_TABLE: ApiTable;
}

fn api_table() -> &'static ApiTable {
    unsafe { &API_TABLE }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum BootStatus {
    Success,
    Fail,
    InvalidArgument,
    KeyStoreMarkerInvalid,
    HashcryptFinishedWithStatusSuccess,
    HashcryptFinishedWithStatusFail,
}

impl TryFrom<u32> for BootStatus {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0x5ac3c35a => BootStatus::Success,
            0xc35ac35a => BootStatus::Fail,
            0xc35a5ac3 => BootStatus::InvalidArgument,
            0xc3c35a5a => BootStatus::KeyStoreMarkerInvalid,
            0xc15a5ac3 => BootStatus::HashcryptFinishedWithStatusSuccess,
            0xc15a5acb => BootStatus::HashcryptFinishedWithStatusFail,
            _ => return Err(()),
        })
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum SecureBool {
    True,
    False,
    CallProtectSecurityFlags,
    CallProtectIsAppReady,
    TrackerVerified,
}

impl TryFrom<u32> for SecureBool {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            0xc33cc33c => SecureBool::True,
            0x5aa55aa5 => SecureBool::False,
            0xc33c5aa5 => SecureBool::CallProtectSecurityFlags,
            0x5aa5c33c => SecureBool::CallProtectIsAppReady,
            0x55aacc33 => SecureBool::TrackerVerified,
            _ => {
                return Err(());
            }
        })
    }
}

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

pub fn skboot_authenticate(start: *const u32, max_image_length: u32) -> Result<(), AuthenticateError> {
    // Note:
    // The ROM reserved space for global variables in RAM on this device is:
    // 0x1001_2000 to 0x1000_A000

    // 43.9 Secure ROM API page 1282 of RT6xx User manual

    let mut session_ref = null_mut();
    let mut user_buf = [0u32; 1024];

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
                user_rhk: null(), // TODO perhaps application-specific RHK?
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

    if cortex_m::peripheral::NVIC::is_enabled(mimxrt685s_pac::Interrupt::HASHCRYPT) {
        warn!("ROM API kept HASHCRYPT unmasked...");
        cortex_m::peripheral::NVIC::mask(mimxrt685s_pac::Interrupt::HASHCRYPT);
    }

    let status = unsafe { (api_table().iap_driver.deinit)(session_ref) };
    if status != KbStatus::Success as u32 {
        error!("kdeinit failed");
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

#[interrupt]
fn HASHCRYPT() {
    unsafe { (api_table().skboot.hashcrypt_irq_handler)() }
}
