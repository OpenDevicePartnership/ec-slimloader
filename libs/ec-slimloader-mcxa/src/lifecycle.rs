use core::mem;

use crate::rom_api::{NbootLifecycleDiscriminator, NbootLifecycleState, NbootRootKeyRevocation, NbootRootKeyUsage};

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
// SCRATCH (write staging):
//   CFPA  0x0100_2000 - 0x0100_21FF
//   CMPA  0x0100_2200 - 0x0100_23FF
//   CMPA customer-defined 0x0100_2400 - 0x0100_37FF

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

pub fn is_cmpa_erased() -> bool {
    // NOTE: ifr_verify_erase_page is NOT a read-only check; it erases then verifies (destructive).
    // Therefore it cannot be used to check erased state of CFG area (protected by ROM).
    // read_volatile is the correct approach for a non-destructive erased check.
    let base = IFRConfigAreaBase::Cmpa as u32;
    let word_count = IFRPage::Cmpa.byte_len() / core::mem::size_of::<u32>();
    for i in 0..word_count {
        let addr = base + (i as u32 * 4);
        let val = unsafe { core::ptr::read_volatile(addr as *const u32) };
        if val != ERASED_WORD {
            return false;
        }
    }
    true
}

#[inline(always)]
fn load_cmpa_header_marker() -> u16 {
    let word = load_cmpa_boot_cfg0();
    (word >> 16) as u16
}

