#![allow(clippy::not_unsafe_ptr_arg_deref)]
// Safety: null, misaligned, invalid inputs or out of bounds references will cause ROM API to return an error code, which is handled by the caller.
use super::Status;
use crate::error::FlexspiStatus;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SerialNorOptionTag {
    Config = 0x0C, // SDK vs. RM mismatch; TODO: confirm correct value and semantics.
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorOptionSize {
    Option0Only = 0,
    Option0AndOption1 = 1,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorDeviceType {
    ReadSfdpSdr = 0,
    ReadSfdpDdr = 1,
    Hyperflash1V8 = 2,
    Hyperflash3V0 = 3,
    MacronixOctalDdr = 4,
    MacronixOctalSdr = 5,
    MicronOctalDdr = 6,
    MicronOctalSdr = 7,
    AdestoOctalDdr = 8,
    AdestoOctalSdr = 9,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorOptionPadEncoding {
    // Encoded values for option0.query_pads / option0.cmd_pads.
    // These match the ROM option field encoding, not the literal kSerialFlash_*Pad values.
    One = 0,
    Four = 2,
    Eight = 3,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorQuadModeSetting {
    NotConfigured = 0,
    StatusReg1Bit6 = 1,
    StatusReg2Bit1 = 2,
    StatusReg2Bit7 = 3,
    StatusReg2Bit1Via0x31 = 4,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorMiscMode {
    Disabled = 0,
    Mode0_4_4 = 1,
    Mode0_8_8 = 2,
    DataOrderSwapped = 3,
    SecondPinMux = 4,
    InternalLoopback = 5,
    SpiMode = 6,
    ExternalDqs = 8,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexspiSerialClockFrequency {
    NoChange = 0,
    MHz30 = 1,
    MHz50 = 2,
    MHz60 = 3,
    MHz75 = 4,
    MHz80 = 5,
    MHz100 = 6,
    MHz133 = 7,
    MHz166 = 8,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialNorFlashConnection {
    SinglePortA = 0,
    Parallel = 1,
    SinglePortB = 2,
    BothPorts = 3,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexspiClockSource {
    // Table 60 selector values for the ROM set_clock_source API.
    NoClock = 0,
    Pll0 = 1,
    FroHf = 3,
    Pll1 = 5,
    UsbPll = 6,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexspiClockConfigFrequency {
    // Table 61 values for the ROM config_clock API.
    MHz30 = 1,
    MHz50 = 2,
    MHz60 = 3,
    MHz75 = 4,
    MHz100 = 5,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexspiClockConfigMode {
    Sdr = 0,
    Ddr = 1,
}

#[inline(always)]
pub const fn pack_serial_nor_option0(
    option_size: SerialNorOptionSize,
    device_type: SerialNorDeviceType,
    query_pad: SerialNorOptionPadEncoding,
    cmd_pad: SerialNorOptionPadEncoding,
    quad_mode_setting: SerialNorQuadModeSetting,
    misc_mode: SerialNorMiscMode,
    max_freq: FlexspiSerialClockFrequency,
) -> u32 {
    ((SerialNorOptionTag::Config as u32) << 28)
        | ((option_size as u32) << 24)
        | ((device_type as u32) << 20)
        | ((query_pad as u32) << 16)
        | ((cmd_pad as u32) << 12)
        | ((quad_mode_setting as u32) << 8)
        | ((misc_mode as u32) << 4)
        | (max_freq as u32)
}

#[inline(always)]
pub const fn pack_serial_nor_option1(
    flash_connection: SerialNorFlashConnection,
    dqs_pinmux_group: u32,
    pinmux_group: u32,
    status_override: u32,
    dummy_cycles: u32,
) -> u32 {
    ((flash_connection as u32) << 28)
        | ((dqs_pinmux_group & 0xF) << 20)
        | ((pinmux_group & 0xF) << 16)
        | ((status_override & 0xFF) << 8)
        | (dummy_cycles & 0xFF)
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SerialNorConfigOption {
    // Packed ROM ABI input for `flexspiNorDriver->get_config(...)`.
    //
    // Typical MCXA ROM examples:
    // - Quad NOR, Quad SDR read @ 75 MHz:  option0 = 0xC000_0004, option1 = 0
    // - Quad NOR, Quad DDR read @ 60 MHz:  option0 = 0xC010_0003, option1 = 0
    //
    // Build these words with `pack_serial_nor_option0(...)` and
    // `pack_serial_nor_option1(...)`, keeping the transport struct itself raw.
    //
    // Example call pattern from the ROM docs:
    //   let mut option = SerialNorConfigOption { option0: 0xC000_0001, option1: 0 };
    //   flexspi_nor().get_config(instance, &mut cfg, &mut option);
    pub option0: u32,
    pub option1: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexspiOperationType {
    Command = 0,
    Config = 1,
    Write = 2,
    Read = 3,
}

impl FlexspiOperationType {
    pub const END: Self = Self::Read;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FlashRunContextFields {
    pub por_mode: u8,
    pub current_mode: u8,
    pub exit_no_cmd_sequence: u8,
    pub restore_sequence: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union FlashRunContext {
    pub B: FlashRunContextFields,
    pub U: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FlexspiLutSeq {
    pub seq_num: u8,
    pub seq_id: u8,
    pub reserved: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FlexspiDllTime {
    pub time_100ps: u8,
    pub delay_cells: u8,
}

#[repr(C)]
pub struct FlexspiMemConfig {
    pub tag: u32,
    pub version: u32,
    pub reserved0: u32,
    pub read_sample_clk_src: u8,
    pub cs_hold_time: u8,
    pub cs_setup_time: u8,
    pub column_address_width: u8,
    pub device_mode_cfg_enable: u8,
    pub device_mode_type: u8,
    pub wait_time_cfg_commands: u16,
    pub device_mode_seq: FlexspiLutSeq,
    pub device_mode_arg: u32,
    pub config_cmd_enable: u8,
    pub config_mode_type: [u8; 3],
    pub config_cmd_seqs: [FlexspiLutSeq; 3],
    pub reserved1: u32,
    pub config_cmd_args: [u32; 3],
    pub reserved2: u32,
    pub controller_misc_option: u32,
    pub device_type: u8,
    pub sflash_pad_type: u8,
    pub serial_clk_freq: u8,
    pub lut_custom_seq_enable: u8,
    pub reserved3: [u32; 2],
    pub sflash_a1_size: u32,
    pub sflash_a2_size: u32,
    pub sflash_b1_size: u32,
    pub sflash_b2_size: u32,
    pub cs_pad_setting_override: u32,
    pub sclk_pad_setting_override: u32,
    pub data_pad_setting_override: u32,
    pub dqs_pad_setting_override: u32,
    pub timeout_in_ms: u32,
    pub command_interval: u32,
    pub data_valid_time: [FlexspiDllTime; 2],
    pub busy_offset: u16,
    pub busy_bit_polarity: u16,
    pub lookup_table: [u32; 64],
    pub lut_custom_seq: [FlexspiLutSeq; 12],
    pub dll0_cr_val: u32,
    pub dll1_cr_val: u32,
    pub reserved4: [u32; 2],
}

#[repr(C)]
pub struct FlexspiNorConfig {
    pub mem_config: FlexspiMemConfig,
    pub page_size: u32,
    pub sector_size: u32,
    pub ipcmd_serial_clk_freq: u8,
    pub is_uniform_block_size: u8,
    pub is_data_order_swapped: u8,
    pub reserved0: [u8; 1],
    pub serial_nor_type: u8,
    pub need_exit_no_cmd_mode: u8,
    pub half_clk_for_non_read_cmd: u8,
    pub need_restore_no_cmd_mode: u8,
    pub block_size: u32,
    pub flash_state_ctx: FlashRunContext,
    pub reserved2: [u32; 10],
}

#[repr(C)]
pub struct FlexspiXfer {
    pub operation: FlexspiOperationType,
    pub base_address: u32,
    pub seq_id: u32,
    pub seq_num: u32,
    pub is_parallel_mode_enable: bool,
    pub tx_buffer: *const u32,
    pub tx_size: u32,
    pub rx_buffer: *mut u32,
    pub rx_size: u32,
}

#[repr(C)]
pub(super) struct FlexspiNorFlashDriverRaw {
    pub version: u32,
    pub init: unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig) -> Status,
    pub page_program:
        unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, dst: u32, src: *const u32) -> Status,
    pub erase_all: unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig) -> Status,
    pub erase: unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, start: u32, len: u32) -> Status,
    pub erase_sector: unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, addr: u32) -> Status,
    pub erase_block: unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, addr: u32) -> Status,
    pub get_config:
        unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, opt: *mut SerialNorConfigOption) -> Status,
    pub read: unsafe extern "C" fn(
        instance: u32,
        cfg: *mut FlexspiNorConfig,
        dst: *mut u32,
        start: u32,
        bytes: u32,
    ) -> Status,
    pub xfer: unsafe extern "C" fn(instance: u32, xfer: *mut FlexspiXfer) -> Status,
    pub update_lut: unsafe extern "C" fn(instance: u32, seq_index: u32, lut_base: *const u32, num_seq: u32) -> Status,
    pub set_clock_source: unsafe extern "C" fn(clock_src: u32) -> Status,
    pub config_clock: unsafe extern "C" fn(instance: u32, freq_option: u32, sample_clk_mode: u32),
    pub partial_program:
        unsafe extern "C" fn(instance: u32, cfg: *mut FlexspiNorConfig, dst: u32, src: *const u32, len: u32) -> Status,
}

#[derive(Clone, Copy)]
pub struct FlexspiNorFlashDriver {
    raw: &'static FlexspiNorFlashDriverRaw,
}

impl FlexspiNorFlashDriver {
    pub(super) const fn from_raw(raw: &'static FlexspiNorFlashDriverRaw) -> Self {
        Self { raw }
    }

    pub fn version(&self) -> u32 {
        self.raw.version
    }

    pub fn init(&self, instance: u32, cfg: *mut FlexspiNorConfig) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.init)(instance, cfg)) }
    }

    pub fn page_program(&self, instance: u32, cfg: *mut FlexspiNorConfig, dst: u32, src: *const u32) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.page_program)(instance, cfg, dst, src)) }
    }

    pub fn erase_all(&self, instance: u32, cfg: *mut FlexspiNorConfig) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.erase_all)(instance, cfg)) }
    }

    pub fn erase(&self, instance: u32, cfg: *mut FlexspiNorConfig, start: u32, len: u32) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.erase)(instance, cfg, start, len)) }
    }

    pub fn erase_sector(&self, instance: u32, cfg: *mut FlexspiNorConfig, addr: u32) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.erase_sector)(instance, cfg, addr)) }
    }

    pub fn erase_block(&self, instance: u32, cfg: *mut FlexspiNorConfig, addr: u32) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.erase_block)(instance, cfg, addr)) }
    }

    pub fn get_config(
        &self,
        instance: u32,
        cfg: *mut FlexspiNorConfig,
        opt: *mut SerialNorConfigOption,
    ) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.get_config)(instance, cfg, opt)) }
    }

    pub fn read(
        &self,
        instance: u32,
        cfg: *mut FlexspiNorConfig,
        dst: *mut u32,
        start: u32,
        bytes: u32,
    ) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.read)(instance, cfg, dst, start, bytes)) }
    }

    pub fn xfer(&self, instance: u32, xfer: *mut FlexspiXfer) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.xfer)(instance, xfer)) }
    }

    pub fn update_lut(&self, instance: u32, seq_index: u32, lut_base: *const u32, num_seq: u32) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.update_lut)(instance, seq_index, lut_base, num_seq)) }
    }

    pub fn set_clock_source(&self, clock_src: FlexspiClockSource) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.set_clock_source)(clock_src as u32)) }
    }

    pub fn config_clock(
        &self,
        instance: u32,
        freq_option: FlexspiClockConfigFrequency,
        sample_clk_mode: FlexspiClockConfigMode,
    ) {
        unsafe { (self.raw.config_clock)(instance, freq_option as u32, sample_clk_mode as u32) }
    }

    pub fn partial_program(
        &self,
        instance: u32,
        cfg: *mut FlexspiNorConfig,
        dst: u32,
        src: *const u32,
        len: u32,
    ) -> FlexspiStatus {
        unsafe { FlexspiStatus::from((self.raw.partial_program)(instance, cfg, dst, src, len)) }
    }
}
