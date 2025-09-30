/// Root block of the Device driver

#[derive(Debug)]
pub struct Device<I> {
    pub(crate) interface: I,

    #[doc(hidden)]
    base_address: u8,
}

impl<I> Device<I> {
    /// Create a new instance of the block based on device interface
    pub const fn new(interface: I) -> Self {
        Self {
            interface,
            base_address: 0,
        }
    }

    /// A reference to the interface used to communicate with the device
    pub(crate) fn interface(&mut self) -> &mut I {
        &mut self.interface
    }

    /// Read all readable register values in this block from the device.
    /// The callback is called for each of them.
    /// Any registers in child blocks are not included.
    ///
    /// The callback has three arguments:
    ///
    /// - The address of the register
    /// - The name of the register (with index for repeated registers)
    /// - The read value from the register
    ///
    /// This is useful for e.g. debug printing all values.
    /// The given [field_sets::FieldSetValue] has a Debug and Format implementation that forwards to the concrete type
    /// the lies within so it can be printed without matching on it.
    #[allow(unused_mut)]
    #[allow(unused_variables)]
    pub fn read_all_registers(
        &mut self,
        mut callback: impl FnMut(u32, &'static str, field_sets::FieldSetValue),
    ) -> Result<(), I::Error>
    where
        I: ::device_driver::RegisterInterface<AddressType = u32>,
    {
        let reg = self.boot_cfg_0().read()?;

        callback(96 + 0 * 0, "boot_cfg_0", reg.into());

        let reg = self.boot_cfg_1().read()?;

        callback(97 + 0 * 0, "boot_cfg_1", reg.into());

        let reg = self.sec_boot_cfg_5().read()?;

        callback(101 + 0 * 0, "sec_boot_cfg_5", reg.into());

        let reg = self.rkth().read()?;

        callback(120 + 0 * 0, "rkth", reg.into());

        Ok(())
    }

    /// Read all readable register values in this block from the device.
    /// The callback is called for each of them.
    /// Any registers in child blocks are not included.
    ///
    /// The callback has three arguments:
    ///
    /// - The address of the register
    /// - The name of the register (with index for repeated registers)
    /// - The read value from the register
    ///
    /// This is useful for e.g. debug printing all values.
    /// The given [field_sets::FieldSetValue] has a Debug and Format implementation that forwards to the concrete type
    /// the lies within so it can be printed without matching on it.
    #[allow(unused_mut)]
    #[allow(unused_variables)]
    pub async fn read_all_registers_async(
        &mut self,
        mut callback: impl FnMut(u32, &'static str, field_sets::FieldSetValue),
    ) -> Result<(), I::Error>
    where
        I: ::device_driver::AsyncRegisterInterface<AddressType = u32>,
    {
        let reg = self.boot_cfg_0().read_async().await?;

        callback(96 + 0 * 0, "boot_cfg_0", reg.into());

        let reg = self.boot_cfg_1().read_async().await?;

        callback(97 + 0 * 0, "boot_cfg_1", reg.into());

        let reg = self.sec_boot_cfg_5().read_async().await?;

        callback(101 + 0 * 0, "sec_boot_cfg_5", reg.into());

        let reg = self.rkth().read_async().await?;

        callback(120 + 0 * 0, "rkth", reg.into());

        Ok(())
    }

    pub fn boot_cfg_0(
        &mut self,
    ) -> ::device_driver::RegisterOperation<'_, I, u32, field_sets::BootCfg0, ::device_driver::RW> {
        let address = self.base_address + 96;

        ::device_driver::RegisterOperation::<'_, I, u32, field_sets::BootCfg0, ::device_driver::RW>::new(
            self.interface(),
            address as u32,
            field_sets::BootCfg0::new,
        )
    }

    pub fn boot_cfg_1(
        &mut self,
    ) -> ::device_driver::RegisterOperation<'_, I, u32, field_sets::BootCfg1, ::device_driver::RW> {
        let address = self.base_address + 97;

        ::device_driver::RegisterOperation::<'_, I, u32, field_sets::BootCfg1, ::device_driver::RW>::new(
            self.interface(),
            address as u32,
            field_sets::BootCfg1::new,
        )
    }

    pub fn sec_boot_cfg_5(
        &mut self,
    ) -> ::device_driver::RegisterOperation<'_, I, u32, field_sets::SecBootCfg5, ::device_driver::RW> {
        let address = self.base_address + 101;

        ::device_driver::RegisterOperation::<'_, I, u32, field_sets::SecBootCfg5, ::device_driver::RW>::new(
            self.interface(),
            address as u32,
            field_sets::SecBootCfg5::new,
        )
    }

    pub fn rkth(&mut self) -> ::device_driver::RegisterOperation<'_, I, u32, field_sets::Rkth, ::device_driver::RW> {
        let address = self.base_address + 120;

        ::device_driver::RegisterOperation::<'_, I, u32, field_sets::Rkth, ::device_driver::RW>::new(
            self.interface(),
            address as u32,
            field_sets::Rkth::new,
        )
    }
}

/// Module containing the generated fieldsets of the registers and commands
pub mod field_sets {
    #[allow(unused_imports)]
    use super::*;

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct BootCfg0 {
        /// The internal bits
        bits: [u8; 4],
    }

    impl ::device_driver::FieldSet for BootCfg0 {
        const SIZE_BITS: u32 = 32;
        fn new_with_zero() -> Self {
            Self::new_zero()
        }
        fn get_inner_buffer(&self) -> &[u8] {
            &self.bits
        }
        fn get_inner_buffer_mut(&mut self) -> &mut [u8] {
            &mut self.bits
        }
    }

