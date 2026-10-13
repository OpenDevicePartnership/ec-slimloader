#![no_std]

//! Data layout for the IFR CFPA & CMPA

use core::ops::{Index, IndexMut};

pub use arbitrary_int::{u3, u4, u5, u7, u10, u24};
use bitbybit::{bitenum, bitfield};

#[repr(C)]
#[derive(Debug, Clone)]
pub struct IFR {
    pub update: Update,
    pub cfpa: CFPA,
    pub cmpa: CMPA,
}

impl IFR {
    pub const DEVCFG_ADDR: u32 = 0x0100_0000;
    pub const SCRATCH_ADDR: u32 = 0x0100_2000;
    pub const SECURE_DEVCFG_ADDR: u32 = 0x1100_0000;
    pub const SECURE_SCRATCH_ADDR: u32 = 0x1100_2000;

    /// Get the current DEVCFG IFR values directly from the flash
    ///
    /// # Safety
    ///
    /// Only sound to call when on an MCXA5xx device
    pub unsafe fn current() -> &'static IFR {
        unsafe { &*(Update::DEVCFG_ADDR as *const IFR) }
    }

    /// Get the current secure DEVCFG IFR values directly from the flash
    ///
    /// # Safety
    ///
    /// Only sound to call when on an MCXA5xx device
    pub unsafe fn secure_current() -> &'static IFR {
        unsafe { &*(Update::SECURE_DEVCFG_ADDR as *const IFR) }
    }

    const _SIZE_CHECK: () = const {
        if core::mem::size_of::<Update>() as u32 != CFPA::DEVCFG_ADDR - Update::DEVCFG_ADDR {
            panic!("Update has wrong size");
        }
        if core::mem::size_of::<CFPA>() as u32 != CMPA::DEVCFG_ADDR - CFPA::DEVCFG_ADDR {
            panic!("CFPA has wrong size");
        }
        if core::mem::size_of::<CMPA>() as u32 != 0x200 {
            panic!("CMPA has wrong size");
        }
        if core::mem::size_of::<Self>() as u32 != 0x400 {
            panic!("IFR has wrong size");
        }
    };

    pub fn as_array(&self) -> &[u8; core::mem::size_of::<Self>()] {
        // Safety: We're a repr(C) struct with no padding bytes
        unsafe { core::mem::transmute(self) }
    }

    /// Get an all-zero IFR instance
    pub fn zeroed() -> Self {
        // Safety: All 0's is a valid
        unsafe { core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Update {
    /// Device configuration update request type.
    pub devcfg_upd_type: DevcfgUpdType,
    pub _reserved: [u32; 3],
}

impl Update {
    pub const DEVCFG_ADDR: u32 = 0x0100_0000;
    pub const SCRATCH_ADDR: u32 = 0x0100_2000;
    pub const SECURE_DEVCFG_ADDR: u32 = 0x1100_0000;
    pub const SECURE_SCRATCH_ADDR: u32 = 0x1100_2000;
}

#[bitfield(u32, debug, default = 0)]
pub struct DevcfgUpdType {
    #[bits(0..=31, rw)]
    pub devcfg_upd_type: Option<UpdateType>,
}

#[bitenum(u32)]
#[derive(Debug)]
#[repr(u32)]
pub enum UpdateType {
    CfpaUpdated = 0x55504446,
    CmpaUpdated = 0x5550444D,
    Image0Updated = 0x494D4730,
    Image1Updated = 0x494D4731,
    NfpaUpdated = 0x4E465041,
    /// Not a valid variant
    Erased = 0xFFFF_FFFF,
    Zeroed = 0,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CFPA {
    pub header: Header,
    /// CFPA page version.
    /// ROM enforces monotonic increment check during CFPA-CMAC page update. If the new counter value is less than the value in active CFPA page,
    /// then the update is rejected and CMAC signing is skipped. CFPA_PAGE_VERSION may begin at 0x1 and user may manually increment the value by 1 or use value 0xFF_FFFF and ROM will auto-increment this field.
    pub cfpa_page_version: u32,
    /// Image signing key revocation counter.
    /// This monotonically increasing unary encode counter (e.g., 0x0, 0x1, 0x3, 0x7, 0xF...) field is used by boot ROM to enforce image signing key (ISK) certificate revocation policy.
    /// Boot ROM accepts an ISK certificate as valid only if the "constraint" field in ISK certificate is the same or greater than the ISK revocation counter.
    /// The 32-bit ISK revocation counter is formed using 8 bits of IMAGE_KEY_REVOKE[7:0] field in OTP as MSB bits and lower 24 bits from this field.
    /// ISK certificate validation is done as part of boot, FW update (SB3) and recovery (SB3) image authentication.
    pub image_key_revoke: u32,
    pub dbg_revoke_vu: DbgRevokeVu,
    /// Secure Firmware version. Rollback counter for Execution Environment 0 firmware (Monotonic counter) - CPU0 primary image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee0_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 1 firmware (Monotonic counter) - eg.,  CPU0 NS image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee1_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 2 firmware (Monotonic counter)  - eg., CPU1  secure image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee2_fw_version: u32,
    /// Secure Firmware version. Rollback counter for Execution Environment 3 firmware (Monotonic counter)  - eg., CPU1 NS image low-power wake image
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub ee3_fw_version: u32,
    /// NBU Firmware version. FMC_SBL firmware version (Monotonic counter)
    ///
    /// This monotonic counter field can be used to track the firmware version of NBU partition.
    /// An antirollback check can be enforced during update by using this field and "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_NBU' counter ID parameter, in SB3 file.
    pub fmc_sbl_fw_version: u32,
    /// Secure Firmware version. Rollback counter for SB3 file used for recovery (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub recovery_sb3_version: u32,
    /// Secure Firmware version. Rollback counter for SB3 file used for FW update (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub update_sb3_version: u32,
    /// Secure Firmware version. Rollback counter for low-power wake image (Monotonic counter)
    ///
    /// This monotonic counter field tracks the firmware version of secure partition.
    /// ROM uses this field to enforce anti-rollback checking during secure boot and secure update (SB3 header).  
    /// For image authentication pass during secure boot the FW version in image manifest must be equal or greater than this value.
    /// During secure update & recovery boot the version number in SB3 header Must be equal or greater than the value in this field.
    /// Apart from SB3 header check, user can enforce anti-rollback check during SB3 processing by using "kSB3_COMMAND_fwVersionCheck" SB command with 'kNBOOT_CNT_secure' counter ID parameter, in SB3 file.
    pub lp_fw_version: u32,
    pub rotk_revoke: RotkRevoke,
    pub _reserved0: [u32; 3],
    /// Authentication failure counter.
    ///
    /// This monotonic counter field is incremented by boot ROM on authentication failure during boot, SB3, or debug authentication (Monotonic counter).
    pub err_auth_fail_count: u32,
    /// Tamper event counter.
    ///
    /// This monotonic counter field is incremented by boot ROM whenever the reset cause during boot is detected as
    /// - ITRC reset caused by security sensors or
    /// - WDT 0/1 reset or
    /// - Tamper pin reset.
    pub err_itrc_count: u32,
    pub _reserved1: [u32; 2],
    /// Monotonic erase counter for IPED region 0 through 7.
    ///  
    /// This value is used by bootloader to dynamically compute region IV.
    /// This counter will increment by one, during each erase cycle of the corresponding flash region.
    /// User should not write anything in this field. This field is entirely handled by ROM.
    ///
    /// Final IV value for given region used by IPED for encryption/decryption is computed by ROM bootloader and incorporates device UUID, IPED region number and MCTR_IPED_CTXn.
    /// Application should always use ROM APIs to erase whole Prince region to keep IV consistent.
    pub mctr_iped_ctx: [u32; 8],
    /// Monotonic counter for application use.
    ///
    /// ROM enforces monotonic increment check during CFPA-CMAC page update.
    /// If the new counter value is less than the value in active CFPA page, then the update is rejected and CMAC signing is skipped.
    pub mctr_cust_ctr: [u32; 8],
    /// Monotonic flags for application use.
    ///
    /// Once a bit is set in this field it should be set on sub-sequent updates of the page. ROM emulates One Time Programmable (OTP) bits behavior during CFPA-CMAC update.
    /// Compared to current value, if the new value has bit cleared, then the update is rejected and CMAC signing is skipped.
    pub mflag_cust: [u32; 8],
    pub flash_acl: [FlashAcl; 8],
    pub _reserved2: [u32; 8],
    /// CMAC of hash (SHA384/256) of authenticated image manifest.
    pub sbl_img0_cmac_cache: ReverseArray<u32, 4>,
    /// CMAC of hash (SHA384/256) of authenticated image manifest.
    pub sbl_img1_cmac_cache: ReverseArray<u32, 4>,
    pub _reserved3: [u32; 4],
    /// Vector address when waking from power-down states when CMPA.LP_SEC_BOOT is set to 2b'10.
    pub lp_vector_addr: u32,
    pub _reserved4: [u32; 23],
    /// Additional Authentication Data for IPED context
    pub iped_gcm_aad_ctx: [u32; 8],
    pub _reserved5: [u32; 20],
}

impl CFPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0010;
    pub const SCRATCH_ADDR: u32 = 0x0100_2010;
    pub const SECURE_DEVCFG_ADDR: u32 = 0x1100_0010;
    pub const SECURE_SCRATCH_ADDR: u32 = 0x1100_2010;

    pub const ZERO: Self = Self {
        header: Header::ZERO,
        cfpa_page_version: 0,
        image_key_revoke: 0,
        dbg_revoke_vu: DbgRevokeVu::ZERO,
        ee0_fw_version: 0,
        ee1_fw_version: 0,
        ee2_fw_version: 0,
        ee3_fw_version: 0,
        fmc_sbl_fw_version: 0,
        recovery_sb3_version: 0,
        update_sb3_version: 0,
        lp_fw_version: 0,
        rotk_revoke: RotkRevoke::ZERO,
        _reserved0: [0; _],
        err_auth_fail_count: 0,
        err_itrc_count: 0,
        _reserved1: [0; _],
        mctr_iped_ctx: [0; _],
        mctr_cust_ctr: [0; _],
        mflag_cust: [0; _],
        flash_acl: [FlashAcl::ZERO; _],
        _reserved2: [0; _],
        sbl_img0_cmac_cache: ReverseArray::new([0; _]),
        sbl_img1_cmac_cache: ReverseArray::new([0; _]),
        _reserved3: [0; _],
        lp_vector_addr: 0,
        _reserved4: [0; _],
        iped_gcm_aad_ctx: [0; _],
        _reserved5: [0; _],
    };

    pub fn is_erased(&self) -> bool {
        self.cfpa_page_version == u32::MAX
    }

    pub fn as_array(&self) -> &[u8; core::mem::size_of::<Self>()] {
        // Safety: We're a repr(C) struct with no padding bytes
        unsafe { core::mem::transmute(self) }
    }
}

#[bitfield(u32, debug)]
pub struct Header {
    /// CFPA Header marker should be set to 0x9635. After this header is set, all non-zero values will take effect; leaving all values set to 0xff will cause undefined behavior. It is recommended to set all values to 0x00 before setting the CFPA header.
    #[bits(16..=31, rw)]
    pub marker: u16,
    /// Inverted value of CFPA_LC_STATE.
    /// This INV_CPFA_LC_STATE ^ CFPA_LC_STATE[7:0] should be 0xFF, otherwise the CPFA_LC_STATE will not be valid.
    #[bits(8..=15, rw)]
    pub inv_cfpa_lc_state: Option<InvLifeCycleState>,
    /// Life cycle state.
    /// This field is used to advance life cycle state to OEM LC states (Develop, Develop2, In-Field, In-field locked, Bricked, FA).
    #[bits(0..=7, rw)]
    pub cfpa_lc_state: Option<LifeCycleState>,
}

/// Below are the allowed values for this field. Use of other values may lead to bricked state.  
#[bitenum(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum LifeCycleState {
    Develop = 0x03,
    Develop2 = 0x07,
    InField = 0x0F,
    InFieldLocked = 0xCF,
    OemFieldReturn = 0x1F,
    FailureAnalysis = 0xA5,
    Bricked = 0x5A,
}

impl LifeCycleState {
    /// Returns a monotonic rank for forward-only progression checks.
    /// Higher rank = further along the lifecycle.
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

    /// Get the [InvLifeCycleState] for this state
    pub const fn inverse(self) -> InvLifeCycleState {
        match self {
            LifeCycleState::Develop => InvLifeCycleState::Develop,
            LifeCycleState::Develop2 => InvLifeCycleState::Develop2,
            LifeCycleState::InField => InvLifeCycleState::InField,
            LifeCycleState::InFieldLocked => InvLifeCycleState::InFieldLocked,
            LifeCycleState::OemFieldReturn => InvLifeCycleState::OemFieldReturn,
            LifeCycleState::FailureAnalysis => InvLifeCycleState::FailureAnalysis,
            LifeCycleState::Bricked => InvLifeCycleState::Bricked,
        }
    }
}

#[bitenum(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum InvLifeCycleState {
    Develop = 0xFC,
    Develop2 = 0xF8,
    InField = 0xF0,
    InFieldLocked = 0x30,
    OemFieldReturn = 0xE0,
    FailureAnalysis = 0x5A,
    Bricked = 0xA5,
}

