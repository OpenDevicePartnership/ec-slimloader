//! Credential-constraint fields for MCXA5xx CMPA.
//!
//! Source: MCXAP172M240F70RM Rev. 1, embedded `IFR.xlsx`, `DEVCFG`
//! rows 151–152 (including cell comments). The types describe the direct
//! lower half only; encoding supplies reserved bits and the inverted upper half.
//! Effective access also depends on the device lifecycle.

/// Who controls a debug domain or command (CC_SOCU_PIN).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugControl {
    /// Access can be enabled through debug authentication.
    DebugAuthentication = 0,
    /// The corresponding CC_SOCU_DFLT bit fixes the access state.
    Fixed = 1,
}

/// The corresponding CC_SOCU_DFLT setting for a debug domain or command.
///
/// With [`DebugControl::Fixed`], this disables or enables access. With
/// [`DebugControl::DebugAuthentication`], Disabled selects ROM authentication
/// and Enabled selects customer-application authentication (RM §7.3.4.7.1.4,
/// Table 144). It is not an unconditional promise of access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugState {
    Disabled = 0,
    Enabled = 1,
}

/// Whether mass erase is allowed to erase DEVCFG.
///
/// Despite the hardware name `ME_DEVCFG_DIS`, `IFR.xlsx` cell W152 specifies
/// zero for erase disabled and one for erase allowed. Allowing erasure can
/// return an in-field device to Develop, affecting the secure-boot claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevcfgErase {
    Disabled = 0,
    Enabled = 1,
}

/// Treatment of SGI keystore keys before granting debug access (cell V152).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugKeyFlush {
    /// Retain the SGI keys.
    Retain = 0,
    /// Clear the SGI keys before granting access.
    Clear = 1,
}

/// UUID matching required during debug authentication (cell U152).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugUuidCheck {
    /// A debug certificate can be shared across devices.
    SharedCertificate = 0,
    /// A per-device certificate with a matching UUID is required.
    PerDeviceCertificate = 1,
}

/// The seven configurable direct bits of CC_SOCU_PIN (`DEVCFG` row 151).
///
/// Reserved bits and the inverse copy are not caller-configurable. There are
/// exactly 128 configurations; this is intentionally distinct from [`CcSocuDflt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcSocuPin {
    /// Non-secure non-invasive debug control.
    pub niden: DebugControl,
    /// Non-secure invasive debug control.
    pub dbgen: DebugControl,
    /// Secure non-invasive debug control.
    pub spniden: DebugControl,
    /// Secure invasive debug control.
    pub spiden: DebugControl,
    /// ISP boot command control.
    pub isp_cmd: DebugControl,
    /// Set_FA and Set_Brick command control.
    pub fa_cmd: DebugControl,
    /// Flash mass-erase command control.
    pub me_cmd: DebugControl,
}

impl CcSocuPin {
    /// Encode the direct bits, fixed reserved values, and inverted copy.
    pub const fn encode(self) -> u32 {
        let direct = 0xE000
            | (self.niden as u16)
            | ((self.dbgen as u16) << 1)
            | ((self.spniden as u16) << 2)
            | ((self.spiden as u16) << 3)
            | ((self.isp_cmd as u16) << 6)
            | ((self.fa_cmd as u16) << 7)
            | ((self.me_cmd as u16) << 8);
        with_inverse(direct)
    }
}

/// The ten configurable direct bits of CC_SOCU_DFLT (`DEVCFG` row 152).
///
/// Reserved bits and the inverse copy are not caller-configurable. There are
/// exactly 1,024 configurations; defaults belong to the provisioning policy,
/// not this hardware representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcSocuDflt {
    /// Non-secure non-invasive debug setting.
    pub niden: DebugState,
    /// Non-secure invasive debug setting.
    pub dbgen: DebugState,
    /// Secure non-invasive debug setting.
    pub spniden: DebugState,
    /// Secure invasive debug setting.
    pub spiden: DebugState,
    /// ISP boot command setting.
    pub isp_cmd: DebugState,
    /// Set_FA and Set_Brick command setting.
    pub fa_cmd: DebugState,
    /// Flash mass-erase command setting.
    pub me_cmd: DebugState,
    /// Whether mass erase may erase DEVCFG.
    pub devcfg_erase: DevcfgErase,
    /// Whether SGI keys are cleared before debug access.
    pub key_flush: DebugKeyFlush,
    /// Whether debug certificates must match this device's UUID.
    pub uuid_check: DebugUuidCheck,
}

impl CcSocuDflt {
    /// Encode the direct bits, zero reserved bits, and inverted copy.
    pub const fn encode(self) -> u32 {
        let direct = (self.niden as u16)
            | ((self.dbgen as u16) << 1)
            | ((self.spniden as u16) << 2)
            | ((self.spiden as u16) << 3)
            | ((self.isp_cmd as u16) << 6)
            | ((self.fa_cmd as u16) << 7)
            | ((self.me_cmd as u16) << 8)
            | ((self.devcfg_erase as u16) << 13)
            | ((self.key_flush as u16) << 14)
            | ((self.uuid_check as u16) << 15);
        with_inverse(direct)
    }
}

const fn with_inverse(direct: u16) -> u32 {
    (direct as u32) | (((!direct) as u32) << 16)
}
