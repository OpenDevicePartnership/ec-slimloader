#[repr(C)]
#[derive(Default, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Version {
    bugfix: u8,
    minor: u8,
    major: u8,
    name: u8,
}

#[repr(C)]
pub struct SKBoot {
    pub authenticate: unsafe extern "C" fn(start_addr: *const u32, is_verified: *mut u32) -> u32,
    pub hashcrypt_irq_handler: unsafe extern "C" fn() -> (),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KbAuthenticate {
    pub profile: u32,
    pub min_build_number: u32,
    pub max_image_length: u32,
    pub user_rhk: *const u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct KbRegion {
    pub address: u32,
    pub length: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KbLoadSB {
    pub profile: u32,
    pub min_build_number: u32,
    pub override_sbboot_section_id: u32,
    pub user_sbkek: *const u32,
    pub region_count: u32,
    pub regions: *const KbRegion,
}

#[repr(C)]
pub union KbSettings {
    pub authenticate: KbAuthenticate,
    pub load_sb: KbLoadSB,
}

#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(unused)]
pub enum KbOperation {
    AuthenticateImage = 1,
    LoadImage = 2,
}

#[repr(C)]
pub struct KbOptions {
    pub version: u32,
    pub buffer: *mut u8,
    pub buffer_length: u32,
    pub op: KbOperation,
    pub settings: KbSettings,
}

#[repr(C)]
pub struct KbSessionRef {
    pub context: KbOptions,
    pub cau_3_initialized: bool,
    pub memory_map: *const u8,
}

#[repr(C)]
#[derive(PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(unused)]
pub enum KbStatus {
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
pub struct IAPDriver {
    pub init: unsafe extern "C" fn(*mut *mut KbSessionRef, *const KbOptions) -> u32,
    pub deinit: unsafe extern "C" fn(*mut KbSessionRef) -> u32,
    pub execute: unsafe extern "C" fn(*mut KbSessionRef, *const u8, u32) -> u32,
}

#[repr(C)]
pub struct OTPDriver {
    pub init: unsafe extern "C" fn(src_clk_freq: u32) -> u32,
    pub deinit: unsafe extern "C" fn() -> u32,
    pub fuse_read: unsafe extern "C" fn(addr: u32, *mut u8) -> u32,
    pub fuse_program: unsafe extern "C" fn(addr: u32, data: u32, lock: bool) -> u32,
    pub crc_calc: unsafe extern "C" fn(src: *const u32, number_of_worlds: u32, crc_checksum: *const u32) -> u32,
    pub reload: unsafe extern "C" fn() -> u32,
    pub crc_check: unsafe extern "C" fn(start_addr: u32, end_addr: u32, crc_addr: u32) -> u32,
}

/// ROM API layout 42.9.3.1, RT6xx user manual UM11147.
#[repr(C)]
pub struct ApiTable {
    bootloader_fn: unsafe extern "C" fn(*const u8),
    pub version: Version,
    pub copyright: &'static [u8; 0],
    reserved: u32,
    pub iap_driver: &'static IAPDriver,
    reserved1: u32,
    reserved2: u32,
    flash_driver: &'static [u8; 0], // stubbed
    pub otp_driver: &'static OTPDriver,
    pub skboot: &'static SKBoot,
}

extern "C" {
    static API_TABLE: ApiTable;
}

pub fn api_table() -> &'static ApiTable {
    unsafe { &API_TABLE }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BootStatus {
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
pub enum SecureBool {
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