#[bitfield(u32, debug, default = 0)]
pub struct DbgRevokeVu {
    /// Debug certificate revocation counter.
    /// This monotonic counter field is used for revoking debug certificates.
    /// As part of debug authentication, CC_VU field in debug certificate/credential is checked against DCFG_VENDOR_USAGE value.
    /// The 32-bit DCFG_VENDOR_USAGE is formed using upper 16 bit of CMPA.VENDOR_USAGE and lower 16 bits of this field.  
    #[bits(0..=15, rw)]
    pub debug_certificate_revocation_counter: u16,
}

#[bitfield(u32, debug, default = 0)]
pub struct RotkRevoke {
    /// This field is managed by ROM and user shouldn't set this field. ROM during update of IMGx_CMAC updates this field.
    #[bits(30..=31, r)]
    pub isp_activ_img: Option<IspActivImg>,
    /// Generate Alias key certificate during boot. This option is only applicable if DICE_ALIAS_KEY_UPD is set.
    ///
    /// `false` - Certificate is not generated
    /// `true` - Certificate is generated
    ///
    /// Note, applications should clear this field after initial certificate generation to reduce boot time.
    #[bit(29, rw)]
    pub dice_upd_alias_cert: bool,
    /// Generate alias key pair during boot in addition to CDI.
    /// 0 - Alias key pair is not generated
    /// 1 - Alias key pair is generate
    #[bit(28, rw)]
    pub dice_upd_alias_key: bool,