    impl BootCfg0 {
        /// Create a new instance, loaded with the reset value (if any)
        pub const fn new() -> Self {
            Self { bits: [0, 0, 0, 0] }
        }
        /// Create a new instance, loaded with all zeroes
        pub const fn new_zero() -> Self {
            Self { bits: [0; 4] }
        }

        ///Read the `primary_boot_src` field of the register.
        ///

        pub fn primary_boot_src(&self) -> Result<super::BootSrc, <super::BootSrc as TryFrom<u8>>::Error> {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 0, 4) };

            raw.try_into()
        }

        ///Read the `default_isp_mode` field of the register.
        ///

        pub fn default_isp_mode(&self) -> Result<super::DefaultIspMode, <super::DefaultIspMode as TryFrom<u8>>::Error> {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 4, 7) };

            raw.try_into()
        }

        ///Read the `boot_clk_speed` field of the register.
        ///

        pub fn boot_clk_speed(&self) -> super::BootClkSpeed {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 7, 8) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `rsa_4_k_en` field of the register.
        ///
        /// Use 4096 bit RSA keys only for certificate validations. By default the ROM assume 2048-bit keys.

        pub fn rsa_4_k_en(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 8, 9) };

            raw > 0
        }

        ///Read the `tzm_image_type` field of the register.
        ///

        pub fn tzm_image_type(&self) -> super::TzmImageType {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 13, 15) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `psa_bstate_skip` field of the register.
        ///
        /// If set, ROM skips computation of boot state defined by PSA specification. As part of boot state computation ROM includes OTP words
        /// - Shadow register values of 95 to 104
        /// - Fuse values of words 128 to 147

        pub fn psa_bstate_skip(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 15, 16) };

            raw > 0
        }

        ///Read the `psa_bstate_inc_keys` field of the register.
        ///
        /// If set, boot state computation includes OTP shadow register values of words 106 to 127.

        pub fn psa_bstate_inc_keys(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 16, 17) };

            raw > 0
        }

        ///Read the `redundant_spi_port` field of the register.
        ///

        pub fn redundant_spi_port(&self) -> super::RedundantSpiPort {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 17, 20) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `secure_boot_en` field of the register.
        ///

        pub fn secure_boot_en(&self) -> super::SecureBoot {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 20, 22) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `dice_inc_otp` field of the register.
        ///
        /// Include non-field updatable OTP Fields in DICE computation. OTP values in shadow registers are used in computation for words 95, 96, 98, 99, 104, 120 - 127.

        pub fn dice_inc_otp(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 22, 23) };

            raw > 0
        }

        ///Read the `dice_skip` field of the register.
        ///
        /// If set, ROM skips computation of Composite Device Identifier (CDI) defined in DICE specification. But ROM will continue to hide UDS source in OTP and PUF (index 15) before passing control to user code.

        pub fn dice_skip(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 23, 24) };

            raw > 0
        }

        ///Read the `boot_fail_pin_port` field of the register.
        ///
        /// GPIO port to use for indicating boot failure. Boot ROM will drive this pin high before locking the chip on error conditions. Applications can use this pin to power cycle the system.

        pub fn boot_fail_pin_port(&self) -> u8 {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 24, 27) };

            raw
        }

        ///Read the `boot_fail_pin_num` field of the register.
        ///
        /// GPIO pin number to use for indicating boot failure. Boot ROM will drive this pin high before locking the chip on error conditions. Applications can use this pin to power cycle the system.

        pub fn boot_fail_pin_num(&self) -> u8 {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 27, 32) };

            raw
        }

        ///Write the `primary_boot_src` field of the register.
        ///

        pub fn set_primary_boot_src(&mut self, value: super::BootSrc) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 0, 4, &mut self.bits) };
        }

        ///Write the `default_isp_mode` field of the register.
        ///

        pub fn set_default_isp_mode(&mut self, value: super::DefaultIspMode) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 4, 7, &mut self.bits) };
        }

        ///Write the `boot_clk_speed` field of the register.
        ///

        pub fn set_boot_clk_speed(&mut self, value: super::BootClkSpeed) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 7, 8, &mut self.bits) };
        }

        ///Write the `rsa_4_k_en` field of the register.
        ///
        /// Use 4096 bit RSA keys only for certificate validations. By default the ROM assume 2048-bit keys.

        pub fn set_rsa_4_k_en(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 8, 9, &mut self.bits) };
        }

        ///Write the `tzm_image_type` field of the register.
        ///

        pub fn set_tzm_image_type(&mut self, value: super::TzmImageType) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 13, 15, &mut self.bits) };
        }

        ///Write the `psa_bstate_skip` field of the register.
        ///
        /// If set, ROM skips computation of boot state defined by PSA specification. As part of boot state computation ROM includes OTP words
        /// - Shadow register values of 95 to 104
        /// - Fuse values of words 128 to 147

        pub fn set_psa_bstate_skip(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 15, 16, &mut self.bits) };
        }

        ///Write the `psa_bstate_inc_keys` field of the register.
        ///
        /// If set, boot state computation includes OTP shadow register values of words 106 to 127.

        pub fn set_psa_bstate_inc_keys(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 16, 17, &mut self.bits) };
        }

        ///Write the `redundant_spi_port` field of the register.
        ///

        pub fn set_redundant_spi_port(&mut self, value: super::RedundantSpiPort) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 17, 20, &mut self.bits) };
        }

        ///Write the `secure_boot_en` field of the register.
        ///

        pub fn set_secure_boot_en(&mut self, value: super::SecureBoot) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 20, 22, &mut self.bits) };
        }

        ///Write the `dice_inc_otp` field of the register.
        ///
        /// Include non-field updatable OTP Fields in DICE computation. OTP values in shadow registers are used in computation for words 95, 96, 98, 99, 104, 120 - 127.

        pub fn set_dice_inc_otp(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 22, 23, &mut self.bits) };
        }

        ///Write the `dice_skip` field of the register.
        ///
        /// If set, ROM skips computation of Composite Device Identifier (CDI) defined in DICE specification. But ROM will continue to hide UDS source in OTP and PUF (index 15) before passing control to user code.

        pub fn set_dice_skip(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 23, 24, &mut self.bits) };
        }

        ///Write the `boot_fail_pin_port` field of the register.
        ///
        /// GPIO port to use for indicating boot failure. Boot ROM will drive this pin high before locking the chip on error conditions. Applications can use this pin to power cycle the system.

        pub fn set_boot_fail_pin_port(&mut self, value: u8) {
            let raw = value;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 24, 27, &mut self.bits) };
        }

        ///Write the `boot_fail_pin_num` field of the register.
        ///
        /// GPIO pin number to use for indicating boot failure. Boot ROM will drive this pin high before locking the chip on error conditions. Applications can use this pin to power cycle the system.

        pub fn set_boot_fail_pin_num(&mut self, value: u8) {
            let raw = value;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 27, 32, &mut self.bits) };
        }
    }

    impl From<[u8; 4]> for BootCfg0 {
        fn from(bits: [u8; 4]) -> Self {
            Self { bits }
        }
    }

    impl From<BootCfg0> for [u8; 4] {
        fn from(val: BootCfg0) -> Self {
            val.bits
        }
    }

    impl core::fmt::Debug for BootCfg0 {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            let mut d = f.debug_struct("BootCfg0");

            d.field("primary_boot_src", &self.primary_boot_src());

            d.field("default_isp_mode", &self.default_isp_mode());

            d.field("boot_clk_speed", &self.boot_clk_speed());

            d.field("rsa_4_k_en", &self.rsa_4_k_en());

            d.field("tzm_image_type", &self.tzm_image_type());

            d.field("psa_bstate_skip", &self.psa_bstate_skip());

            d.field("psa_bstate_inc_keys", &self.psa_bstate_inc_keys());

            d.field("redundant_spi_port", &self.redundant_spi_port());

            d.field("secure_boot_en", &self.secure_boot_en());

            d.field("dice_inc_otp", &self.dice_inc_otp());

            d.field("dice_skip", &self.dice_skip());

            d.field("boot_fail_pin_port", &self.boot_fail_pin_port());

            d.field("boot_fail_pin_num", &self.boot_fail_pin_num());

            d.finish()
        }
    }

    #[cfg(feature = "defmt")]
    impl defmt::Format for BootCfg0 {
        fn format(&self, f: defmt::Formatter) {
            defmt::write!(f, "BootCfg0 {{ ");

            defmt::write!(f, "primary_boot_src: {}, ", &self.primary_boot_src());

            defmt::write!(f, "default_isp_mode: {}, ", &self.default_isp_mode());

            defmt::write!(f, "boot_clk_speed: {}, ", &self.boot_clk_speed());

            defmt::write!(f, "rsa_4_k_en: {=bool}, ", &self.rsa_4_k_en());

            defmt::write!(f, "tzm_image_type: {}, ", &self.tzm_image_type());

            defmt::write!(f, "psa_bstate_skip: {=bool}, ", &self.psa_bstate_skip());

            defmt::write!(f, "psa_bstate_inc_keys: {=bool}, ", &self.psa_bstate_inc_keys());

            defmt::write!(f, "redundant_spi_port: {}, ", &self.redundant_spi_port());

            defmt::write!(f, "secure_boot_en: {}, ", &self.secure_boot_en());

            defmt::write!(f, "dice_inc_otp: {=bool}, ", &self.dice_inc_otp());

            defmt::write!(f, "dice_skip: {=bool}, ", &self.dice_skip());

            defmt::write!(f, "boot_fail_pin_port: {=u8}, ", &self.boot_fail_pin_port());

            defmt::write!(f, "boot_fail_pin_num: {=u8}, ", &self.boot_fail_pin_num());

            defmt::write!(f, "}}");
        }
    }

    impl core::ops::BitAnd for BootCfg0 {
        type Output = Self;
        fn bitand(mut self, rhs: Self) -> Self::Output {
            self &= rhs;
            self
        }
    }

    impl core::ops::BitAndAssign for BootCfg0 {
        fn bitand_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l &= *r;
            }
        }
    }

    impl core::ops::BitOr for BootCfg0 {
        type Output = Self;
        fn bitor(mut self, rhs: Self) -> Self::Output {
            self |= rhs;
            self
        }
    }

    impl core::ops::BitOrAssign for BootCfg0 {
        fn bitor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l |= *r;
            }
        }
    }

    impl core::ops::BitXor for BootCfg0 {
        type Output = Self;
        fn bitxor(mut self, rhs: Self) -> Self::Output {
            self ^= rhs;
            self
        }
    }

    impl core::ops::BitXorAssign for BootCfg0 {
        fn bitxor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l ^= *r;
            }
        }
    }

    impl core::ops::Not for BootCfg0 {
        type Output = Self;
        fn not(mut self) -> Self::Output {
            for val in self.bits.iter_mut() {
                *val = !*val;
            }
            self
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct BootCfg1 {
        /// The internal bits
        bits: [u8; 4],
    }

    impl ::device_driver::FieldSet for BootCfg1 {
        const SIZE_BITS: u32 = 32;
        fn new_with_zero() -> Self {
            Self::new_zero()
        }
        fn get_inner_buffer(&self) -> &[u8] {
            &self.bits
        }
        fn get_inner_buffer_mut(&mut self) -> &mut [u8] {
            &mut self.bits
        }
    }

    impl BootCfg1 {
        /// Create a new instance, loaded with the reset value (if any)
        pub const fn new() -> Self {
            Self { bits: [0, 0, 0, 0] }
        }
        /// Create a new instance, loaded with all zeroes
        pub const fn new_zero() -> Self {
            Self { bits: [0; 4] }
        }

        ///Read the `qspi_reset_pin_enable` field of the register.
        ///
        /// Use QSPI_RESET_PIN to reset the flash device.

        pub fn qspi_reset_pin_enable(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 14, 15) };

            raw > 0
        }

        ///Read the `qspi_reset_pin_port` field of the register.
        ///
        /// GPIO port to use for O/QSPI reset function.

        pub fn qspi_reset_pin_port(&self) -> u8 {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 15, 18) };

            raw
        }

        ///Read the `qspi_reset_pin_num` field of the register.
        ///
        /// GPIO pin number to use for O/QSPI reset function.

        pub fn qspi_reset_pin_num(&self) -> u8 {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 18, 23) };

            raw
        }

        ///Write the `qspi_reset_pin_enable` field of the register.
        ///
        /// Use QSPI_RESET_PIN to reset the flash device.

        pub fn set_qspi_reset_pin_enable(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 14, 15, &mut self.bits) };
        }

        ///Write the `qspi_reset_pin_port` field of the register.
        ///
        /// GPIO port to use for O/QSPI reset function.

        pub fn set_qspi_reset_pin_port(&mut self, value: u8) {
            let raw = value;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 15, 18, &mut self.bits) };
        }

        ///Write the `qspi_reset_pin_num` field of the register.
        ///
        /// GPIO pin number to use for O/QSPI reset function.

        pub fn set_qspi_reset_pin_num(&mut self, value: u8) {
            let raw = value;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 18, 23, &mut self.bits) };
        }
    }

    impl From<[u8; 4]> for BootCfg1 {
        fn from(bits: [u8; 4]) -> Self {
            Self { bits }
        }
    }

    impl From<BootCfg1> for [u8; 4] {
        fn from(val: BootCfg1) -> Self {
            val.bits
        }
    }

    impl core::fmt::Debug for BootCfg1 {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            let mut d = f.debug_struct("BootCfg1");

            d.field("qspi_reset_pin_enable", &self.qspi_reset_pin_enable());

            d.field("qspi_reset_pin_port", &self.qspi_reset_pin_port());

            d.field("qspi_reset_pin_num", &self.qspi_reset_pin_num());

            d.finish()
        }
    }

    #[cfg(feature = "defmt")]
    impl defmt::Format for BootCfg1 {
        fn format(&self, f: defmt::Formatter) {
            defmt::write!(f, "BootCfg1 {{ ");

            defmt::write!(f, "qspi_reset_pin_enable: {=bool}, ", &self.qspi_reset_pin_enable());

            defmt::write!(f, "qspi_reset_pin_port: {=u8}, ", &self.qspi_reset_pin_port());

            defmt::write!(f, "qspi_reset_pin_num: {=u8}, ", &self.qspi_reset_pin_num());

            defmt::write!(f, "}}");
        }
    }

    impl core::ops::BitAnd for BootCfg1 {
        type Output = Self;
        fn bitand(mut self, rhs: Self) -> Self::Output {
            self &= rhs;
            self
        }
    }

    impl core::ops::BitAndAssign for BootCfg1 {
        fn bitand_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l &= *r;
            }
        }
    }

    impl core::ops::BitOr for BootCfg1 {
        type Output = Self;
        fn bitor(mut self, rhs: Self) -> Self::Output {
            self |= rhs;
            self
        }
    }

    impl core::ops::BitOrAssign for BootCfg1 {
        fn bitor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l |= *r;
            }
        }
    }

    impl core::ops::BitXor for BootCfg1 {
        type Output = Self;
        fn bitxor(mut self, rhs: Self) -> Self::Output {
            self ^= rhs;
            self
        }
    }

    impl core::ops::BitXorAssign for BootCfg1 {
        fn bitxor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l ^= *r;
            }
        }
    }

    impl core::ops::Not for BootCfg1 {
        type Output = Self;
        fn not(mut self) -> Self::Output {
            for val in self.bits.iter_mut() {
                *val = !*val;
            }
            self
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct SecBootCfg5 {
        /// The internal bits
        bits: [u8; 2],
    }

    impl ::device_driver::FieldSet for SecBootCfg5 {
        const SIZE_BITS: u32 = 16;
        fn new_with_zero() -> Self {
            Self::new_zero()
        }
        fn get_inner_buffer(&self) -> &[u8] {
            &self.bits
        }
        fn get_inner_buffer_mut(&mut self) -> &mut [u8] {
            &mut self.bits
        }
    }

    impl SecBootCfg5 {
        /// Create a new instance, loaded with the reset value (if any)
        pub const fn new() -> Self {
            Self { bits: [0, 0] }
        }
        /// Create a new instance, loaded with all zeroes
        pub const fn new_zero() -> Self {
            Self { bits: [0; 2] }
        }

        ///Read the `revoke_rootkey` field of the register.
        ///
        /// Revoke upto 4 root keys. When a bit is set corresponding root key is revoked.

        pub fn revoke_rootkey(&self) -> u8 {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 0, 4) };

            raw
        }

        ///Read the `fa_mode_en` field of the register.
        ///
        /// Enable Fault Analysis mode.
        /// - When set ROM checks and erases customer sensitive assets (AES keys or key codes) stored in IFR/OTP.
        /// - Issues zeroized command to PUF (disables key decoding until POR).
        /// - Blocks all HW routed OTP keys and set lock bits on those registers.
        /// - Enables all debug ports and waits in loop for tester.

        pub fn fa_mode_en(&self) -> bool {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 4, 5) };

            raw > 0
        }

        ///Read the `enable_crc_check` field of the register.
        ///
        /// Enable CRC checking of OTP words.

        pub fn enable_crc_check(&self) -> super::CrcCheck {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 5, 7) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `use_puf` field of the register.
        ///
        /// Use PUF to store AES keys and UDS.

        pub fn use_puf(&self) -> super::KeyIn {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 7, 8) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `puf_block_enroll` field of the register.
        ///
        /// Block further enrollement of PUF block. When this bit is set ROM blocks generation of new activation codes.

        pub fn puf_block_enroll(&self) -> super::Enroll {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 8, 9) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Read the `puf_block_set_key` field of the register.
        ///
        /// Block further enrollement of PUF block. When this bit is set ROM blocks generation of new key codes.

        pub fn puf_block_set_key(&self) -> super::KeyGen {
            let raw = unsafe { ::device_driver::ops::load_lsb0::<u8, ::device_driver::ops::LE>(&self.bits, 9, 10) };

            unsafe { raw.try_into().unwrap_unchecked() }
        }

        ///Write the `revoke_rootkey` field of the register.
        ///
        /// Revoke upto 4 root keys. When a bit is set corresponding root key is revoked.

        pub fn set_revoke_rootkey(&mut self, value: u8) {
            let raw = value;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 0, 4, &mut self.bits) };
        }

        ///Write the `fa_mode_en` field of the register.
        ///
        /// Enable Fault Analysis mode.
        /// - When set ROM checks and erases customer sensitive assets (AES keys or key codes) stored in IFR/OTP.
        /// - Issues zeroized command to PUF (disables key decoding until POR).
        /// - Blocks all HW routed OTP keys and set lock bits on those registers.
        /// - Enables all debug ports and waits in loop for tester.

        pub fn set_fa_mode_en(&mut self, value: bool) {
            let raw = value as _;

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 4, 5, &mut self.bits) };
        }

        ///Write the `enable_crc_check` field of the register.
        ///
        /// Enable CRC checking of OTP words.

        pub fn set_enable_crc_check(&mut self, value: super::CrcCheck) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 5, 7, &mut self.bits) };
        }

        ///Write the `use_puf` field of the register.
        ///
        /// Use PUF to store AES keys and UDS.

        pub fn set_use_puf(&mut self, value: super::KeyIn) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 7, 8, &mut self.bits) };
        }

        ///Write the `puf_block_enroll` field of the register.
        ///
        /// Block further enrollement of PUF block. When this bit is set ROM blocks generation of new activation codes.

        pub fn set_puf_block_enroll(&mut self, value: super::Enroll) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 8, 9, &mut self.bits) };
        }

        ///Write the `puf_block_set_key` field of the register.
        ///
        /// Block further enrollement of PUF block. When this bit is set ROM blocks generation of new key codes.

        pub fn set_puf_block_set_key(&mut self, value: super::KeyGen) {
            let raw = value.into();

            unsafe { ::device_driver::ops::store_lsb0::<u8, ::device_driver::ops::LE>(raw, 9, 10, &mut self.bits) };
        }
    }

    impl From<[u8; 2]> for SecBootCfg5 {
        fn from(bits: [u8; 2]) -> Self {
            Self { bits }
        }
    }

    impl From<SecBootCfg5> for [u8; 2] {
        fn from(val: SecBootCfg5) -> Self {
            val.bits
        }
    }

    impl core::fmt::Debug for SecBootCfg5 {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            let mut d = f.debug_struct("SecBootCfg5");

            d.field("revoke_rootkey", &self.revoke_rootkey());

            d.field("fa_mode_en", &self.fa_mode_en());

            d.field("enable_crc_check", &self.enable_crc_check());

            d.field("use_puf", &self.use_puf());

            d.field("puf_block_enroll", &self.puf_block_enroll());

            d.field("puf_block_set_key", &self.puf_block_set_key());

            d.finish()
        }
    }

    #[cfg(feature = "defmt")]
    impl defmt::Format for SecBootCfg5 {
        fn format(&self, f: defmt::Formatter) {
            defmt::write!(f, "SecBootCfg5 {{ ");

            defmt::write!(f, "revoke_rootkey: {=u8}, ", &self.revoke_rootkey());

            defmt::write!(f, "fa_mode_en: {=bool}, ", &self.fa_mode_en());

            defmt::write!(f, "enable_crc_check: {}, ", &self.enable_crc_check());

            defmt::write!(f, "use_puf: {}, ", &self.use_puf());

            defmt::write!(f, "puf_block_enroll: {}, ", &self.puf_block_enroll());

            defmt::write!(f, "puf_block_set_key: {}, ", &self.puf_block_set_key());

            defmt::write!(f, "}}");
        }
    }

    impl core::ops::BitAnd for SecBootCfg5 {
        type Output = Self;
        fn bitand(mut self, rhs: Self) -> Self::Output {
            self &= rhs;
            self
        }
    }

    impl core::ops::BitAndAssign for SecBootCfg5 {
        fn bitand_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l &= *r;
            }
        }
    }

    impl core::ops::BitOr for SecBootCfg5 {
        type Output = Self;
        fn bitor(mut self, rhs: Self) -> Self::Output {
            self |= rhs;
            self
        }
    }

    impl core::ops::BitOrAssign for SecBootCfg5 {
        fn bitor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l |= *r;
            }
        }
    }

    impl core::ops::BitXor for SecBootCfg5 {
        type Output = Self;
        fn bitxor(mut self, rhs: Self) -> Self::Output {
            self ^= rhs;
            self
        }
    }

    impl core::ops::BitXorAssign for SecBootCfg5 {
        fn bitxor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l ^= *r;
            }
        }
    }

    impl core::ops::Not for SecBootCfg5 {
        type Output = Self;
        fn not(mut self) -> Self::Output {
            for val in self.bits.iter_mut() {
                *val = !*val;
            }
            self
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct Rkth {
        /// The internal bits
        bits: [u8; 32],
    }

    impl ::device_driver::FieldSet for Rkth {
        const SIZE_BITS: u32 = 256;
        fn new_with_zero() -> Self {
            Self::new_zero()
        }
        fn get_inner_buffer(&self) -> &[u8] {
            &self.bits
        }
        fn get_inner_buffer_mut(&mut self) -> &mut [u8] {
            &mut self.bits
        }
    }

    impl Rkth {
        /// Create a new instance, loaded with the reset value (if any)
        pub const fn new() -> Self {
            Self {
                bits: [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
            }
        }
        /// Create a new instance, loaded with all zeroes
        pub const fn new_zero() -> Self {
            Self { bits: [0; 32] }
        }
    }

    impl From<[u8; 32]> for Rkth {
        fn from(bits: [u8; 32]) -> Self {
            Self { bits }
        }
    }

    impl From<Rkth> for [u8; 32] {
        fn from(val: Rkth) -> Self {
            val.bits
        }
    }

    impl core::fmt::Debug for Rkth {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            let mut d = f.debug_struct("Rkth");

            d.finish()
        }
    }

    #[cfg(feature = "defmt")]
    impl defmt::Format for Rkth {
        fn format(&self, f: defmt::Formatter) {
            defmt::write!(f, "Rkth {{ ");

            defmt::write!(f, "}}");
        }
    }

    impl core::ops::BitAnd for Rkth {
        type Output = Self;
        fn bitand(mut self, rhs: Self) -> Self::Output {
            self &= rhs;
            self
        }
    }

    impl core::ops::BitAndAssign for Rkth {
        fn bitand_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l &= *r;
            }
        }
    }

    impl core::ops::BitOr for Rkth {
        type Output = Self;
        fn bitor(mut self, rhs: Self) -> Self::Output {
            self |= rhs;
            self
        }
    }

    impl core::ops::BitOrAssign for Rkth {
        fn bitor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l |= *r;
            }
        }
    }

    impl core::ops::BitXor for Rkth {
        type Output = Self;
        fn bitxor(mut self, rhs: Self) -> Self::Output {
            self ^= rhs;
            self
        }
    }

    impl core::ops::BitXorAssign for Rkth {
        fn bitxor_assign(&mut self, rhs: Self) {
            for (l, r) in self.bits.iter_mut().zip(&rhs.bits) {
                *l ^= *r;
            }
        }
    }

    impl core::ops::Not for Rkth {
        type Output = Self;
        fn not(mut self) -> Self::Output {
            for val in self.bits.iter_mut() {
                *val = !*val;
            }
            self
        }
    }

    /// Enum containing all possible field set types
    pub enum FieldSetValue {
        BootCfg0(BootCfg0),

        BootCfg1(BootCfg1),

        SecBootCfg5(SecBootCfg5),

        Rkth(Rkth),
    }
    impl core::fmt::Debug for FieldSetValue {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Self::BootCfg0(val) => core::fmt::Debug::fmt(val, f),

                Self::BootCfg1(val) => core::fmt::Debug::fmt(val, f),

                Self::SecBootCfg5(val) => core::fmt::Debug::fmt(val, f),

                Self::Rkth(val) => core::fmt::Debug::fmt(val, f),

                #[allow(unreachable_patterns)]
                _ => unreachable!(),
            }
        }
    }

    #[cfg(feature = "defmt")]
    impl defmt::Format for FieldSetValue {
        fn format(&self, f: defmt::Formatter) {
            match self {
                Self::BootCfg0(val) => defmt::Format::format(val, f),

                Self::BootCfg1(val) => defmt::Format::format(val, f),

                Self::SecBootCfg5(val) => defmt::Format::format(val, f),

                Self::Rkth(val) => defmt::Format::format(val, f),
            }
        }
    }

    impl From<BootCfg0> for FieldSetValue {
        fn from(val: BootCfg0) -> Self {
            Self::BootCfg0(val)
        }
    }

    impl From<BootCfg1> for FieldSetValue {
        fn from(val: BootCfg1) -> Self {
            Self::BootCfg1(val)
        }
    }

    impl From<SecBootCfg5> for FieldSetValue {
        fn from(val: SecBootCfg5) -> Self {
            Self::SecBootCfg5(val)
        }
    }

    impl From<Rkth> for FieldSetValue {
        fn from(val: Rkth) -> Self {
            Self::Rkth(val)
        }
    }
}

