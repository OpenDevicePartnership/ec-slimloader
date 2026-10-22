#![no_std]

use core::convert::Infallible;

use certificate::{derive_image_rkth_pair, DeriveError};
use embassy_mcxa::rom::FlashError;
use embassy_mcxa::{peripherals, Peri};
use header::{HeaderError, ImageHeader};
use mcxa_ifr::{
    u3, u4, u5, BootCfg0, BootCfg1, DiceCsrKeyType, Header, InverseBigBool, LifeCycleState, LspiQflashCfg0, QspiPort,
    RotkRevoke, RotkUsage, RotkUsageVal, SecureBootCfg, Update, CFPA, IFR,
};

pub mod certificate;
pub mod header;

pub struct Provisioner {
    ifr: IFR,
    secure_alias: bool,
    cmpa_updated: bool,
    cfpa_updated: bool,
}

impl Provisioner {
    const DEFAULT_CC_SOCU_PIN: u32 = 0x1FFFE000; // default value for CC_SOCU_PIN in CMPA, for breakdown, refer to NXP secure provisioning guide.
    const DEFAULT_CC_SOCU_DFLT: u32 = 0xBFFF4000; // default value for CC_SOCU_DFLT in CMPA, for breakdown, refer to NXP secure provisioning guide.

    /// # Safety
    /// Only sound to call on an mcxa5xx device
    pub unsafe fn new_from_page(secure_alias: bool) -> Self {
        let disk_ifr = if secure_alias {
            IFR::secure_current()
        } else {
            IFR::current()
        };

        Self {
            ifr: disk_ifr.clone(),
            secure_alias,
            cmpa_updated: false,
            cfpa_updated: false,
        }
    }

    pub fn is_erased(&self) -> bool {
        self.ifr.cfpa.is_erased() || self.ifr.cmpa.is_erased()
    }

    /// Commit the changes to flash. Only goes into effect after reboot, so prefer using [Self::commit_and_reboot]
    pub fn commit(&mut self) -> Result<(), FlashError> {
        if self.cfpa_updated {
            if self.ifr.cfpa.cfpa_page_version == u32::MAX {
                self.ifr.cfpa.cfpa_page_version = 1;
            } else {
                self.ifr.cfpa.cfpa_page_version += 1;
            }
        }

        match (self.cfpa_updated, self.cmpa_updated) {
            (false, false) => {
                // Nothing to commit
            }
            (_, true) => {
                self.erase_scratch()?;
                self.commit_cmpa_and_cfpa()?;
            }
            (true, false) => {
                self.erase_scratch()?;
                self.commit_cfpa_only()?;
            }
        }

        Ok(())
    }

    /// Commit the changes to flash and reboot so it takes effect immediately
    pub fn commit_and_reboot(&mut self) -> Result<Infallible, FlashError> {
        self.commit()?;
        cortex_m::peripheral::SCB::sys_reset()
    }

    fn erase_scratch(&self) -> Result<(), FlashError> {
        let ifr_scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.erase_sector(ifr_scratch_addr, 8 * 1024)?;
        drv.ifr_verify_erase_sector(ifr_scratch_addr, 8 * 1024)?;

        Ok(())
    }

    fn commit_cfpa_only(&mut self) -> Result<(), FlashError> {
        let scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        self.ifr
            .update
            .devcfg_upd_type
            .set_devcfg_upd_type(mcxa_ifr::UpdateType::CfpaUpdated);

        let ifr_subslice = &self.ifr.as_array().as_slice()[..size_of::<Update>() + size_of::<CFPA>()];

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.program_phrase(scratch_addr, ifr_subslice)?;
        drv.verify_program(scratch_addr, ifr_subslice)?;

        Ok(())
    }

    fn commit_cmpa_and_cfpa(&mut self) -> Result<(), FlashError> {
        let scratch_addr = if self.secure_alias {
            IFR::SECURE_SCRATCH_ADDR
        } else {
            IFR::SCRATCH_ADDR
        };

        self.ifr
            .update
            .devcfg_upd_type
            .set_devcfg_upd_type(mcxa_ifr::UpdateType::CmpaUpdated);

        let ifr_slice = self.ifr.as_array().as_slice();

        let mut drv = embassy_mcxa::rom::get().flash()?;

        drv.program_phrase(scratch_addr, ifr_slice)?;
        drv.verify_program(scratch_addr, ifr_slice)?;

        Ok(())
    }