    /// RoT Key enable. Determines if ROTK can be used during secure boot.
    #[bits(0..=1, rw, stride = 2)]
    pub rotk_en: [RotkEn; 4],
}

/// Active image indicator used in ISP mode to manage SWAP enable before reciev-sb command.
#[bitenum(u2)]
#[derive(Debug)]
pub enum IspActivImg {
    SwapIsDisabled = 0b00,
    SwapIsEnabled = 0b01,
    SblImageI = 0b10,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum RotkEn {
    Enabled1 = 0b00,
    Enabled2 = 0b01,
    Revoked1 = 0b10,
    Revoked2 = 0b11,
}

/// Select one of the 8 pre-defined access control attributes for the given sector. Access attributes control read, write and execute access along with sticky lock protection.
///
/// After a locked access level is selected, the sub-sequent updates of this field can be done with higher lock level only.
/// - If current sector ACL value (GLBACn index) is greater than the new value, then it is permitted except if the new value is 4 or 5.
///   - 7 (___L) > 6 (__XL) > 3 (R__L) > 2 (R_XL) > 1 (RW_L) > 0 (RWX_)
/// - If current sector ACL value is 0, 4, or 5 then any new value is permitted.
#[bitfield(u32, debug, default = 0)]
pub struct FlashAcl {
    #[bits(0..=2, rw, stride = 4)]
    pub acl_sec: [AclSec; 8],
}

#[bitenum(u3, exhaustive = true)]
#[derive(Debug)]
pub enum AclSec {
    /// Default flash memory behavior: R/W unlocked
    Default = 0,
    /// Data flash memory with this setting: R/W + locked
    Data = 1,
    /// ROM with this setting: RX + locked
    RomData = 2,
    /// Data read-only memory (DROM) with this setting: ROM + locked
    DataReadOnly = 3,
    /// ROM with this setting: RX unlocked
    Rom = 4,
    /// XOM with this setting: XOM unlocked
    Xom = 5,
    /// XOM with this setting: XOM + locked
    XomData = 6,
    /// Hidden (no access + locked)
    Hidden = 7,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CMPA {
    pub boot_cfg0: BootCfg0,
    pub boot_cfg1: BootCfg1,
    pub boot_led_status: BootLedStatus,
    pub boot_timers: BootTimers,
    pub lspi_qflash_cfg0: LspiQflashCfg0,
    pub lspi_qflash_cfg1: LspiQflashCfg1,
    pub lspi_flash_cfg0: u32, // Reserved
    pub lspi_flash_cfg1: u32, // TODO
    pub isp_uart_cfg: u32,    // TODO
    pub isp_i2c_cfg: u32,     // TODO
    pub isp_can_cfg: u32,     // TODO
    pub isp_spi_cfg0: u32,    // TODO
    pub isp_spi_cfg1: u32,    // TODO
    pub isp_usb_id: u32,      // TODO
    pub isp_usb_cfg: u32,     // TODO
    pub isp_misc_cfg: u32,    // TODO
    pub cc_socu_pin: u32,     // TODO
    pub cc_socu_dflt: u32,    // TODO
    pub vendor_usage: u32,    // TODO
    pub _reserved0: [u32; 1],
    pub secure_boot_cfg: SecureBootCfg,
    pub rotk_usage: RotkUsage,
    /// Secondary boot loader (SBL) or First Mutable Code (FMC) image start address.
    ///
    /// When FMC_SBL_EN is set ROM checks for presence of valid image at this location. If not present then goes to recover boot path.
    pub sbl_start_addr: u32,
    pub err_log_addr: u32,
    /// Root of Trust Key Hash is SHA256 or SHA384 of RoTKpublic. Hash algorithm is selected based on RoTK EC type (secp256r1 -> SHA256 or secp384r1 -> SHA384).
    /// Same RoTKs and RoTKTH values are shared between debug authentication, SB3.1 firmware updates container and signed boot image based on CMPA.RoTKx_Usage .
    ///
    /// For SHA256 the bytes padded with 4 zero bytes at the start.
    pub rotkh: ReverseArray<u32, 12>,
    /// CUST_MK_SK is stored in form of RFC3394 blob and it is used by bootloader to decrypt SB3.1 encryption key during processing of SB file by bootloader.
    /// CUST_MK_SK is generated during device provisioning process by HSM_KEY_GEN (random key) or by HSM_STORE_KEY (user defined key) commands.
    /// To store this key into CMPA, SB_STORE_KEY command should be used.
    pub cust_mk_sk_key_blob: [u32; 12],
    /// Root of Trust Key Hash is SHA256 or SHA384 of RoTKpublic. Hash algorithm is selected based on RoTK EC type (secp256r1 -> SHA256 or secp384r1 -> SHA384).
    /// Same RoTKs and RoTKTH values are shared between debug authentication, SB3.1 firmware updates container and signed boot image based on CMPA.RoTKx_Usage.
    pub pqc_rotkh: ReverseArray<u32, 12>,
    pub quick_gpio: [QuickGpio; 6],
    pub _reserved1: [u32; 4],
    pub iped: [Iped; 8],
    pub _reserved2: [u32; 19],
    pub dice_x509_sram_buf_len: DiceX509SramBufLen,
    pub dice_x509_ecdsa_sram_addr: u32,
    /// If set as zero, then use the 0x30002000 RAM address as the default.
    pub dice_x509_mldsa_sram_addr: u32,
    /// If set as zero, then use the 0x30019000 RAM address as the default.
    pub dice_alias_key_sram_addr: u32,
    /// Certificate template structure for MLDSA alias key identity.
    pub mldsa_cert_temp_addr: u32,
    /// Root of Trust Key Hash is SHA256 or SHA384 of RoTKpublic. Hash algorithm is selected based on RoTK EC type (secp256r1 -> SHA256 or secp384r1 -> SHA384).
    /// Same RoTKs and RoTKTH values are shared between debug authentication, SB3.1 firmware updates container and signed boot image based on CMPA.RoTKx_Usage .
    pub mldsa_cert_temp_hash: ReverseArray<u32, 12>,
}

impl CMPA {
    pub const DEVCFG_ADDR: u32 = 0x0100_0200;
    pub const SCRATCH_ADDR: u32 = 0x0100_2200;
    pub const SECURE_DEVCFG_ADDR: u32 = 0x1100_0200;
    pub const SECURE_SCRATCH_ADDR: u32 = 0x1100_2200;