/// Primary boot Source. (a.k.a. Master boot source)

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum BootSrc {
    /// ISP pins will determine boot source.
    IspPinBoot = 0,

    /// Boot from Octal/Quad SPI flash device using FlexSpi channel A interface pins.
    QpsiABoot = 1,

    /// Boot from eMMC device or SD card connected to SDHC0 port.
    Sdhc0Boot = 2,

    /// Boot from eMMC device or SD card connected to SDHC0 port.
    Sdhc1Boot = 3,

    /// Boot using SPI slave interface using master boot mode.
    SpiSlvBoot = 4,

    /// Boot from Octal/Quad SPI flash device using FlexSpi channel B interface pins. Only load-to-RAM image are supported in this mode.
    QspiBBoot = 5,

    /// Boot using UART interface using master boot mode.
    UartBoot = 6,

    /// Boot from 1-bit SPI flash device from FlexCom interface pins selected by REDUNDANT_SPI_PORT field. Only load-to-RAM images are supported in this mode.
    SpiFcBoot = 7,

    /// Always enter ISP mode. DEFAULT_ISP_MODE field will determine the ISP interface.
    IspMode = 9,

    /// Boot from Octal/Quad SPI flash device using FlexSPI channel B interface pins. If image is not found check recovery boot using SPI-flash device through FlexComm.
    QspiBRecBoot = 11,

    /// Boot from Octal/Quad SPI flash device using FlexSPI channel A interface pins. If image is not found check recovery boot using SPI-flash device through FlexComm.
    QspiARecBoot = 12,

    /// Boot from SDHC0 port device. If image is not found check recovery boot using SPI-flash device through FlexComm.
    Sdhc0RecBoot = 13,

    /// Boot from SDHC1 port device. If image is not found check recovery boot using SPI-flash device  through FlexComm.
    Sdhc1RecBoot = 15,
}

