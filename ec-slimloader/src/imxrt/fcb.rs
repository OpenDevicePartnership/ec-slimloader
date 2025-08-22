use mimxrt600_fcb::FlexSpiLutOpcode::{CMD_SDR, READ_SDR, STOP};
use mimxrt600_fcb::FlexSpiNumPads::Single;
use mimxrt600_fcb::{flexspi_lut_seq, FlexSPIFlashConfigurationBlock};

#[cfg(all(feature = "imxrt-fcb-1spi-nor", feature = "imxrt-fcb-rt685evk"))]
compile_error!("Cannot allocate more than one FCB at a time!");

#[cfg(all(feature = "imxrt-fcb-1spi-a1-nor", feature = "imxrt-fcb-1spi-b1-nor"))]
compile_error!("Cannot configure FCB for more than one bank at a time!");

#[cfg(feature = "imxrt-fcb-1spi-nor")]
use mimxrt600_fcb::FlexSpiLutOpcode::RADDR_SDR;
#[cfg(feature = "imxrt-fcb-rt685evk")]
use mimxrt600_fcb::FlexSpiLutOpcode::{CMD_DDR, DUMMY_DDR, RADDR_DDR, READ_DDR, WRITE_DDR, WRITE_SDR};
#[cfg(feature = "imxrt-fcb-rt685evk")]
use mimxrt600_fcb::FlexSpiNumPads::Octal;
#[cfg(feature = "imxrt-fcb-1spi-nor")]
use mimxrt600_fcb::{ControllerMiscOption, SFlashPadType, SerialClkFreq, SerialNORType};

#[cfg(feature = "imxrt-fcb-rt685evk")]
#[link_section = ".fcb"]
#[used]
static FCB_685EVK: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build().lookup_table([
    // Read
    flexspi_lut_seq(CMD_DDR, Octal, 0xee, CMD_DDR, Octal, 0x11),
    flexspi_lut_seq(RADDR_DDR, Octal, 0x20, DUMMY_DDR, Octal, 0x29),
    flexspi_lut_seq(READ_DDR, Octal, 0x04, STOP, Single, 0x00),
    0,
    // Read status SPI
    flexspi_lut_seq(CMD_SDR, Single, 0x05, READ_SDR, Single, 0x04),
    0,
    0,
    0,
    // Read status OPI
    flexspi_lut_seq(CMD_DDR, Octal, 0x05, CMD_DDR, Octal, 0xFA),
    flexspi_lut_seq(RADDR_DDR, Octal, 0x20, DUMMY_DDR, Octal, 0x14),
    flexspi_lut_seq(READ_DDR, Octal, 0x04, STOP, Single, 0x00),
    0,
    // Write enable
    flexspi_lut_seq(CMD_SDR, Single, 0x06, STOP, Single, 0x00),
    0,
    0,
    0,
    // Write enable - OPI
    flexspi_lut_seq(CMD_DDR, Octal, 0x06, CMD_DDR, Octal, 0xF9),
    0,
    0,
    0,
    // Erase Sector
    flexspi_lut_seq(CMD_DDR, Octal, 0x21, CMD_DDR, Octal, 0xDE),
    flexspi_lut_seq(RADDR_DDR, Octal, 0x20, STOP, Single, 0x00),
    0,
    0,
    // Enable OPI DDR mode
    flexspi_lut_seq(CMD_SDR, Single, 0x72, CMD_SDR, Single, 0x00),
    flexspi_lut_seq(CMD_SDR, Single, 0x00, CMD_SDR, Single, 0x00),
    flexspi_lut_seq(CMD_SDR, Single, 0x00, WRITE_SDR, Single, 0x01),
    0,
    // Unused
    0,
    0,
    0,
    0,
    // Erase block
    flexspi_lut_seq(CMD_DDR, Octal, 0xDC, CMD_DDR, Octal, 0x23),
    flexspi_lut_seq(RADDR_DDR, Octal, 0x20, STOP, Single, 0x00),
    0,
    0,
    // Page program
    flexspi_lut_seq(CMD_DDR, Octal, 0x12, CMD_DDR, Octal, 0xED),
    flexspi_lut_seq(RADDR_DDR, Octal, 0x20, WRITE_DDR, Octal, 0x04),
    0,
    0,
    // Unused
    0,
    0,
    0,
    0,
    // Erase chip
    flexspi_lut_seq(CMD_DDR, Octal, 0x60, CMD_DDR, Octal, 0x9F),
    0,
    0,
    0,
    // Remainder is unused
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
]);

#[cfg(feature = "imxrt-fcb-1spi-a1-nor")]
#[link_section = ".fcb"]
#[used]
static FCB_A1NOR: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build()
    .device_mode_cfg_enable(0)
    .wait_time_cfg_commands(0)
    .device_mode_arg([0; 4])
    .config_mode_type([0, 1, 2])
    .controller_misc_option(ControllerMiscOption(0x10))
    .sflash_pad_type(SFlashPadType::QuadPads)
    .serial_clk_freq(SerialClkFreq::SdrDdr50mhz)
    .sflash_a1_size(0x0040_0000)
    .sflash_b1_size(0)
    .lookup_table([
        // Sequence 0 - Read Data (in the default Single SPI lane mode coming out of reset)
        // 0x03 - Read Data command, 0x18 - W25Q16FW address size (24 bits)
        flexspi_lut_seq(CMD_SDR, Single, 0x03, RADDR_SDR, Single, 0x18),
        // Sequence 1 - Read 128 Data Bytes and Stop
        // 0x80 - read 128 bytes, stop
        flexspi_lut_seq(READ_SDR, Single, 0x80, STOP, Single, 0x00),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ])
    .serial_nor_type(SerialNORType::StandardSpi)
    .flash_state_ctx(0);

#[cfg(feature = "imxrt-fcb-1spi-b1-nor")]
#[link_section = ".fcb"]
#[used]
static FCB_B1NOR: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build()
    .device_mode_cfg_enable(0)
    .wait_time_cfg_commands(0)
    .device_mode_arg([0; 4])
    .config_mode_type([0, 1, 2])
    .controller_misc_option(ControllerMiscOption(0x10))
    .sflash_pad_type(SFlashPadType::QuadPads)
    .serial_clk_freq(SerialClkFreq::SdrDdr50mhz)
    .sflash_a1_size(0)
    .sflash_b1_size(0x0040_0000)
    .lookup_table([
        // Sequence 0 - Read Data (in the default Single SPI lane mode coming out of reset)
        // 0x03 - Read Data command, 0x18 - W25Q16FW address size (24 bits)
        flexspi_lut_seq(CMD_SDR, Single, 0x03, RADDR_SDR, Single, 0x18),
        // Sequence 1 - Read 128 Data Bytes and Stop
        // 0x80 - read 128 bytes, stop
        flexspi_lut_seq(READ_SDR, Single, 0x80, STOP, Single, 0x00),
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ])
    .serial_nor_type(SerialNORType::StandardSpi)
    .flash_state_ctx(0);