    pub const ZERO: Self = Self {
        boot_cfg0: BootCfg0::ZERO,
        boot_cfg1: BootCfg1::ZERO,
        boot_led_status: BootLedStatus::ZERO,
        boot_timers: BootTimers::ZERO,
        lspi_qflash_cfg0: LspiQflashCfg0::ZERO,
        lspi_qflash_cfg1: LspiQflashCfg1::ZERO,
        lspi_flash_cfg0: 0,
        lspi_flash_cfg1: 0,
        isp_uart_cfg: 0,
        isp_i2c_cfg: 0,
        isp_can_cfg: 0,
        isp_spi_cfg0: 0,
        isp_spi_cfg1: 0,
        isp_usb_id: 0,
        isp_usb_cfg: 0,
        isp_misc_cfg: 0,
        cc_socu_pin: 0,
        cc_socu_dflt: 0,
        vendor_usage: 0,
        _reserved0: [0; _],
        secure_boot_cfg: SecureBootCfg::ZERO,
        rotk_usage: RotkUsage::ZERO,
        sbl_start_addr: 0,
        err_log_addr: 0,
        rotkh: ReverseArray::new([0; _]),
        cust_mk_sk_key_blob: [0; _],
        pqc_rotkh: ReverseArray::new([0; _]),
        quick_gpio: [QuickGpio::ZERO; _],
        _reserved1: [0; _],
        iped: [Iped::ZERO; _],
        _reserved2: [0; _],
        dice_x509_sram_buf_len: DiceX509SramBufLen::ZERO,
        dice_x509_ecdsa_sram_addr: 0,
        dice_x509_mldsa_sram_addr: 0,
        dice_alias_key_sram_addr: 0,
        mldsa_cert_temp_addr: 0,
        mldsa_cert_temp_hash: ReverseArray::new([0; _]),
    };