impl core::convert::TryFrom<u8> for BootSrc {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::IspPinBoot),

            1 => Ok(Self::QpsiABoot),

            2 => Ok(Self::Sdhc0Boot),

            3 => Ok(Self::Sdhc1Boot),

            4 => Ok(Self::SpiSlvBoot),

            5 => Ok(Self::QspiBBoot),

            6 => Ok(Self::UartBoot),

            7 => Ok(Self::SpiFcBoot),

            9 => Ok(Self::IspMode),

            11 => Ok(Self::QspiBRecBoot),

            12 => Ok(Self::QspiARecBoot),

            13 => Ok(Self::Sdhc0RecBoot),

            15 => Ok(Self::Sdhc1RecBoot),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "BootSrc",
            }),
        }
    }
}

impl From<BootSrc> for u8 {
    fn from(val: BootSrc) -> Self {
        match val {
            BootSrc::IspPinBoot => 0,

            BootSrc::QpsiABoot => 1,

            BootSrc::Sdhc0Boot => 2,

            BootSrc::Sdhc1Boot => 3,

            BootSrc::SpiSlvBoot => 4,

            BootSrc::QspiBBoot => 5,

            BootSrc::UartBoot => 6,

            BootSrc::SpiFcBoot => 7,

            BootSrc::IspMode => 9,

            BootSrc::QspiBRecBoot => 11,

            BootSrc::QspiARecBoot => 12,

            BootSrc::Sdhc0RecBoot => 13,

            BootSrc::Sdhc1RecBoot => 15,
        }
    }
}

