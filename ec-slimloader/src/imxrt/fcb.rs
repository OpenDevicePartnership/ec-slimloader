#![allow(unused)]

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
static FCB_685EVK: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build();

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