    pub fn as_array(&self) -> &[u8; core::mem::size_of::<Self>()] {
        // Safety: We're a repr(C) struct with no padding bytes
        unsafe { core::mem::transmute(self) }
    }
}

#[bitfield(u32, debug, default = 0)]
pub struct BootCfg0 {
    /// CMPA Header marker should be set to 0x5963. After this header is set, all non-zero values will take effect; leaving all settings at 0xff will cause undefined behavior. It is recommended to set all values to 0x00 before setting the CMPA header value.
    #[bits(16..=31, rw)]
    pub marker: u16,
    /// Boot speed. Selects the core frequency and voltage to use during ROM execution.
    #[bits(12..=13, rw)]
    pub boot_speed: BootSpeed,
    /// Recovery boot enable.
    #[bits(8..=9, rw)]
    pub rec_boot_en: BigBool,
    /// Enable external flash(LSPI) as recovery boot media.
    #[bit(5, rw)]
    pub rec_lspi: bool,
    /// Enable external flash(FlexSPI) as recovery boot media.
    #[bit(4, rw)]
    pub rec_flexspi: bool,
    /// Enable dual image boot from external flash as primary boot media.
    #[bit(3, rw)]
    pub eflash_dual_en: bool,
    /// Enable external flash (FlexSPI) as primary boot media.
    #[bit(2, rw)]
    pub eflash_booten: bool,
    /// Enable dual image boot from internal flash as primary boot media.
    #[bit(1, rw)]
    pub iflash_dual_en: bool,
    /// Enable internal flash as primary boot media.
    #[bit(0, rw)]
    pub iflash_booten: bool,
}

/// Boot speed. Selects the core frequency and voltage to use during ROM execution.
#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum BootSpeed {
    // 48MHz, FRO192, // default boot speed. If CMPA header is not valid, should boot from 48MHz MD mode.
    MdMode = 0b00,
    // 96MHz, FRO192,
    SdMode = 0b01,
    // 200MHz, SPLL,
    OdMode = 0b10,
    // 250MHz, SPLL, // The actual speed will be depending on test results.
    OdModePlus = 0b11,
}

/// All true variants mean the same to the boot rom
#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum BigBool {
    False = 0b00,
    True = 0b01,
    True1 = 0b10,
    True2 = 0b11,
}