/// When a valid image is not available to master boot, ROM switches to ISP mode for programming primary boot devices. This field determines the default ISP mode.

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum DefaultIspMode {
    /// Auto detect ISP mode. ROM monitors USB, UART, SPI and I2C interfaces for any activity.
    AutoIsp = 0,

    /// Support ISP command interface using USB HID class only.
    UsbHidIsp = 1,

    /// Support ISP command interface on UART port only.
    UartIsp = 2,

    /// Support ISP command interface on SPI port only.
    SpiIsp = 3,

    /// Support ISP command interface on I2C port only.
    I2CIsp = 4,

    /// Disable ISP fall through when proper image is not found on primary boot device.
    DisableIsp = 7,
}

impl core::convert::TryFrom<u8> for DefaultIspMode {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::AutoIsp),

            1 => Ok(Self::UsbHidIsp),

            2 => Ok(Self::UartIsp),

            3 => Ok(Self::SpiIsp),

            4 => Ok(Self::I2CIsp),

            7 => Ok(Self::DisableIsp),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "DefaultIspMode",
            }),
        }
    }
}

impl From<DefaultIspMode> for u8 {
    fn from(val: DefaultIspMode) -> Self {
        match val {
            DefaultIspMode::AutoIsp => 0,

            DefaultIspMode::UsbHidIsp => 1,

            DefaultIspMode::UartIsp => 2,

            DefaultIspMode::SpiIsp => 3,

            DefaultIspMode::I2CIsp => 4,

            DefaultIspMode::DisableIsp => 7,
        }
    }
}