    pub fn bump_auth_fail_count(&mut self) -> &mut Self {
        self.ifr.cfpa.err_auth_fail_count = self.ifr.cfpa.err_auth_fail_count.wrapping_add(1);
        self.cfpa_updated = true;
        self
    }

    pub fn bump_firmware_version(&mut self) -> &mut Self {
        self.ifr.cfpa.ee0_fw_version = self.ifr.cfpa.ee0_fw_version.wrapping_add(1);
        self.cfpa_updated = true;
        self
    }

    pub fn with_rotk_revoke(&mut self, val: RotkRevoke) -> &mut Self {
        self.ifr.cfpa.rotk_revoke = val;
        self.cfpa_updated = true;
        self
    }

    pub fn with_lifecycle(&mut self, val: LifeCycleState) -> &mut Self {
        self.ifr.cfpa.header.set_cfpa_lc_state(val);
        self.ifr.cfpa.header.set_inv_cfpa_lc_state(val.inverse());
        self.cfpa_updated = true;
        self
    }

    /// Sets a "default" secure Boot configuration for CMPA. This is intended for first-time provisioning of a DEV unit, and must only be called in the Develop lifecycle state.
    /// Not intended for production or factory use.
    pub fn with_default_secure_cmpa_config(&mut self, config: CmpaDefaultConfig) -> Result<&mut Self, ProvisionError> {
        if self.ifr.cfpa.header.cfpa_lc_state() != Ok(LifeCycleState::Develop) {
            return Err(ProvisionError::InvalidLifeCycle);
        }

        self.ifr.cmpa.boot_cfg0 = config.boot_cfg0.with_marker(0x5963);
        self.ifr.cmpa.boot_cfg1 = config.boot_cfg1;
        self.ifr.cmpa.rotk_usage = config.rotk_usage;
        self.ifr.cmpa.secure_boot_cfg = SecureBootCfg::builder()
            .with_sec_boot_en(mcxa_ifr::SecBootEn::OnlyPki)
            .with_lp_sec_boot(mcxa_ifr::LpSecBoot::Cold)
            .with_dice_csr_key_type(config.dice_csr_key_type)
            .with_enf_cnsa(mcxa_ifr::EnfCnsa::CNSA2)
            .with_enf_tzm_preset(config.enf_tzm_preset.into())
            .with_fast_boot_en(mcxa_ifr::InverseBigBool::False)
            .with_active_img_prot(mcxa_ifr::ActiveImgProt::GLBAC2)
            .with_fips_sha_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_aes_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_ecdsa_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_drbg_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_cmac_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_kdf_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_dis_nxp_fw(mcxa_ifr::DisNxpFw::DisableNxpSignedSb3Fw)
            .build();
        self.ifr.cmpa.sbl_start_addr = config.sbl_start_addr;

        self.with_rotkh(config.rotkh, config.pqc_rotkh);

        self.ifr.cmpa.cc_socu_pin = Self::DEFAULT_CC_SOCU_PIN;
        self.ifr.cmpa.cc_socu_dflt = Self::DEFAULT_CC_SOCU_DFLT;

        self.cmpa_updated = true;

        Ok(self)
    }