#[inline(always)]
pub fn cmpa_header_marker_is_valid() -> bool {
    // CMPA BOOT_CFG0 header marker semantics (MCXA):
    // Marker should be set to 0x5963. After this header is set, all non-zero values will take effect;
    // leaving all settings at 0xFF will cause undefined behavior. It is recommended to set all values
    // to 0x00 before setting the CMPA header marker.
    //
    // Layout assumed consistent with CFPA header marker usage: marker stored in bits [31:16].
    const CMPA_HEADER_MARKER: u16 = 0x5963;
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecureBootLevel {
    AllAllowed = 0,     // b00
    CrcOrSigned = 1,    // b01
    SignedOnly = 2,     // b10 (CMAC or ECDSA)
    EcdsaMldsaOnly = 3, // b11
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

pub fn secure_boot_enforced() -> bool {
    // If CMPA is erased/unprovisioned, SEC_BOOT_EN bits will read back as 0b11 (all ones)
    // and would incorrectly look like "enforced". Treat erased CMPA as "not enforced".
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && matches!(secure_boot_level(), SecureBootLevel::EcdsaMldsaOnly)
}

pub fn cnsa_enforced() -> bool {
    let cnsa_level = match cmpa_secure_boot_cfg().enf_cnsa {
        0 => CnsaLevel::NotEnforced,
        1 => CnsaLevel::CnsaOne,
        2 | 3 => CnsaLevel::CnsaTwo,
        _ => CnsaLevel::NotEnforced,
    };
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cnsa_level == CnsaLevel::CnsaTwo
}

pub fn fast_boot_enabled() -> bool {
    // Fast boot is enabled when FAST_BOOT_EN field is 0b00, and disabled otherwise (full auth flow required)
    !is_cmpa_erased() && cmpa_header_marker_is_valid() && cmpa_secure_boot_cfg().fast_boot_en == 0
}

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
        sec_boot_en: ((raw >> 0) & 0x3) as u8,
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
            Self::BootCfg0 | Self::BootCfg1 | Self::SecureBootCfg | Self::RotkUsage | Self::SblStartAddr => {
                mem::size_of::<u32>()
            }
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

pub fn load_rotkh_from_cmpa() -> Option<[u32; CmpaUpdateConfigData::Rotkh.word_len()]> {
    let region = CmpaUpdateConfigData::Rotkh;
    if !cmpa_header_marker_is_valid() {
        // If secure boot is not enforced, CMPA may be left unprovisioned (invalid header).
        // Still allow reading the words so higher-level logic can use the image RKTH as the
        // source of truth while warning on mismatch.
        // Note: an erased CMPA (all 0xFF) will decode SEC_BOOT_EN as 0b11, so treat "erased"
        // as unprovisioned even if `secure_boot_enforced()` appears true.
        if secure_boot_enforced() && !is_cmpa_erased() {
            return None;
        }
    }
    let mut buf = [0u32; CmpaUpdateConfigData::Rotkh.word_len()];
    for i in 0..region.word_len() {
        let addr = region.start() + (i as u32 * 4);
        buf[i] = unsafe { core::ptr::read_volatile(addr as *const u32) };
    }
    Some(buf)
}

pub fn load_pqc_rotkh_from_cmpa() -> Option<[u32; CmpaUpdateConfigData::PqcRotkh.word_len()]> {
    let region = CmpaUpdateConfigData::PqcRotkh;
    // 384 bits ML-DSA-87 root key hash, left padded to 48 bytes like the ECDSA ROTKH
    if !cmpa_header_marker_is_valid() {
        if secure_boot_enforced() && !is_cmpa_erased() {
            return None;
        }
    }
    let mut buf = [0u32; CmpaUpdateConfigData::PqcRotkh.word_len()];
    for i in 0..region.word_len() {
        let addr = region.start() + (i as u32 * 4);
        buf[i] = unsafe { core::ptr::read_volatile(addr as *const u32) };
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
    const CFPA_HEADER_MARKER: u16 = 0x9635;
    let marker = (header >> 16) as u16;
    if marker != CFPA_HEADER_MARKER {
        return false;
    }

    let lifecycle = (header & 0xFF) as u8;
    let inv_lifecycle = ((header >> 8) & 0xFF) as u8;
    inv_lifecycle == (!lifecycle)
}

#[inline(always)]
pub fn load_cfpa_header_word() -> Option<u32> {
    const CFPA_HEADER: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0010;
    let header = unsafe { core::ptr::read_volatile(CFPA_HEADER as *const u32) };
    if !cfpa_header_word_is_valid(header) {
        return None;
    }

    Some(header)
}

// Any field reading back this value should be treated as "not provisioned" rather than a real configuration.
const ERASED_WORD: u32 = 0xFFFF_FFFF;

#[inline(always)]
fn load_cfpa_word(address: u32) -> Option<u32> {
    load_cfpa_header_word()?;
    Some(unsafe { core::ptr::read_volatile(address as *const u32) })
}

// Load lifecycle state functions
pub fn load_image_key_revocation_from_cfpa() -> Option<u32> {
    const CFPA_IMAGE_KEY_REVOKE: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0018;
    let word = load_cfpa_word(CFPA_IMAGE_KEY_REVOKE)?;
    if word == ERASED_WORD {
        return None;
    } // Erased state means don't trust.
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

pub fn load_root_key_revocation_from_cfpa() -> Option<[NbootRootKeyRevocation; 4]> {
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
        ((word >> 0) & 0x3) as u8,
        ((word >> 2) & 0x3) as u8,
        ((word >> 4) & 0x3) as u8,
        ((word >> 6) & 0x3) as u8,
    ]
}

#[inline(always)]
fn root_key_revocation_from_rotk_revoke_word(word: u32) -> [NbootRootKeyRevocation; 4] {
    // NBOOT `soc_rootKeyRevocation[]` uses a per-key revoke/enable constant. The CFPA ROTK_REVOKE word encodes the revocation state for each root key in 2 bits, where
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

pub fn load_firmware_version_from_cfpa() -> Option<u32> {
    const CFPA_EE0_FW_VERSION: u32 = IFRConfigAreaBase::Cfpa as u32 + 0x0020;
    // Use the EE0 firmware version slot so verification matches the image version field we
    // expect to advance for the active execution environment.
    let word = load_cfpa_word(CFPA_EE0_FW_VERSION)?;
    if word == ERASED_WORD {
        return None;
    }
    Some(word)
}

pub fn load_lifecycle_from_cfpa() -> Option<NbootLifecycleState> {
    let header = load_cfpa_header_word()?;
    NbootLifecycleDiscriminator::from_raw(header as u8).map(NbootLifecycleDiscriminator::state)
}

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
    let rotk0_usage = map((word >> 0) & 0x7);
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

/// Decode a raw lifecycle value into the typed NBOOT lifecycle state.
pub fn decode_lifecycle(raw_value: u32) -> NbootLifecycleState {
    NbootLifecycleState::from_any_raw(raw_value).unwrap_or(NbootLifecycleState::Develop)
}