/// Defines clock speeds during boot.

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum BootClkSpeed {
    /// Normal boot. All clocks are set to 48MHz using IRC48M, except USB block. USB block will use external XTAL clock.
    NormalClk = 0,

    /// High-speed boot.
    /// * Core clock is set to 198MHz using main_pll with IRC48M as input
    /// * UART, I2C : 48MHz (IRC48M)
    /// * SPI, SDHC: 198MHz (main_pll)
    /// * USB: external XTAL
    /// * OSPI: Set to differnet speed using aux0_pll. Speed of OSPI interface is obtained from Boot Configuration Block present on OSPI-flash device.
    ///     - SDR: 30/50/60/72/80/90/100 MHz
    ///     - DDR: 30/50/60/72/80 MHz
    HispeedClk = 1,
}

impl core::convert::TryFrom<u8> for BootClkSpeed {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::NormalClk),

            1 => Ok(Self::HispeedClk),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "BootClkSpeed",
            }),
        }
    }
}

impl From<BootClkSpeed> for u8 {
    fn from(val: BootClkSpeed) -> Self {
        match val {
            BootClkSpeed::NormalClk => 0,

            BootClkSpeed::HispeedClk => 1,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum TzmImageType {
    /// TrustZone-M mode is determined by the image header.
    TzmNormal = 0,

    /// Disable TrustZone-M features. ROM will always boot to a non-secure code and all TZ-M features are disabled.
    TzmDisable = 1,

    /// TrustZone-M features are enabled. ROM will always boot to secure code.
    TzmEnable = 2,

    /// TrustZone-M features are enabled and setting are loaded from image header and locked before branching to user code.
    TzmPreset = 3,
}

impl core::convert::TryFrom<u8> for TzmImageType {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::TzmNormal),

            1 => Ok(Self::TzmDisable),

            2 => Ok(Self::TzmEnable),

            3 => Ok(Self::TzmPreset),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "TzmImageType",
            }),
        }
    }
}