    /// First boot IFR (CMPA / CFPA) provisioning for DEV and/or factory floor, non-security critical.
    /// Keeps secure boot, DICE and TZM disabled, and sets a safe default for other fields.
    pub fn with_initial_config(&mut self) -> &mut Self {
        if self.is_erased() {
            let err_auth_fail_count = self.ifr.cfpa.err_auth_fail_count;
            let err_itrc_count = self.ifr.cfpa.err_itrc_count;

            self.ifr = IFR::zeroed();
            self.ifr.cfpa.header = Header::builder()
                .with_marker(0x9635)
                .with_cfpa_lc_state(LifeCycleState::Develop)
                .with_inv_cfpa_lc_state(mcxa_ifr::InvLifeCycleState::Develop)
                .build();

            // Restore the error fields so the ROM can keep track of them
            self.ifr.cfpa.err_auth_fail_count = err_auth_fail_count;
            self.ifr.cfpa.err_itrc_count = err_itrc_count;

            self.cfpa_updated = true;
        }

        self.ifr.cmpa.boot_cfg0 = BootCfg0::builder()
            .with_marker(0x5963)
            .with_boot_speed(mcxa_ifr::BootSpeed::MdMode)
            .with_rec_boot_en(false.into())
            .with_rec_lspi(false)
            .with_rec_flexspi(false)
            .with_eflash_dual_en(false)
            .with_eflash_booten(true)
            .with_iflash_dual_en(false)
            .with_iflash_booten(true)
            .build();
        self.ifr.cmpa.boot_cfg1 = BootCfg1::builder()
            .with_ext_cmpa_32b_size(160) // Max size
            .with_flash_remap_size(const { u5::new(0) })
            .with_isp_ft_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_api_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_dm_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_pin_entry(mcxa_ifr::EntryEnabled::Enabled)
            .with_isp_usb_en(false)
            .with_isp_i2c_en(false)
            .with_isp_can_en(false)
            .with_isp_spi_en(false)
            .with_isp_uart_en(true)
            .build();
        self.ifr.cmpa.rotk_usage = RotkUsage::builder()
            .with_dice_inc_nxp_field_cfg(false)
            .with_dice_inc_cust_cfg(false)
            .with_dice_inc_nxp_cfg(false)
            .with_disable_dice(true) // Dice not yet set up
            .with_rotk_usage([RotkUsageVal::All; 4])
            .build();
        self.ifr.cmpa.secure_boot_cfg = SecureBootCfg::builder()
            .with_sec_boot_en(mcxa_ifr::SecBootEn::AllAllowed)
            .with_lp_sec_boot(mcxa_ifr::LpSecBoot::Cold)
            .with_dice_csr_key_type(DiceCsrKeyType::Sha384AndMLDSA)
            .with_enf_cnsa(mcxa_ifr::EnfCnsa::CNSA2)
            .with_enf_tzm_preset(false.into())
            .with_fast_boot_en(mcxa_ifr::InverseBigBool::False)
            .with_active_img_prot(mcxa_ifr::ActiveImgProt::FlashAcl)
            .with_fips_sha_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_aes_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_ecdsa_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_drbg_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_cmac_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_kdf_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_dis_nxp_fw(mcxa_ifr::DisNxpFw::DisableNxpSignedSb3Fw)
            .build();
        self.ifr.cmpa.lspi_qflash_cfg0 = LspiQflashCfg0::builder()
            .with_qspi_port(cfg_select! {
                feature = "mcxa5xxevk" => QspiPort::PortA1, // TODO: Make a separate feature for this instead of an evk feature
                _ => QspiPort::PortB1,
            })
            .with_qspi_pwr_hold_time(mcxa_ifr::QspiPwrHoldTime::NoDelay)
            .with_qspi_hold_time(mcxa_ifr::QspiHoldTime::WaitFor500Microseconds)
            .with_qspi_reset_gpio_pin(const { u5::new(0) })
            .with_qspi_reset_gpio_port(const { u3::new(0) })
            .with_qspi_reset_enable(false)
            .with_qspi_frequency(mcxa_ifr::QspiFrequency::Freq100Mhz)
            .with_qspi_dummy_cycles(const { u4::new(0) })
            .with_qspi_auto_probe_en(true)
            .build();
        self.ifr.cmpa.sbl_start_addr = 0x1000_0000;

        self.ifr.cmpa.cc_socu_pin = Self::DEFAULT_CC_SOCU_PIN;
        self.ifr.cmpa.cc_socu_dflt = Self::DEFAULT_CC_SOCU_DFLT;

        self.cmpa_updated = true;

        self
    }

    pub fn check_production_settings(&mut self) -> Result<&mut Self, ProvisionError> {
        if self.is_erased()
            || !matches!(
                self.ifr.cmpa.secure_boot_cfg.sec_boot_en(),
                mcxa_ifr::SecBootEn::OnlyPki
            )
            || !matches!(
                self.ifr.cmpa.secure_boot_cfg.enf_cnsa(),
                mcxa_ifr::EnfCnsa::CNSA2 | mcxa_ifr::EnfCnsa::CNSA2Dup
            )
            || matches!(self.ifr.cmpa.secure_boot_cfg.fast_boot_en(), InverseBigBool::True)
            || !matches!(self.ifr.cmpa.secure_boot_cfg.lp_sec_boot(), mcxa_ifr::LpSecBoot::Cold)
        {
            return Err(ProvisionError::InvalidSettings);
        }

        Ok(self)
    }

