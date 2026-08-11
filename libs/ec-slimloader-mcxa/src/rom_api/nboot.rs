#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Safety: null, misaligned, invalid inputs or out of bounds references will cause ROM API to return an error code, which is handled by the caller.
use super::{NbootBool, NbootStatusProtected};
use crate::error::*;

// Boolean/result markers returned via nboot_bool_t out-parameters.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootBoolValue {
    True = 0x3C5A_C33C,
    True256 = 0x3C5A_C35A,
    True384 = 0x3C5A_C3A5,
    False = 0x5AA5_5AA5,
}

impl NbootBoolValue {
    #[inline(always)]
    pub const fn from_raw(raw: NbootBool) -> Option<Self> {
        match raw {
            0x3C5A_C33C => Some(Self::True),
            0x3C5A_C35A => Some(Self::True256),
            0x3C5A_C3A5 => Some(Self::True384),
            0x5AA5_5AA5 => Some(Self::False),
            _ => None,
        }
    }
}

#[inline(always)]
pub const fn nboot_bool_is_true(value: NbootBool) -> bool {
    matches!(NbootBoolValue::from_raw(value), Some(NbootBoolValue::True)) // Only using TRUE because only supporting hybrid mode, NO ECDSA only support.
}

#[repr(C)]
pub struct NbootCtx {
    // Opaque context buffer. Size must match NBOOT_CONTEXT_SIZE from the ROM header.
    pub opaque: [u8; 0xA94],
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootMemCryptRegion {
    Region0 = 0,
    Region1 = 1,
    Region2 = 2,
    Region3 = 3,
    Region4 = 4,
    Region5 = 5,
    Region6 = 6,
    Region7 = 7,
}

pub type NbootMemCryptOperation = u32; // Need to confirm exact typed values.

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootMemCryptIpedModeSelect {
    Rounds12 = 0x3C5A_C33C,
    Rounds22 = 0x5AA5_5AA5,
    FullyPipelined = 0x5A5A_5A5A,
    NotFullyPipelined = 0xA5A5_A5A5,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootMemCryptEngineType {
    Npx = 0x5959_5959,
    Iped = 0x9595_9595,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NbootNpxRegionConfig {
    // Must be NbootMemCryptEngineType::Npx.
    pub configId: NbootMemCryptEngineType,
    // Start address (inclusive).
    pub startAddress: u32,
    // End address (inclusive).
    pub endAddress: u32,
    // Subregion bitmap.
    pub subregion: u64,
    // IV erase counter.
    pub ivEraseCounter: u32,
    // Region lock flag.
    pub regionLock: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NbootIpedRegionConfig {
    // Must be NbootMemCryptEngineType::Iped.
    pub configId: NbootMemCryptEngineType,
    // Start address (inclusive).
    pub startAddress: u32,
    // End address (inclusive).
    pub endAddress: u32,
    // IV erase counter.
    pub ivEraseCounter: u32,
    // Region flags.
    pub regionFlags: u8,
    // Additional authenticated data.
    pub aad: u32,
}

#[repr(C)]
pub union NbootMemCryptRegionConfig {
    pub npxConfig: NbootNpxRegionConfig,
    pub ipedConfig: NbootIpedRegionConfig,
}

#[repr(C)]
pub struct NbootRotAuthParms {
    pub soc_rootKeyRevocation: [u32; 4],
    pub soc_imageKeyRevocation: u32,
    pub soc_rkh: [u32; 12],
    pub soc_rkh_1: [u32; 12], // PQC_ROTKH (hash of hashes)
    pub soc_numberOfRootKeys: u32,
    pub soc_rootKeyUsage: [u32; 4],
    pub soc_rootKeyTypeAndLength: u32,
    pub soc_lifecycle: u32,
}

#[repr(C)]
pub struct NbootImgAuthParms {
    pub soc_RoTNVM: NbootRotAuthParms,
    pub soc_trustedFirmwareVersion: u32,
}

#[repr(C)]
pub struct NbootImgAuthenticateCmacParms {
    pub expectedMAC: [u32; 4],
}

#[repr(C)]
pub struct NbootSb4LoadManifestParms {
    // Returned RoTNVM/auth parameters.
    pub soc_RoTNVM: NbootRotAuthParms,
    // Returned trusted firmware version.
    pub soc_trustedFirmwareVersion: u32,
    // Returned maximum SB block size.
    pub maxBlockSize: u32,
    // Returned PCK blob.
    pub pckBlob: [u8; 48],
}

// Root key configuration constants for NBOOT
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootRootKeyRevocation {
    Enabled = 0xAA,
    Revoked = 0xBB,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootRootKeyUsage {
    All = 0x0,
    DebugCa = 0x1,
    ImageCaFwCa = 0x2,
    DebugCaImageCaFwCa = 0x3,
    ImageKeyFwKey = 0x4,
    ImageKey = 0x5,
    FwKey = 0x6,
    Unused = 0x7,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootRootKeyType {
    EcdsaP384Mldsa87 = 0x0000_FD04, // Hybrid root key type: ECDSA P-384 + ML-DSA-87
}

// Lifecycle state codes (low-byte discriminators) and full CFPA LC_STATE values.
// Per Table 18 (Life Cycle States): LC_STATE is a u32 like 0x9635_FC03.
// Some call sites only carry the low-byte discriminator (e.g. 0x03 for Develop),
// so we keep both representations.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootLifecycleDiscriminator {
    Develop = 0x03,
    Develop2 = 0x07,
    InField = 0x0F,
    InFieldLocked = 0xCF,
    OemFieldReturn = 0x1F,
    FailureAnalysis = 0x3F,
    Bricked = 0x5A,
}

impl NbootLifecycleDiscriminator {
    #[inline(always)]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0x03 => Some(Self::Develop),
            0x07 => Some(Self::Develop2),
            0x0F => Some(Self::InField),
            0xCF => Some(Self::InFieldLocked),
            0x1F => Some(Self::OemFieldReturn),
            0x3F => Some(Self::FailureAnalysis),
            0x5A => Some(Self::Bricked),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn state(self) -> NbootLifecycleState {
        match self {
            Self::Develop => NbootLifecycleState::Develop,
            Self::Develop2 => NbootLifecycleState::Develop2,
            Self::InField => NbootLifecycleState::InField,
            Self::InFieldLocked => NbootLifecycleState::InFieldLocked,
            Self::OemFieldReturn => NbootLifecycleState::OemFieldReturn,
            Self::FailureAnalysis => NbootLifecycleState::FailureAnalysis,
            Self::Bricked => NbootLifecycleState::Bricked,
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NbootLifecycleState {
    Develop = 0x9635_FC03,
    Develop2 = 0x9635_F807,
    InField = 0x9635_F00F,
    InFieldLocked = 0x9635_30CF,
    OemFieldReturn = 0x9635_E01F,
    FailureAnalysis = 0x9635_C03F,
    Bricked = 0x9635_A55A,
}

impl NbootLifecycleState {
    #[inline(always)]
    pub const fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0x9635_FC03 => Some(Self::Develop),
            0x9635_F807 => Some(Self::Develop2),
            0x9635_F00F => Some(Self::InField),
            0x9635_30CF => Some(Self::InFieldLocked),
            0x9635_E01F => Some(Self::OemFieldReturn),
            0x9635_C03F => Some(Self::FailureAnalysis),
            0x9635_A55A => Some(Self::Bricked),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn discriminator(self) -> NbootLifecycleDiscriminator {
        match self {
            Self::Develop => NbootLifecycleDiscriminator::Develop,
            Self::Develop2 => NbootLifecycleDiscriminator::Develop2,
            Self::InField => NbootLifecycleDiscriminator::InField,
            Self::InFieldLocked => NbootLifecycleDiscriminator::InFieldLocked,
            Self::OemFieldReturn => NbootLifecycleDiscriminator::OemFieldReturn,
            Self::FailureAnalysis => NbootLifecycleDiscriminator::FailureAnalysis,
            Self::Bricked => NbootLifecycleDiscriminator::Bricked,
        }
    }

    #[inline(always)]
    pub const fn nboot_soc_lifecycle(self) -> u32 {
        let discriminator = self.discriminator() as u16;
        (((!discriminator) as u32) << 16) | (discriminator as u32)
    }

    #[inline(always)]
    pub const fn from_any_raw(raw: u32) -> Option<Self> {
        if let Some(state) = Self::from_raw(raw) {
            return Some(state);
        }
        // Short-form input: bare discriminator byte only. A long-form value that
        // failed the full-word match above is corrupt; do not guess from its low byte.
        if raw <= 0xFF {
            if let Some(d) = NbootLifecycleDiscriminator::from_raw(raw as u8) {
                return Some(d.state());
            }
        }
        None
    }

    /// Returns a monotonic rank for forward-only progression checks.
    /// Higher rank = further along the lifecycle.
    #[inline(always)]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Develop => 0,
            Self::Develop2 => 1,
            Self::InField => 2,
            Self::InFieldLocked => 2, // Same rank as InField since locking is not a lifecycle progression.
            Self::OemFieldReturn => 3,
            Self::FailureAnalysis => 4,
            Self::Bricked => 5,
        }
    }

    /// Returns true if advancing to `next` is a valid forward progression (no regressions, no same state).
    #[inline(always)]
    pub const fn can_advance_to(self, next: Self) -> bool {
        next.rank() >= self.rank() && (self as u32 != next as u32)
    }
}

// Sealed trait: CanAdvanceTo<Next>

// Only the combinations listed below exist. Attempting to call
// `cfpa_stage_lifecycle_advance_and_reset` with any other (From, To) pair
// is a *compile* error.

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Develop {}
    impl Sealed for super::Develop2 {}
    impl Sealed for super::InField {}
    impl Sealed for super::InFieldLocked {}
    impl Sealed for super::OemFieldReturn {}
    impl Sealed for super::FailureAnalysis {}
    impl Sealed for super::Bricked {}
}

// Zero-sized types, one per lifecycle state, used as type parameters to enforce valid lifecycle transitions at compile time.
pub struct Develop;
pub struct Develop2;
pub struct InField;
pub struct InFieldLocked;
pub struct OemFieldReturn;
pub struct FailureAnalysis;
pub struct Bricked;

/// Compile-time proof that advancing from `Self` to `Next` is a valid transition.
/// Only the explicit `impl` blocks below are permitted.
pub trait CanAdvanceTo<Next>: sealed::Sealed {}

impl CanAdvanceTo<Develop2> for Develop {}
impl CanAdvanceTo<InField> for Develop {}
impl CanAdvanceTo<InField> for Develop2 {}
impl CanAdvanceTo<InFieldLocked> for Develop2 {}
impl CanAdvanceTo<InFieldLocked> for InField {}
impl CanAdvanceTo<InField> for InFieldLocked {}
impl CanAdvanceTo<OemFieldReturn> for InField {}
impl CanAdvanceTo<OemFieldReturn> for InFieldLocked {}
impl CanAdvanceTo<FailureAnalysis> for OemFieldReturn {}
impl CanAdvanceTo<Bricked> for Develop2 {}
impl CanAdvanceTo<Bricked> for InFieldLocked {}
impl CanAdvanceTo<Bricked> for InField {}
impl CanAdvanceTo<Bricked> for OemFieldReturn {}
impl CanAdvanceTo<Bricked> for FailureAnalysis {}

// Forward declarations of subtables.
#[repr(C)]
pub(super) struct NbootDriverRaw {
    // Initialize NBOOT context (must be called before other NBOOT APIs).
    pub nboot_context_init: unsafe extern "C" fn(ctx: *mut NbootCtx) -> NbootStatusProtected,
    // Deinitialize NBOOT context.
    pub nboot_context_deinit: unsafe extern "C" fn(ctx: *mut NbootCtx) -> NbootStatusProtected,
    // Set context UUID (bytes; see NXP header for exact semantics).
    pub nboot_context_set_uuid: unsafe extern "C" fn(ctx: *mut NbootCtx, uuid: *const u8) -> NbootStatusProtected,
    // SB4: load/parse manifest.
    pub nboot_sb4_load_manifest: unsafe extern "C" fn(
        ctx: *mut NbootCtx,
        manifest: *const u32,
        parms: *mut NbootSb4LoadManifestParms,
    ) -> NbootStatusProtected,
    // SB4: load next block.
    pub nboot_sb4_load_block: unsafe extern "C" fn(ctx: *mut NbootCtx, block: *mut u32) -> NbootStatusProtected,
    // SB4: authenticate & complete check (ROMAPI entry).
    pub nboot_sb4_check_authenticity_and_completeness_romapi: unsafe extern "C" fn(
        ctx: *mut NbootCtx,
        address: *const u32,
        parms: *mut NbootSb4LoadManifestParms,
    ) -> NbootStatusProtected,
    // Authenticate image (signature verification result returned via is_signature_verified).
    pub nboot_img_authenticate_romapi: unsafe extern "C" fn(
        ctx: *mut NbootCtx,
        image_start: *const u8,
        is_signature_verified: *mut NbootBool,
        parms: *mut NbootImgAuthParms,
    ) -> NbootStatusProtected,

    // Enable/Configure in-memory encryption for a given address range.
    pub nboot_mem_crypt_enable_encrypt_for_address_range: unsafe extern "C" fn(
        ctx: *mut NbootCtx,
        region_number: NbootMemCryptRegion,
        region_config: *mut NbootMemCryptRegionConfig,
        iped_mode_select: NbootMemCryptIpedModeSelect,
    ) -> NbootStatusProtected,
    // Check whether an operation is allowed for an address range and return flags/IV counters.
    pub nboot_mem_crypt_range_checker: unsafe extern "C" fn(
        ctx: *mut NbootCtx,
        operation: NbootMemCryptOperation,
        address: u32,
        length: u32,
        flags: *mut u8,
        npx_iv_erase_cntr: *mut u32,
        iped_iv_erase_cntr: *mut u32,
        npx_erase_check_en: u32,
    ) -> NbootStatusProtected,
    // Enable background hashing (ROM-managed hash engine / DMA channel selection).
    pub nboot_background_hash_enable:
        unsafe extern "C" fn(ctx: *mut NbootCtx, hash_dma_channel: u32) -> NbootStatusProtected,
}

#[derive(Clone, Copy)]
pub struct NbootDriver {
    raw: &'static NbootDriverRaw,
}

impl NbootDriver {
    pub(super) const fn from_raw(raw: &'static NbootDriverRaw) -> Self {
        Self { raw }
    }

    pub fn nboot_context_init(&self, ctx: *mut NbootCtx) -> NbootStatus {
        unsafe { NbootStatus::from_raw((self.raw.nboot_context_init)(ctx)) }
    }

    pub fn nboot_context_deinit(&self, ctx: *mut NbootCtx) -> NbootStatus {
        unsafe { NbootStatus::from_raw((self.raw.nboot_context_deinit)(ctx)) }
    }

    pub fn nboot_context_set_uuid(&self, ctx: *mut NbootCtx, uuid: *const u8) -> NbootStatus {
        unsafe { NbootStatus::from_raw((self.raw.nboot_context_set_uuid)(ctx, uuid)) }
    }

    pub fn nboot_sb4_load_manifest(
        &self,
        ctx: *mut NbootCtx,
        manifest: *const u32,
        parms: *mut NbootSb4LoadManifestParms,
    ) -> NbootStatus {
        unsafe { NbootStatus::from_raw((self.raw.nboot_sb4_load_manifest)(ctx, manifest, parms)) }
    }

    pub fn nboot_sb4_load_block(&self, ctx: *mut NbootCtx, block: *mut u32) -> NbootStatus {
        unsafe { NbootStatus::from_raw((self.raw.nboot_sb4_load_block)(ctx, block)) }
    }

    pub fn nboot_sb4_check_authenticity_and_completeness_romapi(
        &self,
        ctx: *mut NbootCtx,
        address: *const u32,
        parms: *mut NbootSb4LoadManifestParms,
    ) -> NbootStatus {
        unsafe {
            NbootStatus::from_raw((self.raw.nboot_sb4_check_authenticity_and_completeness_romapi)(
                ctx, address, parms,
            ))
        }
    }

    pub fn nboot_img_authenticate_romapi(
        &self,
        ctx: *mut NbootCtx,
        image_start: *const u8,
        is_signature_verified: *mut NbootBool,
        parms: *mut NbootImgAuthParms,
    ) -> NbootStatus {
        unsafe {
            NbootStatus::from_raw((self.raw.nboot_img_authenticate_romapi)(
                ctx,
                image_start,
                is_signature_verified,
                parms,
            ))
        }
    }

    pub fn nboot_mem_crypt_enable_encrypt_for_address_range(
        &self,
        ctx: *mut NbootCtx,
        region_number: NbootMemCryptRegion,
        region_config: *mut NbootMemCryptRegionConfig,
        iped_mode_select: NbootMemCryptIpedModeSelect,
    ) -> NbootStatus {
        unsafe {
            NbootStatus::from_raw((self.raw.nboot_mem_crypt_enable_encrypt_for_address_range)(
                ctx,
                region_number,
                region_config,
                iped_mode_select,
            ))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn nboot_mem_crypt_range_checker(
        &self,
        ctx: *mut NbootCtx,
        operation: NbootMemCryptOperation,
        address: u32,
        length: u32,
        flags: *mut u8,
        npx_iv_erase_cntr: *mut u32,
        iped_iv_erase_cntr: *mut u32,
        npx_erase_check_en: u32,
    ) -> NbootStatus {
        unsafe {
            NbootStatus::from_raw((self.raw.nboot_mem_crypt_range_checker)(
                ctx,
                operation,
                address,
                length,
                flags,
                npx_iv_erase_cntr,
                iped_iv_erase_cntr,
                npx_erase_check_en,
            ))
        }
    }

    pub fn nboot_background_hash_enable(&self, ctx: *mut NbootCtx, hash_dma_channel: u32) -> NbootStatus {
        // should enum for DMA channel selection be added? (Answer is yes but which channels??) For now just pass 0 for default channel. By doing an enum,
        // we can impose limitations on valid values (e.g. if only 2 channels are supported, etc.)
        unsafe { NbootStatus::from_raw((self.raw.nboot_background_hash_enable)(ctx, hash_dma_channel)) }
    }
}