impl From<TzmImageType> for u8 {
    fn from(val: TzmImageType) -> Self {
        match val {
            TzmImageType::TzmNormal => 0,

            TzmImageType::TzmDisable => 1,

            TzmImageType::TzmEnable => 2,

            TzmImageType::TzmPreset => 3,
        }
    }
}

/// FlexComm port to use for redundant SPI flash boot.

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum RedundantSpiPort {
    /// Use FlexCom0 pins P0_0 (SCK), P0_1 (MISO), P0_2 (MOSI), P0_3 (SEL)
    Fc0 = 0,

    /// Use FlexCom1 pins P0_7 (SCK), P0_8 (MISO), P0_9 (MOSI), P0_10 (SEL)
    Fc1 = 1,

    /// Use FlexCom2 pins P0_14 (SCK), P0_15 (MISO), P0_16 (MOSI), P0_17 (SEL)
    Fc2 = 2,

    /// Use FlexCom3 pins P0_21 (SCK), P0_22 (MISO), P0_23 (MOSI), P0_24 (SEL)
    Fc3 = 3,

    /// Use FlexCom4 pins P0_28 (SCK), P0_29 (MISO), P0_30 (MOSI), P0_31 (SEL)
    Fc4 = 4,

    /// Use FlexCom5 pins P1_3 (SCK), P1_4 (MISO), P1_5 (MOSI), P1_6 (SEL)
    Fc5 = 5,

    /// Use FlexCom6 pins P3_25 (SCK), P3_26 (MISO), P3_27 (MOSI), P3_28 (SEL)
    Fc6 = 6,

    /// Use FlexCom7 pins P4_0 (SCK), P4_1 (MISO), P4_2 (MOSI), P4_3 (SEL)
    Fc7 = 7,
}

