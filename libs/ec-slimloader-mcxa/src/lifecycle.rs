use core::mem;

use crate::rom_api::{NbootLifecycleDiscriminator, NbootLifecycleState, NbootRootKeyRevocation, NbootRootKeyUsage};
use crate::verification::zero_mask;

// MCXA configuration flash layout (CFG vs SCRATCH)
//
// NOTE:
// - Reads should use the CFG area.
// - Updates should be staged into the SCRATCH area (then committed by the proper
//   ROM/programming flow; this module only provides addresses and helpers).
//
// CFG (read):
//   CFPA  0x0100_0000 - 0x0100_01FF
//   CMPA  0x0100_0200 - 0x0100_03FF
//   CMPA customer-defined 0x0100_0400 - 0x0100_17FF
//
// SCRATCH (write staging): MUST use secure alias.
//   CFPA  0x1100_2000 - 0x1100_21FF
//   CMPA  0x1100_2200 - 0x1100_23FF
//   CMPA customer-defined 0x1100_2400 - 0x1100_37FF

const CMPA_HEADER_MARKER: u16 = 0x5963;
const CFPA_HEADER_MARKER: u16 = 0x9635;
// Any field reading back this value should be treated as "not provisioned" rather than a real configuration.
const ERASED_WORD: u32 = 0xFFFF_FFFF;