    pub fn with_rotkh(&mut self, rotkh: [u32; 12], pqc_rotkh: [u32; 12]) -> &mut Self {
        self.ifr.cmpa.rotkh = rotkh;
        self.ifr.cmpa.pqc_rotkh = pqc_rotkh;
        self.cmpa_updated = true;
        self
    }

    /// Checks if the rotkh in the IFR corresponds with data stored in flash.
    /// This is done by calculating the hashes over the bytes stored at the addresses.
    ///
    /// Provide the starting addresses of the multiple SBL image splits or provide just one address if there's just one SBL image.
    ///
    /// If no starting addresses are given, the function errors with [ProvisionError::NoImages]
    pub fn check_rotkh(
        &mut self,
        mut peri: Peri<'_, peripherals::SGI0>,
        starting_addresses: impl Iterator<Item = u32>,
    ) -> Result<&mut Self, ProvisionError> {
        const MAX_PROVISIONED_IMAGE_SIZE: u32 = 2 * 1024 * 1024; // Max 2MB flash.
        let mut image_count = 0;

        for starting_address in starting_addresses {
            image_count += 1;
            let image_base = starting_address as *const u8;

            let image_header = unsafe { ImageHeader::from_ptr(image_base, MAX_PROVISIONED_IMAGE_SIZE) }?;

            let (image_ecdsa_rkth, image_pqc_rkth) = derive_image_rkth_pair(
                peri.reborrow(),
                image_base,
                image_header.extended_header_offset(),
                image_header.image_length(),
            )?;

            if image_ecdsa_rkth.as_words() != self.ifr.cmpa.rotkh {
                return Err(ProvisionError::RotkhMismatch);
            }
            if image_pqc_rkth.as_words() != self.ifr.cmpa.pqc_rotkh {
                return Err(ProvisionError::RotkhMismatch);
            }
        }

        if image_count > 0 {
            Ok(self)
        } else {
            Err(ProvisionError::NoImages)
        }
    }

    /// Enables secure boot policies in CMPA
    pub fn with_secure_boot_policies(&mut self) -> &mut Self {
        self.ifr.cmpa.secure_boot_cfg = SecureBootCfg::builder()
            .with_sec_boot_en(mcxa_ifr::SecBootEn::OnlyPki)
            .with_lp_sec_boot(mcxa_ifr::LpSecBoot::Cold)
            .with_dice_csr_key_type(DiceCsrKeyType::Sha384AndMLDSA)
            .with_enf_cnsa(mcxa_ifr::EnfCnsa::CNSA2)
            .with_enf_tzm_preset(true.into())
            .with_fast_boot_en(mcxa_ifr::InverseBigBool::False)
            .with_active_img_prot(mcxa_ifr::ActiveImgProt::GLBAC4)
            .with_fips_sha_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_aes_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_ecdsa_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_drbg_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_cmac_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_fips_kdf_sten(mcxa_ifr::SelfTestEnable::NotIncluded)
            .with_dis_nxp_fw(mcxa_ifr::DisNxpFw::DisableNxpSignedSb3Fw)
            .build();

        self.cmpa_updated = true;

        self
    }

    pub fn ifr(&self) -> &IFR {
        &self.ifr
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisionError {
    UnexpectedIfrState,
    InvalidLifeCycle,
    InvalidSettings,
    SblHeaderError(HeaderError),
    RotkhDeriveError(DeriveError),
    RotkhMismatch,
    NoImages,
}

impl From<DeriveError> for ProvisionError {
    fn from(v: DeriveError) -> Self {
        Self::RotkhDeriveError(v)
    }
}

impl From<HeaderError> for ProvisionError {
    fn from(v: HeaderError) -> Self {
        Self::SblHeaderError(v)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CmpaDefaultConfig {
    pub boot_cfg0: BootCfg0,
    pub boot_cfg1: BootCfg1,
    pub rotk_usage: RotkUsage,
    pub dice_csr_key_type: DiceCsrKeyType,
    pub enf_tzm_preset: bool,
    pub sbl_start_addr: u32,
    pub rotkh: [u32; 12],
    pub pqc_rotkh: [u32; 12],
}