impl core::convert::TryFrom<u8> for RedundantSpiPort {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Fc0),

            1 => Ok(Self::Fc1),

            2 => Ok(Self::Fc2),

            3 => Ok(Self::Fc3),

            4 => Ok(Self::Fc4),

            5 => Ok(Self::Fc5),

            6 => Ok(Self::Fc6),

            7 => Ok(Self::Fc7),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "RedundantSpiPort",
            }),
        }
    }
}

impl From<RedundantSpiPort> for u8 {
    fn from(val: RedundantSpiPort) -> Self {
        match val {
            RedundantSpiPort::Fc0 => 0,

            RedundantSpiPort::Fc1 => 1,

            RedundantSpiPort::Fc2 => 2,

            RedundantSpiPort::Fc3 => 3,

            RedundantSpiPort::Fc4 => 4,

            RedundantSpiPort::Fc5 => 5,

            RedundantSpiPort::Fc6 => 6,

            RedundantSpiPort::Fc7 => 7,
        }
    }
}

/// Force secure image only.

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum SecureBoot {
    Disabled = 0,

    Enabled = 1,
}

impl Default for SecureBoot {
    fn default() -> Self {
        Self::Enabled
    }
}

impl From<u8> for SecureBoot {
    fn from(val: u8) -> Self {
        match val {
            0 => Self::Disabled,

            _ => Self::default(),
        }
    }
}

impl From<SecureBoot> for u8 {
    fn from(val: SecureBoot) -> Self {
        match val {
            SecureBoot::Disabled => 0,

            SecureBoot::Enabled => 1,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum CrcCheck {
    Disable = 0,

    Enable = 1,

    NxpOnly = 2,

    Enable2 = 3,
}

impl core::convert::TryFrom<u8> for CrcCheck {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Disable),

            1 => Ok(Self::Enable),

            2 => Ok(Self::NxpOnly),

            3 => Ok(Self::Enable2),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "CrcCheck",
            }),
        }
    }
}

impl From<CrcCheck> for u8 {
    fn from(val: CrcCheck) -> Self {
        match val {
            CrcCheck::Disable => 0,

            CrcCheck::Enable => 1,

            CrcCheck::NxpOnly => 2,

            CrcCheck::Enable2 => 3,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum KeyIn {
    Otp = 0,

    Ouf = 1,
}

impl core::convert::TryFrom<u8> for KeyIn {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Otp),

            1 => Ok(Self::Ouf),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "KeyIn",
            }),
        }
    }
}

impl From<KeyIn> for u8 {
    fn from(val: KeyIn) -> Self {
        match val {
            KeyIn::Otp => 0,

            KeyIn::Ouf => 1,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum Enroll {
    Enable = 0,

    Disable = 1,
}

impl core::convert::TryFrom<u8> for Enroll {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Enable),

            1 => Ok(Self::Disable),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "Enroll",
            }),
        }
    }
}

impl From<Enroll> for u8 {
    fn from(val: Enroll) -> Self {
        match val {
            Enroll::Enable => 0,

            Enroll::Disable => 1,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]

pub enum KeyGen {
    Enable = 0,

    Disable = 1,
}

impl core::convert::TryFrom<u8> for KeyGen {
    type Error = ::device_driver::ConversionError<u8>;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
            0 => Ok(Self::Enable),

            1 => Ok(Self::Disable),

            val => Err(::device_driver::ConversionError {
                source: val,
                target: "KeyGen",
            }),
        }
    }
}

impl From<KeyGen> for u8 {
    fn from(val: KeyGen) -> Self {
        match val {
            KeyGen::Enable => 0,

            KeyGen::Disable => 1,
        }
    }
}