// CFG bases (use for reading)
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IFRConfigAreaBase {
    Cfpa = 0x0100_0000,
    Cmpa = 0x0100_0200,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IFRPage {
    Cfpa,
    Cmpa,
    CmpaCustDefined,
    CmpaAll,
}

impl IFRPage {
    #[inline(always)]
    pub const fn start_offset(self) -> u32 {
        match self {
            Self::Cfpa => 0x0000,
            Self::Cmpa => 0x0200,
            Self::CmpaCustDefined => 0x0400,
            Self::CmpaAll => 0x0200, // start of CMPA, used for operations that cover the entire CMPA including customer-defined area.
        }
    }

    #[inline(always)]
    pub const fn end_offset_inclusive(self) -> u32 {
        match self {
            Self::Cfpa => 0x01FF,
            Self::Cmpa => 0x03FF,
            Self::CmpaCustDefined => 0x17FF,
            Self::CmpaAll => 0x17FF, // end of CMPA customer-defined area, used for operations that cover the entire CMPA including customer-defined area.
        }
    }

    #[inline(always)]
    pub const fn byte_len(self) -> usize {
        (self.end_offset_inclusive() - self.start_offset() + 1) as usize
    }
}

// RoTKH locations in CMPA (absolute addresses provided):
// - ROTKH @ 0x0100_0260 (i.e. IFRConfigAreaBase::Cmpa + 0x60)
// - PQC_ROTKH @ 0x0100_02C0 (i.e. IFRConfigAreaBase::Cmpa + 0xC0)
// CMPA secure-boot related fields (MCXA):
// Base is 0x0100_0200 and offsets below come from the reference table.
// Unused fields from the reference table are omitted here but can be added as needed for future features.
// const CMPA_BOOT_LED_STATUS: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0008; // 0x0100_0208
// const CMPA_BOOT_TIMERS: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x000C; // 0x0100_020C

#[inline(always)]
pub fn load_cmpa_boot_cfg0() -> u32 {
    const CMPA_BOOT_CFG0: u32 = IFRConfigAreaBase::Cmpa as u32; // 0x0100_0200
    unsafe { core::ptr::read_volatile(CMPA_BOOT_CFG0 as *const u32) }
}

#[inline(always)]
pub fn load_cmpa_boot_cfg1() -> u32 {
    const CMPA_BOOT_CFG1: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0004; // 0x0100_0204
    unsafe { core::ptr::read_volatile(CMPA_BOOT_CFG1 as *const u32) }
}

// Erased scan: OR-accumulate, no early exit.
pub fn cmpa_erased_deviation() -> u32 {
    // NOTE: ifr_verify_erase_page is NOT a read-only check; it erases then verifies (destructive).
    // Therefore it cannot be used to check erased state of CFG area (protected by ROM).
    // read_volatile is the correct approach for a non-destructive erased check.
    let base = IFRConfigAreaBase::Cmpa as u32;
    let word_count = IFRPage::Cmpa.byte_len() / core::mem::size_of::<u32>();
    let mut d = 0u32;
    for i in 0..word_count {
        d |= unsafe { core::ptr::read_volatile((base + i as u32 * 4) as *const u32) } ^ ERASED_WORD;
    }
    d
}

pub fn is_cmpa_erased() -> bool {
    cmpa_erased_deviation() == 0
}

pub fn is_cfpa_erased() -> bool {
    let base = IFRConfigAreaBase::Cfpa as u32;
    let word_count = IFRPage::Cfpa.byte_len() / core::mem::size_of::<u32>();
    // Skip UPD_TYPE/UPD_PARAM (offsets 0x00-0x0C) and the header word itself (offset 0x10).
    // Check from PAGE_VERSION (offset 0x14, word index 5) onwards.
    // Also skip ROM-maintained error counters — written autonomously by the ROM after auth
    // failures and/or Intrusion detects ; must not be treated as "provisioned config".
    // ERR_AUTH_FAIL_COUNT at absolute offset 0x50, ERR_ITRC_COUNT at absolute offset 0x54.
    const FIRST_WORD_AFTER_HEADER: usize = (0x0010_usize / core::mem::size_of::<u32>()) + 1; // 5
    const ERR_AUTH_FAIL_COUNT_IDX: usize = 0x50 / core::mem::size_of::<u32>(); // 20
    const ERR_ITRC_COUNT_IDX: usize = 0x54 / core::mem::size_of::<u32>(); // 21
    for i in FIRST_WORD_AFTER_HEADER..word_count {
        if i == ERR_AUTH_FAIL_COUNT_IDX || i == ERR_ITRC_COUNT_IDX {
            continue;
        }
        let addr = base + (i as u32 * 4);
        let val = unsafe { core::ptr::read_volatile(addr as *const u32) };
        if val != ERASED_WORD {
            return false;
        }
    }
    true
}

fn cmpa_marker_deviation() -> u32 {
    (load_cmpa_boot_cfg0() >> 16) ^ (CMPA_HEADER_MARKER as u32) // Measures if CMPA header marker is valid (0 deviation = valid, non-zero deviation = invalid) without branching.
}

#[inline(always)]
fn load_cmpa_header_marker() -> u16 {
    (load_cmpa_boot_cfg0() >> 16) as u16
}

#[inline(always)]
pub fn cmpa_header_marker_is_valid() -> bool {
    // CMPA BOOT_CFG0 header marker semantics (MCXA):
    // Marker should be set to 0x5963. After this header is set, all non-zero values will take effect;
    // leaving all settings at 0xFF will cause undefined behavior. It is recommended to set all values
    // to 0x00 before setting the CMPA header marker.
    //
    // Layout assumed consistent with CFPA header marker usage: marker stored in bits [31:16].
    load_cmpa_header_marker() == CMPA_HEADER_MARKER
}

// The following CMPA fields are defined in the reference table but not yet used in this module; they can be added as needed for future features:
// const CMPA_ERR_LOG_ADDR: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x005C; // 0x0100_025C
// const CMPA_CUST_MK_SK_KEY_BLOB_START: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0090; // 0x0100_0290
// const CMPA_CUST_MK_SK_KEY_BLOB_WORDS: usize = 12;

#[inline(always)]
fn load_cmpa_secure_boot_cfg() -> u32 {
    const CMPA_SECURE_BOOT_CFG: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0050; // 0x0100_0250
    unsafe { core::ptr::read_volatile(CMPA_SECURE_BOOT_CFG as *const u32) }
}

/// Measures the "deviation" without any branches from the live configuration of a device from the expected configuration of a development device. This is used to determine if the device
/// is in development mode or production mode. The function returns 0 if the device is in development mode, and non-zero if the device is in production mode.
pub fn dev_config_deviation() -> u32 {
    // Lifecycle leg: full-word match against the derived Develop pattern.
    let lc_dev = load_cfpa_header_word_raw() ^ cfpa_header_for(NbootLifecycleState::Develop); // If we are in Develop LC, 0 deviation because XOR.

    // Provisioned-dev leg: valid CMPA marker AND SEC_BOOT_EN bit 1 clear.
    let provisioned_dev = cmpa_marker_deviation() // is marker valid? (0 = valid, non-zero = invalid)
        | (load_cmpa_secure_boot_cfg() & 0b10).wrapping_mul(0x5555_5555); // Is secure boot enabled? (deviation of 0 = disabled, non-zero = enabled).
                                                                          // Note that value of 0 or 1 in 1:0 in SEC_BOOT_EN means signature verification is not enforced.
                                                                          // Multiply by 0x5555_5555 to spread the bit across all 32 bits for OR-accumulation. This here is the narrowest Shannon bit at source,
                                                                          // beyond our control, so we can widen the single bit to all 32 bits without any branches.
                                                                          // Signature verification enforcement causes deviation from the expected dev-mode configuration.

    // Blank-part leg: whole CMPA page still erased (provisioning bootstrap).
    let sb_dev = !(zero_mask(provisioned_dev) | zero_mask(cmpa_erased_deviation())); // If secure boot is disabled AND CMPA marker is valid,
                                                                                     // then we are in dev mode. Either false means deviation.  CMPA is not erased, deviation. BUT EITHER of these deviating means we have deviation from the expected dev-mode configuration.
                                                                                     // Therefore, in that case, return non-zero deviation. If both are true, then we are in dev mode, return zero deviation.

    lc_dev | sb_dev
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecureBootLevel {
    AllAllowed = 0,     // b00
    CrcOrSigned = 1,    // b01
    SignedOnly = 2,     // b10 (CMAC or ECDSA)
    EcdsaMldsaOnly = 3, // b11
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SecureBootState {
    HybridEnforced,
    Classical,
    Disabled,
    Unknown,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CnsaLevel {
    NotEnforced = 0, // b00 (non-CNSA or no enforcement)
    CnsaOne = 1,     // b01: CNSA1.0 (ECDSA p384 and SHA-384, AES-256)
    CnsaTwo = 2,     // b10 or b11 (hybrid PQC with ECDSA-384 and MLDSA-87, SHA-384, ML-KEM, AES-256)
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LpWakePolicy {
    FullAuthentication = 0, // b00 (same as normal secure boot, applies to low-power wake too)
    CrcOnly = 1,            // b01 (CRC check only for LP wake, full auth for normal boot)
    Jump = 2,               // b10 (jump to CFPA LP wake address without authentication)
    Cmac = 3,               // b11 (CMAC auth for LP wake, full hybrid auth for normal boot)
}

/// Active image protection mode (CMPA.SECURE_BOOT_CFG bits [15:14]).
/// Controls MBC (Memory Block Controller) protection applied to the active (XIP) image region.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XipImageProtect {
    /// Active image protection via CFPA flash ACL settings.
    FlashAclSettingCfpa = 0,
    /// Write protect active image area with sticky lock (GLBAC2)
    WriteProtectSticky = 1,
    /// Write protect active image area without sticky lock
    WriteProtect = 2,
    /// XOM (eXecute-Only Memory) protect active image area with sticky lock
    XomSticky = 3,
}

/// Helper function to load the secure boot enforcement level from CMPA and interpret it according to the reference table for MCXA.
pub fn secure_boot_level() -> SecureBootLevel {
    // CMPA.SECURE_BOOT_CFG: SEC_BOOT_EN is a 2-bit field in bits [1:0].
    match cmpa_secure_boot_cfg().sec_boot_en {
        0 => SecureBootLevel::AllAllowed,
        1 => SecureBootLevel::CrcOrSigned,
        2 => SecureBootLevel::SignedOnly,
        3 => SecureBootLevel::EcdsaMldsaOnly,
        _ => SecureBootLevel::AllAllowed, // treat invalid values as most permissive that way will be caught by policy validation.
    }
}

pub fn secure_boot_state() -> SecureBootState {
    // Fully erased CMPA = brand new device with no security configured
    if is_cmpa_erased() {
        return SecureBootState::Disabled;
    }
    // Invalid header but not fully erased = partially provisioned or corrupt
    if !cmpa_header_marker_is_valid() {
        return SecureBootState::Unknown;
    }

    match secure_boot_level() {
        SecureBootLevel::EcdsaMldsaOnly => SecureBootState::HybridEnforced,
        SecureBootLevel::SignedOnly => SecureBootState::Classical,
        SecureBootLevel::AllAllowed | SecureBootLevel::CrcOrSigned => SecureBootState::Disabled,
    }
}

/// Helper function that returns true only if secure boot is strictest with hybrid ECDSA+MLDSA AND IFR region not left in an unprovisioned/erased state that could cause undefined behavior.
pub fn hybrid_secure_boot_enforced() -> bool {
    matches!(secure_boot_state(), SecureBootState::HybridEnforced)
}

/// Helper function that returns true if CNSA2.0 is enfocred via CMPA SEC_BOOT_CFG.ENF_CNSA field, and false if not enforced or if CMPA is erased/unprovisioned.
pub fn cnsa_enforced() -> bool {
    let cnsa_level = match cmpa_secure_boot_cfg().enf_cnsa {
        0 => CnsaLevel::NotEnforced,
        1 => CnsaLevel::CnsaOne,
        2 | 3 => CnsaLevel::CnsaTwo,
        _ => CnsaLevel::NotEnforced,
    };
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cnsa_level == CnsaLevel::CnsaTwo
}

/// Helper function that returns true if fast boot is enabled via CMPA SEC_BOOT_CFG.FAST_BOOT_EN field, and false if disabled or if CMPA is erased/unprovisioned.
pub fn fast_boot_enabled() -> bool {
    // Fast boot is enabled when FAST_BOOT_EN field is 0b00, and disabled otherwise (full auth flow required)
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cmpa_secure_boot_cfg().fast_boot_en == 0
}

/// Helper function that returns true if low-power wake authentication is enforced via CMPA SEC_BOOT_CFG.LP_SEC_BOOT field, and false if not enforced or if CMPA is erased/unprovisioned.
pub fn low_power_authentication_enforced() -> bool {
    let lp_wake_policy = match cmpa_secure_boot_cfg().lp_sec_boot {
        0 => LpWakePolicy::FullAuthentication,
        1 => LpWakePolicy::CrcOnly,
        2 => LpWakePolicy::Jump,
        3 => LpWakePolicy::Cmac,
        _ => LpWakePolicy::Cmac,
    };
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && lp_wake_policy == LpWakePolicy::FullAuthentication
}

// CMPA.SECURE_BOOT_CFG decoder (MCXA)
//
// 2-bit fields, with 1 bit at bit 2 and bit 5:
// - [1:0]   SEC_BOOT_EN
// - [2]     (reserved)
// - [4:3]   LP_SEC_BOOT
// - [5]     (reserved)
// - [7:6]   DICE_CSR_KEY_TYPE
// - [9:8]   ENF_CNSA
// - [11:10] ENF_TZM_PRESET
// - [13:12] FAST_BOOT_EN
// - [15:14] ACTIVE_IMG_PROT
// - [17:16] FIPS_SHA_STEN
// - [19:18] FIPS_AES_STEN
// - [21:20] FIPS_ECDSA_STEN
// - [23:22] FIPS_DRBG_STEN
// - [25:24] FIPS_CMAC_STEN
// - [27:26] FIPS_KDF_STEN
// - [29:28] Reserved (2-bit)
// - [31:30] DIS_NXP_FW

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CmpaSecureBootCfgDecode {
    raw: u32,
    sec_boot_en: u8,
    lp_sec_boot: u8,
    dice_csr_key_type: u8,
    enf_cnsa: u8,
    enf_tzm_preset: u8,
    fast_boot_en: u8,
    active_img_prot: u8,
    fips_sha_sten: u8,
    fips_aes_sten: u8,
    fips_ecdsa_sten: u8,
    fips_drbg_sten: u8,
    fips_cmac_sten: u8,
    fips_kdf_sten: u8,
    dis_nxp_fw: u8,
}

#[inline(always)]
fn cmpa_secure_boot_cfg() -> CmpaSecureBootCfgDecode {
    let raw = load_cmpa_secure_boot_cfg();

    CmpaSecureBootCfgDecode {
        raw,
        sec_boot_en: (raw & 0x3) as u8,
        // bit 2 is a hole
        lp_sec_boot: ((raw >> 3) & 0x3) as u8,
        // bit 5 is a hole
        dice_csr_key_type: ((raw >> 6) & 0x3) as u8,
        enf_cnsa: ((raw >> 8) & 0x3) as u8,
        enf_tzm_preset: ((raw >> 10) & 0x3) as u8,
        fast_boot_en: ((raw >> 12) & 0x3) as u8,
        active_img_prot: ((raw >> 14) & 0x3) as u8,
        fips_sha_sten: ((raw >> 16) & 0x3) as u8,
        fips_aes_sten: ((raw >> 18) & 0x3) as u8,
        fips_ecdsa_sten: ((raw >> 20) & 0x3) as u8,
        fips_drbg_sten: ((raw >> 22) & 0x3) as u8,
        fips_cmac_sten: ((raw >> 24) & 0x3) as u8,
        fips_kdf_sten: ((raw >> 26) & 0x3) as u8,
        // bits 28-29 are reserved
        dis_nxp_fw: ((raw >> 30) & 0x3) as u8,
    }
}

// Additional CMPA constants
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpaUpdateConfigData {
    BootCfg0,
    BootCfg1,
    LSpiCfg0,
    CcSocuPin,
    CcSocuDflt,
    SecureBootCfg,
    RotkUsage,
    SblStartAddr,
    Rotkh,
    PqcRotkh,
}

impl CmpaUpdateConfigData {
    #[inline(always)]
    pub const fn start(self) -> u32 {
        match self {
            Self::BootCfg0 => IFRConfigAreaBase::Cmpa as u32,
            Self::BootCfg1 => IFRConfigAreaBase::Cmpa as u32 + 0x04,
            Self::LSpiCfg0 => IFRConfigAreaBase::Cmpa as u32 + 0x10,
            Self::CcSocuPin => IFRConfigAreaBase::Cmpa as u32 + 0x40,
            Self::CcSocuDflt => IFRConfigAreaBase::Cmpa as u32 + 0x44,
            Self::SecureBootCfg => IFRConfigAreaBase::Cmpa as u32 + 0x50,
            Self::RotkUsage => IFRConfigAreaBase::Cmpa as u32 + 0x54,
            Self::SblStartAddr => IFRConfigAreaBase::Cmpa as u32 + 0x58,
            Self::Rotkh => IFRConfigAreaBase::Cmpa as u32 + 0x60,
            Self::PqcRotkh => IFRConfigAreaBase::Cmpa as u32 + 0xC0,
        }
    }

    #[inline(always)]
    pub const fn byte_len(self) -> usize {
        const RKTH_WORDS: usize = 12;
        match self {
            Self::BootCfg0
            | Self::BootCfg1
            | Self::CcSocuPin
            | Self::CcSocuDflt
            | Self::SecureBootCfg
            | Self::RotkUsage
            | Self::SblStartAddr
            | Self::LSpiCfg0 => mem::size_of::<u32>(),
            Self::Rotkh | Self::PqcRotkh => RKTH_WORDS * mem::size_of::<u32>(),
        }
    }

    #[inline(always)]
    pub const fn word_len(self) -> usize {
        self.byte_len() / mem::size_of::<u32>()
    }

    #[inline(always)]
    pub const fn byte_offset(self) -> usize {
        match self {
            Self::BootCfg0 => 0x00,
            Self::BootCfg1 => 0x04,
            Self::LSpiCfg0 => 0x10,
            Self::CcSocuPin => 0x40,
            Self::CcSocuDflt => 0x44,
            Self::SecureBootCfg => 0x50,
            Self::RotkUsage => 0x54,
            Self::SblStartAddr => 0x58,
            Self::Rotkh => 0x60,
            Self::PqcRotkh => 0xC0,
        }
    }

    #[inline(always)]
    pub const fn byte_range(self) -> core::ops::Range<usize> {
        let start = self.byte_offset();
        start..(start + self.byte_len())
    }
}

/// Load 48 words (384 bits) of ECDSA RoTKH from CMPA, returning None if: CMPA appears corrupt/partially programmed (invalid header marker but not erased/ partially valid CMPA).
/// Current provisioning uses SHA-512, but retains the leftmost 48 bytes (384 bits).
pub fn load_rotkh_from_cmpa() -> Option<[u32; CmpaUpdateConfigData::Rotkh.word_len()]> {
    let region = CmpaUpdateConfigData::Rotkh;
    if !cmpa_header_marker_is_valid() && !is_cmpa_erased() {
        return None; // CMPA corrupt or partially provisioned — don't trust ROTKH
    }
    let mut buf = [0u32; CmpaUpdateConfigData::Rotkh.word_len()];
    for (i, slot) in buf.iter_mut().enumerate() {
        let addr = region.start() + (i as u32 * 4);
        *slot = unsafe { core::ptr::read_volatile(addr as *const u32) };
    }
    Some(buf)
}

/// Load 48 words (384 bits) of ML-DSA RoTKH from CMPA, returning None if: CMPA appears corrupt/partially programmed (invalid header marker but not erased/ partially valid CMPA).
/// Current provisioning uses SHA-512, but retains the leftmost 48 bytes (384 bits).
pub fn load_pqc_rotkh_from_cmpa() -> Option<[u32; CmpaUpdateConfigData::PqcRotkh.word_len()]> {
    let region = CmpaUpdateConfigData::PqcRotkh;
    // 384 bits ML-DSA-87 root key hash, left padded to 48 bytes like the ECDSA ROTKH
    if !cmpa_header_marker_is_valid() && !is_cmpa_erased() {
        return None; // CMPA corrupt or partially provisioned — don't trust ROTKH
    }
    let mut buf = [0u32; CmpaUpdateConfigData::PqcRotkh.word_len()];
    for (i, slot) in buf.iter_mut().enumerate() {
        let addr = region.start() + (i as u32 * 4);
        *slot = unsafe { core::ptr::read_volatile(addr as *const u32) };
    }
    Some(buf)
}

// CFPA offsets (MCXA) — CFG region @ 0x0100_0000
//
// 0x00 UPD_TYPE
// 0x04 UPD_PARAM0
// 0x08 UPD_PARAM1
// 0x0C UPD_PARAM2
// 0x10 Header word (marker + INV_LC + LC)
// 0x14 CFPA_PAGE_VERSION
// 0x18 IMAGE_KEY_REVOKE
// 0x1C DBG_REVOKE_VU
// 0x20.. FW version words
// 0x40 ROTK_REVOKE
// 0x50 ERR_AUTH_FAIL_COUNT
// 0x54 ERR_ITRC_COUNT

// The following CFPA fields are documented here for reference and can be localized when
// readers/writers are added for them:
// const CFPA_PAGE_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0014;
// const CFPA_DBG_REVOKE_VU: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x001C;
// const CFPA_EE0_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0020;
// const CFPA_EE1_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0024;
// const CFPA_EE2_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0028;
// const CFPA_EE3_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x002C;
// const CFPA_RECOVERY_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0034;
// const CFPA_UPDATE_SB3_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0038;
// const CFPA_LP_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x003C;

// Additional CMPA constants

#[inline(always)]
fn cfpa_header_word_is_valid(header: u32) -> bool {
    let marker = (header >> 16) as u16;
    if marker != CFPA_HEADER_MARKER {
        return false;
    }

    let lifecycle = (header & 0xFF) as u8;
    let inv_lifecycle = ((header >> 8) & 0xFF) as u8;
    inv_lifecycle == (!lifecycle)
}

pub fn load_cfpa_header_word_raw() -> u32 {
    const CFPA_HEADER: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0010;
    unsafe { core::ptr::read_volatile(CFPA_HEADER as *const u32) }
}

/// Load CFPA header word and check validity, returning None if header is invalid (e.g. incorrect marker, which could indicate unprovisioned/partially provisioned state or corruption). This is used as a prerequisite check for other CFPA fields since the header validity is an indicator of whether the CFPA contents can be trusted.
#[inline(always)]
pub fn load_cfpa_header_word() -> Option<u32> {
    let h = load_cfpa_header_word_raw();
    cfpa_header_word_is_valid(h).then_some(h)
}

/// The exact CFPA header word for a given lifecycle state:
/// marker | !lc | lc. Derived, not a magic literal.
const fn cfpa_header_for(lc: NbootLifecycleState) -> u32 {
    let lc = lc as u32 & 0xFF;
    ((CFPA_HEADER_MARKER as u32) << 16) | ((!lc & 0xFF) << 8) | lc
}

#[inline(always)]
fn load_cfpa_word(address: u32) -> Option<u32> {
    load_cfpa_header_word()?;
    Some(unsafe { core::ptr::read_volatile(address as *const u32) })
}

/// Load lifecycle state function: reads the image key revocation word from CFPA.
/// If both CFPA and CMPA are un-provisioned, returns Some(0) — no revocation (permissive for auth).
pub fn load_image_key_revocation_from_cfpa() -> Option<u32> {
    const CFPA_IMAGE_KEY_REVOKE: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0018;
    if is_cfpa_erased() && is_cmpa_erased() {
        return Some(0);
    }
    let word = load_cfpa_word(CFPA_IMAGE_KEY_REVOKE)?;
    if word == ERASED_WORD {
        return None; // Could mean corrupt/ partially provisioned CFPA, should not be trusted for auth decisions.
    }
    Some(word)
}

#[inline(always)]
fn load_cfpa_rotk_revoke_word() -> Option<u32> {
    const CFPA_ROTK_REVOKE: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0040;
    let word = load_cfpa_word(CFPA_ROTK_REVOKE)?;
    if word == ERASED_WORD {
        return None;
    }
    Some(word)
}

/// Load lifecycle state function: reads the root key revocation words from CFPA. Returns a [NbootRootKeyRevocation; 4] array representing the revocation state of each root key,
/// or None if the CFPA header is invalid or the ROTK_REVOKE word is erased (indicating unprovisioned/partially provisioned state that should not be trusted).
/// If both CFPA and CMPA are un-provisioned, returns Some([Enabled; 4]) — all keys enabled (permissive for auth).
/// This is already decoded into the ROM-facing `NbootRootKeyRevocation` values (`Enabled`/`Revoked`), not the raw 2-bit `RoTKx_EN` CFPA field encodings.
pub fn load_root_key_revocation_from_cfpa() -> Option<[NbootRootKeyRevocation; 4]> {
    if is_cfpa_erased() && is_cmpa_erased() {
        return Some([NbootRootKeyRevocation::Enabled; 4]);
    }
    let word = load_cfpa_rotk_revoke_word()?;
    Some(root_key_revocation_from_rotk_revoke_word(word))
}

// CFPA ROTK_REVOKE (word @ 0x40) bit layout (per user-provided breakdown):
//
// 31:30 ISP_ACTIVE_IMG
// 29    DICE_UPD_ALIAS_CERT
// 28    DICE_UPD_ALIAS_KEY
// 27:8  Reserved
//  7:6  RoTK3_EN (2 bits)
//  5:4  RoTK2_EN (2 bits)
//  3:2  RoTK1_EN (2 bits)
//  1:0  RoTK0_EN (2 bits)

#[inline(always)]
fn rotk_en_fields_from_rotk_revoke_word(word: u32) -> [u8; 4] {
    [
        (word & 0x3) as u8,
        ((word >> 2) & 0x3) as u8,
        ((word >> 4) & 0x3) as u8,
        ((word >> 6) & 0x3) as u8,
    ]
}

#[inline(always)]
fn root_key_revocation_from_rotk_revoke_word(word: u32) -> [NbootRootKeyRevocation; 4] {
    // NBOOT `soc_rootKeyRevocation[]` uses a per-key revoke/enable constant (`0xAA`/`0xBB`), not the raw CFPA two-bit field values.
    // The CFPA ROTK_REVOKE word encodes the revocation state for each root key in 2 bits, where
    //   0b00/0b01 => enabled (not revoked)
    //   0b10/0b11 => revoked
    let mut revocation = [NbootRootKeyRevocation::Enabled; 4];
    for (i, en2) in rotk_en_fields_from_rotk_revoke_word(word).iter().copied().enumerate() {
        revocation[i] = match en2 & 0x3 {
            0 | 1 => NbootRootKeyRevocation::Enabled,
            2 | 3 => NbootRootKeyRevocation::Revoked,
            _ => NbootRootKeyRevocation::Enabled,
        };
    }
    revocation
}

/// The following three functions load DICE and ISP configuration bits from the CFPA ROTK_REVOKE word. These functions return None if the CFPA header or ROTK_REVOKE word is invalid/unprovisioned, and otherwise return the decoded bool as option.
#[inline(always)]
pub fn load_dice_upd_alias_key_from_cfpa() -> Option<bool> {
    let word = load_cfpa_rotk_revoke_word()?;
    Some(((word >> 28) & 1) != 0)
}

#[inline(always)]
pub fn load_dice_upd_alias_cert_from_cfpa() -> Option<bool> {
    let word = load_cfpa_rotk_revoke_word()?;
    Some(((word >> 29) & 1) != 0)
}

#[inline(always)]
pub fn load_isp_active_img_from_cfpa() -> Option<u8> {
    let word = load_cfpa_rotk_revoke_word()?;
    Some(((word >> 30) & 0x3) as u8)
}

/// Load firmware version from CFPA EE0_FW_VERSION word. Returns None if CFPA header is invalid or the EE0_FW_VERSION word is erased (indicating unprovisioned/partially provisioned state that should not be trusted).
/// If both CFPA and CMPA are un-provisioned, returns Some(0) — version 0 allows any image version through.
pub fn load_firmware_version_from_cfpa() -> Option<u32> {
    const CFPA_EE0_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0020;
    if is_cfpa_erased() && is_cmpa_erased() {
        return Some(0);
    }
    // Use the EE0 firmware version slot so verification matches the image version field we
    // expect to advance for the active execution environment.
    let word = load_cfpa_word(CFPA_EE0_FW_VERSION)?;
    if word == ERASED_WORD {
        return None;
    }
    Some(word)
}

/// Load lifecycle state from CFPA header word: returns the decoded lifecycle state if the header is valid, or None if the header is invalid (e.g. incorrect marker, which could indicate unprovisioned/partially provisioned state or corruption). This is used as a prerequisite check for other CFPA fields since the header validity is an indicator of whether the CFPA contents can be trusted.
/// Returns the decoded lifecylce to be used by the ROM API NBOOT functions, which uses different format that what is encoded in CFPA.
/// The CFPA header encodes lifecycle in the lowest byte, with a separate inverted lifecycle byte as a validity check, and a 2 byte header marker in upper half of the word.
/// The NBOOT ROM API expects a full 32-bit raw value where the lower half is the lifecycle raw value and the upper half is the !inverse. Even in a brand new unit, LC should be Develop.
pub fn load_lifecycle_from_cfpa() -> Option<NbootLifecycleState> {
    let header = load_cfpa_header_word()?;
    NbootLifecycleDiscriminator::from_raw(header as u8).map(NbootLifecycleDiscriminator::state)
}

/// Helper function that returns true if the lifecycle is advanced past develop2 (i.e. the device is "fused" and no IFR changes are permitted). Returns None if the CFPA header is invalid (e.g. unprovisioned/partially provisioned state).
pub fn is_sam_fused() -> Option<bool> {
    let current_lifecycle = load_lifecycle_from_cfpa()?;
    Some(current_lifecycle != NbootLifecycleState::Develop && current_lifecycle != NbootLifecycleState::Develop2)
}

/// Load key usage NbootRootKeyUsage for all four key sets from CMPA.RoTK_USAGE word, returning None if CMPA header is invalid or the RoTK_USAGE word is erased (indicating unprovisioned/partially provisioned state that should not be trusted). The mapping from the 3-bit usage fields in CMPA to the NbootRootKeyUsage enum is based on the reference table provided by the user.
/// Note that the usage applies acroess ECDSA and ML-DSA root keys, so the same usage value applies to both the ROTKH and PQC_ROTKH for each key set.
pub fn load_rotk_usage_from_cmpa() -> Option<[NbootRootKeyUsage; 4]> {
    let word = cmpa_rotk_usage_word_checked()?;
    fn map(bits: u32) -> NbootRootKeyUsage {
        match bits & 0x7 {
            0 => NbootRootKeyUsage::All,
            1 => NbootRootKeyUsage::DebugCa,
            2 => NbootRootKeyUsage::ImageCaFwCa,
            3 => NbootRootKeyUsage::DebugCaImageCaFwCa,
            4 => NbootRootKeyUsage::ImageKeyFwKey,
            5 => NbootRootKeyUsage::ImageKey,
            6 => NbootRootKeyUsage::FwKey,
            _ => NbootRootKeyUsage::Unused,
        }
    }
    let rotk0_usage = map(word & 0x7);
    let rotk1_usage = map((word >> 3) & 0x7);
    let rotk2_usage = map((word >> 6) & 0x7);
    let rotk3_usage = map((word >> 9) & 0x7);
    Some([rotk0_usage, rotk1_usage, rotk2_usage, rotk3_usage])
}

// CMPA.RoTK_USAGE bit layout (MCXA reference manual):
//
// [2:0]   RoTK0_Usage
// [5:3]   RoTK1_Usage
// [8:6]   RoTK2_Usage
// [11:9]  RoTK3_Usage
// [12]    SKIP_DICE
// [13]    DICE_INC_NXP_CFG
// [14]    DICE_INC_CUST_CFG
// [15]    DICE_INC_NXP_FIELD_CFG
// [31:16] Reserved

#[inline(always)]
fn cmpa_rotk_usage_word_checked() -> Option<u32> {
    const CMPA_ROTK_USAGE: u32 = IFRConfigAreaBase::Cmpa as u32 + 0x0054; // 0x0100_0254

    // If un-provisioned, the CMPA maybe left in an erased state (all 0xFF). If both CFPA and CMPA are erased, treat as "all keys allowed" (permissive for FIRST boot after off chip flashing).
    if is_cfpa_erased() && is_cmpa_erased() {
        return Some(0x0000_0000); // All four RoTKx_Usage = 0 => NbootRootKeyUsage::All
    }
    if !cmpa_header_marker_is_valid() {
        return None;
    }
    let word = unsafe { core::ptr::read_volatile(CMPA_ROTK_USAGE as *const u32) };
    if word == ERASED_WORD {
        return None;
    }
    Some(word)
}

/// CMPA.RoTK_USAGE bit 12 (SKIP_DICE)
pub fn load_dice_skip_from_cmpa() -> bool {
    // If CMPA isn't valid, default to "do not skip DICE" (safer).
    cmpa_rotk_usage_word_checked()
        .map(|word| ((word >> 12) & 1) != 0)
        .unwrap_or(false)
}

/// CMPA.RoTK_USAGE bit 13 (DICE_INC_NXP_CFG)
pub fn load_dice_inc_nxp_cfg_from_cmpa() -> bool {
    cmpa_rotk_usage_word_checked()
        .map(|word| ((word >> 13) & 1) != 0)
        .unwrap_or(false)
}

/// CMPA.RoTK_USAGE bit 14 (DICE_INC_CUST_CFG)
pub fn load_dice_inc_cust_cfg_from_cmpa() -> bool {
    cmpa_rotk_usage_word_checked()
        .map(|word| ((word >> 14) & 1) != 0)
        .unwrap_or(false)
}

/// CMPA.RoTK_USAGE bit 15 (DICE_INC_NXP_FIELD_CFG)
pub fn load_dice_inc_nxp_field_cfg_from_cmpa() -> bool {
    cmpa_rotk_usage_word_checked()
        .map(|word| ((word >> 15) & 1) != 0)
        .unwrap_or(false)
}