impl From<bool> for BigBool {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

/// All false variants mean the same to the boot rom
#[bitenum(u2, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
pub enum InverseBigBool {
    True = 0b00,
    False = 0b01,
    False1 = 0b10,
    False2 = 0b11,
}

impl From<bool> for InverseBigBool {
    fn from(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
}

/// All enabled variants mean the same to the boot rom
#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum EntryEnabled {
    Enabled = 0b00,
    Disabled = 0b01,
    Enabled1 = 0b10,
    Enabled2 = 0b11,
}

#[bitfield(u32, debug, default = 0)]
pub struct BootCfg1 {
    /// Customer extended CMPA size expressed as multiple of 32 bytes.
    ///
    /// Total CMPA area = CMPA_BASE_ADDR + 32 * EXT_CMPA32B_SIZE.  
    /// The minimum value for this field should be 16 and max value 160.
    #[bits(24..=31, rw)]
    pub ext_cmpa_32b_size: u8,
    /// Flash remap size.
    ///
    /// FLASH_REMAP_SIZE defines the size of the secondary boot image
    /// (the range of flash addresses that will be remapped)in internal flash, where remapped
    /// address end = (FLASH_REMAP_SIZE + 1) * 32 KB. For example, if FLASH_REMAP_SIZE = 2,
    /// then the first 96KB of addresses will be remapped to flash bank1 instead of flash bank0
    /// when remap is active. Set this field to 0  if you do not want to use the flash remap feature
    #[bits(16..=20, rw)]
    pub flash_remap_size: u5,
    /// Disable ISP mode entry on image authentication failure.
    #[bits(14..=15, rw)]
    pub isp_ft_entry: EntryEnabled,
    /// Disable ISP mode entry through ROM API call.
    /// ISP mode can be entered through ROM API invocation
    #[bits(12..=13, rw)]
    pub isp_api_entry: EntryEnabled,
    /// Disable ISP mode entry through debug mailbox command.
    #[bits(10..=11, rw)]
    pub isp_dm_entry: EntryEnabled,
    /// Disable ISP mode entry  through pin assertion.
    #[bits(8..=9, rw)]
    pub isp_pin_entry: EntryEnabled,
    /// ISP interface enable
    #[bit(4, rw)]
    pub isp_usb_en: bool,
    /// ISP interface enable
    #[bit(3, rw)]
    pub isp_i2c_en: bool,
    /// ISP interface enable
    #[bit(2, rw)]
    pub isp_can_en: bool,
    /// ISP interface enable
    #[bit(1, rw)]
    pub isp_spi_en: bool,
    /// ISP interface enable
    #[bit(0, rw)]
    pub isp_uart_en: bool,
}

#[bitfield(u32, debug, default = 0)]
pub struct BootLedStatus {
    /// Assert on fatal errors during boot.
    ///
    /// ROM drives the GPIO pin high identified by this field whenever primary boot fails due to fatal errors before locking-up/reset.
    /// P0_0 and P0_1 are not supported.
    /// If this feature is not use then set this field to 0x00.
    #[bits(16..=23, rw)]
    pub boot_fail_led: Gpio,
    /// Assert on ISP fall through.
    ///
    /// ROM drives the GPIO pin high identified by this field whenever primary boot fails and execution falls through to ISP mode.
    /// P7_31 is not supported.
    /// If this feature is not use then set this field to 0xFF.
    #[bits(8..=15, rw)]
    pub isp_boot_led: Gpio,
    /// Assert on recovery boot.
    ///
    /// ROM drives the GPIO pin high, identified by this field whenever primary boot fails and fall through to recovery boot source.
    /// P7_31 is not supported.
    /// If this feature is not use then set this field to 0xFF.
    #[bits(0..=7, rw)]
    pub rec_boot_led: Gpio,
}

#[bitfield(u8, debug)]
pub struct Gpio {
    #[bits(5..=7, rw)]
    pub port: u3,
    #[bits(0..=4, rw)]
    pub pin: u5,
}

#[bitfield(u32, debug)]
pub struct BootTimers {
    /// WDOG timeout:
    ///
    /// Upper 16 bits of 24-bit count value in WWDT0_TC register Timeout value in seconds. The lower 8 bits of  WWDT0_TC are set to 0.
    /// When a non-zero value is programmed in this field ROM configures the watch dog timer to reset the device on timeout before passing execution control to user code.
    #[bits(16..=31, rw)]
    pub wdog_timeout_count: u16,
    /// Powerdown timeout:
    ///
    /// ISP mode peripheral detection timeout value in seconds.
    /// If a non-zero value is program and peripheral activity is not detected within the number of seconds specified here, then the device will go to power down mode to conserve power.    
    #[bits(0..=15, rw)]
    pub powerdown_timeout_secs: u16,
}

#[bitfield(u32, debug, default = 0)]
pub struct LspiQflashCfg0 {
    /// Quad SPI port
    #[bits(30..=31, rw)]
    pub qspi_port: Option<QspiPort>,
    /// Delay after POR before accessing Quad/Octal-SPI flash devices in addition to delay defined by FLEXSPI_HOLD TIME field.
    #[bits(25..=28, rw)]
    pub qspi_pwr_hold_time: QspiPwrHoldTime,
    /// Delay after reset before accessing Quad/Octal-SPI flash devices.
    /// Note, for POR in addition to this wait time FLEXSPI_PWR_HOLD_TIME is added.
    #[bits(23..=24, rw)]
    pub qspi_hold_time: QspiHoldTime,
    /// When FLEXSPI_RESET_ENABLE = 1, this field determines the GPIO  pin number to use for O/QSPI reset function.
    #[bits(18..=22, rw)]
    pub qspi_reset_gpio_pin: u5,
    /// When FLEXSPI_RESET_ENABLE = 1, this field determines the GPIO  port number to use for O/QSPI reset function.
    #[bits(15..=17, rw)]
    pub qspi_reset_gpio_port: u3,
    /// Use O/QSPI_RESET_PIN to reset the flash device.
    #[bit(14, rw)]
    pub qspi_reset_enable: bool,
    /// Q/O-SPI flash interface frequency.
    /// Note, this field is used when FLEXSPI_AUTO_PROBE_EN is set.
    #[bits(11..=13, rw)]
    pub qspi_frequency: Option<QspiFrequency>,
    /// Quad/Octal-SPI dummy cycles for read command.
    ///
    /// If a non-zero value is programmed in this field, then the value is used to override the default number of dummy cycles for a fast read command read from the serial flash’s SFDP information.
    ///
    /// Note: this field is only used if FLEXSPI_AUTO_PROBE_EN is set.
    #[bits(7..=10, rw)]
    pub qspi_dummy_cycles: u4,
    #[bit(0, rw)]
    pub qspi_auto_probe_en: bool,
}

#[bitenum(u2)]
#[derive(Debug)]
pub enum QspiPort {
    /// 4-bit
    PortA1 = 0b00,
    /// 4-bit
    PortB1 = 0b01,
    /// 8-bit
    PortA1B1 = 0b10,
}

#[bitenum(u4, exhaustive = true)]
#[derive(Debug)]
pub enum QspiPwrHoldTime {
    NoDelay = 0b0000,
    WaitAdditional100Microseconds = 0b0001,
    WaitAdditional500Microseconds = 0b0010,
    WaitAdditional1Millisecond = 0b0011,
    WaitAdditional10Milliseconds = 0b0100,
    WaitAdditional20Milliseconds = 0b0101,
    WaitAdditional40Milliseconds = 0b0110,
    WaitAdditional60Milliseconds = 0b0111,
    WaitAdditional80Milliseconds = 0b1000,
    WaitAdditional100Milliseconds = 0b1001,
    WaitAdditional120Milliseconds = 0b1010,
    WaitAdditional140Milliseconds = 0b1011,
    WaitAdditional160Milliseconds = 0b1100,
    WaitAdditional180Milliseconds = 0b1101,
    WaitAdditional200Milliseconds = 0b1110,
    WaitAdditional220Milliseconds = 0b1111,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum QspiHoldTime {
    WaitFor500Microseconds = 0b00,
    WaitFor1Millisecond = 0b01,
    WaitFor3Milliseconds = 0b10,
    WaitFor10Milliseconds = 0b11,
}

#[bitenum(u3)]
#[derive(Debug)]
pub enum QspiFrequency {
    Freq75Mhz = 0b000,
    Freq60Mhz = 0b001,
    Freq50Mhz = 0b010,
    Freq100Mhz = 0b011,
}

#[bitfield(u32, debug)]
pub struct LspiQflashCfg1 {
    /// Any offset in memory mapped FlexSPI Flash area could be remapped to offset zero to support eXecute In Place (XIP) of image programmed at different offset.
    /// This allows to build all update images with same RO base address, which are programmed at offset 0 or higher offset.
    /// FLEXSPI_IMAGE_OFFSET field specifies the offset location of second image. FLEXSPI_REMAP_IMAGE_SIZE field specifies the size multiple to determine the size of area to be remapped.  
    #[bits(17..=20, rw)]
    pub qspi_remap_image_size: QspiRemapImageSize,
    /// Any offset in memory mapped FlexSPI Flash area could be remapped to offset zero to support  eXecute In Place (XIP) of image programmed at different offset.
    /// This allows to build all update images with same RO base address, which are programmed at offset 0 or higher offset.
    ///
    /// FlLEXSPI_IMAGE_OFFSET field specifies the offset location of the second image. FLEXSPI_REMAP_IMAGE_SIZE field specifies the size multiple to determine the size of area to be remapped.
    /// If this field is left blank boot ROM will not enable FlexSPI remap feature.
    ///
    /// The physical flash offset is computed as below:
    ///
    /// physical offset = FLEXSPI_IMAGE_OFFSET * 256KByte;
    #[bits(7..=16, rw)]
    pub qspi_image_offset: u10,
    /// Delay cell numbers for Flash read sampling via DQS (either internal loopback or external DQS).
    /// The value provided here is loaded into the FLEXSPIn_DLLnCR.
    #[bits(0..=6, rw)]
    pub qspi_delay_cell_num: u7,
}

#[bitenum(u4, exhaustive = true)]
#[derive(Debug)]
pub enum QspiRemapImageSize {
    /// Remap size = FLEXSPI_IMAGE_OFFSET * 256KByte;"   SIZE_OFFSET "Size of the remapped area (aka second half) is same as first half. It is determined by FLEXSPI_IMAGE_OFFSET Field.
    Remap = 0b0000,
    /// Size of remapped area is 1MByte.    
    Size1mb = 0b0001,
    /// Size of remapped area is 2MByte.    
    Size2mb = 0b0010,
    /// Size of remapped area is 3MByte.   
    Size3mb = 0b0011,
    /// Size of remapped area is 4MByte.
    Size4mb = 0b0100,
    /// Size of remapped area is 5MByte.    
    Size5mb = 0b0101,
    /// Size of remapped area is 6MByte.    
    Size6mb = 0b0110,
    /// Size of remapped area is 7MByte.    
    Size7mb = 0b0111,
    /// Size of remapped area is 8MByte.    
    Size8mb = 0b1000,
    /// Size of remapped area is 9MByte.    
    Size9mb = 0b1001,
    /// Size of remapped area is 10MByte.
    Size10mb = 0b1010,
    /// Size of remapped area is 11MByte.
    Size11mb = 0b1011,
    /// Size of remapped area is 12MByte.
    Size12mb = 0b1100,
    /// Size of remapped area is 256KByte.    
    Size256kb = 0b1101,
    /// Size of remapped area is 512KByte.   
    Size512kb = 0b1110,
    /// Size of remapped area is 768KByte.  
    Size768kb = 0b1111,
}

#[bitfield(u32, debug, default = 0)]
pub struct SecureBootCfg {
    /// Block NXP signed SB3 loading.
    #[bits(30..=31, rw)]
    pub dis_nxp_fw: DisNxpFw,
    /// Enable self-test for KDF block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(26..=27, rw)]
    pub fips_kdf_sten: SelfTestEnable,
    /// Enable self-test for CMAC block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(24..=25, rw)]
    pub fips_cmac_sten: SelfTestEnable,
    /// Enable self-test for DRBG block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(22..=23, rw)]
    pub fips_drbg_sten: SelfTestEnable,
    /// Enable self-test for ECDSA block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(20..=21, rw)]
    pub fips_ecdsa_sten: SelfTestEnable,
    /// Enable self-test for AES block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(18..=19, rw)]
    pub fips_aes_sten: SelfTestEnable,
    /// Enable self-test for SHA2 block on power-up. Needed for FIPS certification. If this field is non-zero  run self-test and log result in SYSCON->ELS_AS_BOOT_LOG1[FIPS].
    #[bits(16..=17, rw)]
    pub fips_sha_sten: SelfTestEnable,
    /// Protection of active image.
    ///
    /// This field defines protection of flash area occupied by the active image. Only applicable to internal flash.
    #[bits(14..=15, rw)]
    pub active_img_prot: ActiveImgProt,
    /// Fast boot enabled. First boot after update local-CMAC using DUK-Auth key is created and used for sub-sequent boot authentication.
    #[bits(12..=13, rw)]
    pub fast_boot_en: InverseBigBool,
    /// Enforce preset TZM data in image manifest.
    #[bits(10..=11, rw)]
    pub enf_tzm_preset: BigBool,
    /// Enforce CNSA suite approved algorithms for secure boot, secure update and debug authentication.
    #[bits(8..=9, rw)]
    pub enf_cnsa: EnfCnsa,
    /// Define the DICE csr and key generation type.
    #[bits(6..=7, rw)]
    pub dice_csr_key_type: DiceCsrKeyType,
    /// Secure boot option for low-power wake from power-down & deep-powerdown.
    /// For CFPA/CMPA do CRC check always.
    #[bits(3..=4, rw)]
    pub lp_sec_boot: LpSecBoot,
    /// Secure boot enforcement.
    ///
    /// This field defines the minimum image verification procedure (CRC32, CMAC, ECDSA sign).
    /// The Image type field in header indicates the type of verification data (checksum or signature) included in it.
    ///
    /// Plain < CRC32 < CMAC < ECDSA < ECDSA+MLDSA
    #[bits(0..=1, rw)]
    pub sec_boot_en: SecBootEn,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum DisNxpFw {
    AllowNxpSignedSb3Fw1 = 0b00,
    /// Same as [Self::AllowNxpSignedSb3Fw1]
    AllowNxpSignedSb3Fw2 = 0b11,
    DisableNxpSignedSb3Fw = 0b01,
    AllowOnlyOemAndNxpSignedSb3Fw = 0b10,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum SelfTestEnable {
    NotIncluded = 0b00,
    OnFailureContinueToBoot = 0b01,
    OnFailureEnterIspModeForRecovery = 0b10,
    OnFailureLockTheDeviceToEnforcePowerCycle = 0b11,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum ActiveImgProt {
    /// Protection is defined using the CFPA FLASH_ACL settings.
    FlashAcl = 0b00,
    /// Write protect active image area with sticky lock. GLBAC2 is used. FLASH_ACL settings are ignored.
    GLBAC2 = 0b01,
    /// Write protect active image area without sticky lock. GLBAC4 is used. FLASH_ACL settings are ignored.
    GLBAC4 = 0b10,
    /// XOM protect active image area with sticky lock. GLBAC6 is used. FLASH_ACL settings are ignored.
    GLBAC6 = 0b11,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
pub enum EnfCnsa {
    /// Use performance crypt algos (ECC P-256, SHA256 & AES 128).
    Performance = 0b00,
    /// Use CNSA 1.0 specified clasic crypto algorithims. (ECC P-384, SHA384 & AES256).
    CNSA1 = 0b01,
    /// Use CNSA 2.0 specified hybrid (PQC + clasic) crypto algorithims.(ECDSA P-384 + MLDSA-87, ML-KEM, SHA384 & AES256).
    CNSA2 = 0b10,
    /// Same as [Self::CNSA2]
    CNSA2Dup = 0b11,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug)]
pub enum DiceCsrKeyType {
    /// Generate DICE ECC P-384 keys.
    EccP384 = 0b00,
    /// Same as [Self::EccP384]
    EccP384Dup = 0b01,
    /// Generate DICE SHA384 & MLDSA keys.
    Sha384AndMLDSA = 0b10,
    /// Same as [Self::Sha384AndMLDSA]
    Sha384AndMLDSADup = 0b11,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
pub enum LpSecBoot {
    /// Same as cold boot
    Cold = 0b00,
    /// CRC check for CFPA/CMPA & CRC32 check of active image
    CheckForCfpaCmpaAndCrc32 = 0b01,
    /// CRC check for CFPA/CMPA & jump to vector address specified in CFPA.
    CheckForCfpaCmpaAndJump = 0b10,
    /// CRC check for CFPA/CMPA & CMAC check of active image
    CheckForCfpaCmpaAndCmac = 0b11,
}

#[bitenum(u2, exhaustive = true)]
#[derive(Debug, PartialEq, Eq)]
pub enum SecBootEn {
    /// All Image types are allowed.
    AllAllowed = 0b00,
    /// Only CRC32 or cryptographically signed (CMAC or PKI - ECDSA/ MLDSA) images are allowed.
    OnlyCrc32OrSigned = 0b01,
    /// Only cryptographically Signed (CMAC or PKI - ECDSA/ MLDSA) images are allowed.
    OnlySigned = 0b10,
    /// Only PKI signed (ECDSA or MLDSA) images are allowed.
    OnlyPki = 0b11,
}

#[bitfield(u32, debug, default = 0)]
pub struct RotkUsage {
    /// Include NXP field programmed area (NFPA) containing in-field ROM patch in DICE computation.
    #[bit(15, rw)]
    pub dice_inc_nxp_field_cfg: bool,
    /// Include data from CMPA page  in DICE computation.
    #[bit(14, rw)]
    pub dice_inc_cust_cfg: bool,
    /// Include NXP area (IFR1) containing specific part configuration data defined during chip manufacturing process in DICE computation.
    #[bit(13, rw)]
    pub dice_inc_nxp_cfg: bool,
    /// Skip DICE computation.
    /// - 0 - Enable DICE
    /// - 1 - Disable DICE
    #[bit(12, rw)]
    #[doc(alias = "SKIP_DICE")]
    pub disable_dice: bool,
    #[bits(0..=2, rw, stride = 3)]
    pub rotk_usage: [RotkUsageVal; 4],
}

#[bitenum(u3, exhaustive = true)]
#[derive(Debug)]
pub enum RotkUsageVal {
    /// Usable as debug CA, image CA, FW CA, image and FW key.
    All = 0b000,
    /// Usable as debug CA only.
    DebugCaOnly = 0b001,
    /// Usable as image (boot & FW) CA only.
    ImageCaOnly = 0b010,
    /// Usable as debug, boot & FW image CA.
    DebugAndImage = 0b011,
    /// Usable as image key & FW update key only.
    KeyOnly = 0b100,
    /// Usable as boot image key only.
    BootImageKeyOnly = 0b101,
    /// Usable as FW update image key only.
    FwUpdateImageKeyOnly = 0b110,
    /// Key slot is not used.
    Unused = 0b111,
}

#[bitfield(u64, debug)]
pub struct QuickGpio {
    /// Drive GPIO port pin high after reset.
    ///
    /// Each bit corresponds to the pin in the GPIO port. When set ROM drives the corresponding pin high as soon as possible. By default most pins come-up as tri-stated inputs.
    /// This feature allows customer to specify active drive pins soon after reset instead of waiting till complete boot.
    /// Note, if a pin is selected in both QUICK_SET_GPIO_x and QUICK_CLR_GPIO_x fields then pin will be set to high-level with quick transition to low.
    #[bit(0, rw, stride = 1)]
    pub set: [bool; 32],
    /// Drive GPIO port pin low.
    #[bit(32, rw, stride = 1)]
    pub clear: [bool; 32],
}

#[bitfield(u64, debug)]
pub struct Iped {
    /// Upper 24-bits of IPED region 0 start address. Lower 8 address bits are always 0.
    #[bits(8..=31, rw)]
    pub start_addr: u24,
    /// Disable AHB bus error.
    /// If GCM authentication fails generates bus error or not.
    /// - 0 - Bus error enabled.
    /// - 1 - Bus error disabled.
    #[bit(1, rw)]
    pub ahberr_dis: bool,
    /// GCM mode enable.
    /// - 0 -  Region is enabled in CTR mode.
    /// - 1 - Region is enabled in GCM mode.
    #[bit(0, rw)]
    pub gcm_mode: bool,
    /// Upper 24-bits of IPED region 0 end address. Lower 8 address bits are always 0.
    #[bits(40..=63, rw)]
    pub end_addr: u24,
    #[bits(32..=33, rw)]
    pub lock_en: BigBool,
}

#[bitfield(u32, debug)]
pub struct DiceX509SramBufLen {
    #[bits(0..=15, rw)]
    pub dice_ecdsa_cert_buf_size: u16,
    #[bits(16..=31, rw)]
    pub dice_mldsa_cert_buf_size: u16,
}

/// An array that stores the elements in reverse order, but is interacted with in normal order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct ReverseArray<T, const N: usize>([T; N]);

impl<T, const N: usize> ReverseArray<T, N> {
    /// Create a reverse array based on an array in the normal order
    pub const fn new(mut data: [T; N]) -> Self {
        data.reverse();
        Self(data)
    }

    /// Discard the wrapped and get the raw array (which is in reverse order)
    pub fn degrade(self) -> [T; N] {
        self.0
    }

    /// Discard the wrapped and get the raw array in original (non-reverse) order
    pub fn degrade_original(mut self) -> [T; N] {
        self.0.reverse();
        self.0
    }
}

impl<T, const N: usize> Index<usize> for ReverseArray<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[N - 1 - index]
    }
}

impl<T, const N: usize> IndexMut<usize> for ReverseArray<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[N - 1 - index]
    }
}

impl<T, const N: usize> IntoIterator for ReverseArray<T, N> {
    type Item = T;

    type IntoIter = core::iter::Rev<core::array::IntoIter<T, N>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().rev()
    }
}

impl<T, const N: usize> FromIterator<T> for ReverseArray<T, N> {
    fn from_iter<U: IntoIterator<Item = T>>(iter: U) -> Self {
        let mut iter = iter.into_iter();
        Self::new(core::array::from_fn(|_| iter.next().unwrap()))
    }
}
