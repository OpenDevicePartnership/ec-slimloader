#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;
#[cfg(feature = "mimxrt633s")]
use mimxrt600_fcb::FlexSpiLutOpcode::RADDR_SDR;
#[cfg(feature = "imxrt")]
use mimxrt600_fcb::FlexSpiLutOpcode::{
    CMD_DDR, CMD_SDR, DUMMY_DDR, RADDR_DDR, READ_DDR, READ_SDR, STOP, WRITE_DDR, WRITE_SDR,
};
#[cfg(feature = "mimxrt633s")]
use mimxrt600_fcb::FlexSpiNumPads::Quad;
#[cfg(feature = "imxrt")]
use mimxrt600_fcb::FlexSpiNumPads::{Octal, Single};
#[cfg(feature = "imxrt")]
use mimxrt600_fcb::{flexspi_lut_seq, FlexSPIFlashConfigurationBlock};
#[cfg(feature = "mimxrt633s")]
use mimxrt600_fcb::{ControllerMiscOption, SFlashPadType, SerialClkFreq, SerialNORType};
use panic_probe as _;

// auto-generated version information from Cargo.toml
#[cfg(feature = "imxrt")]
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[cfg(feature = "imxrt")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

#[cfg(feature = "mimxrt685s")]
#[link_section = ".fcb"]
#[used]
static FCB: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build().lookup_table([
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

#[cfg(feature = "mimxrt633s")]
#[link_section = ".fcb"]
#[used]
static FCB: FlexSPIFlashConfigurationBlock = FlexSPIFlashConfigurationBlock::build()
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

#[cfg(feature = "imxrt")]
mod imxrt_interrupts;

fn validate_crc(app_descriptor: &AppImageDescriptor) -> bool {
    // TODO - integrate imxrt CRC hardware engine for computation
    // #[cfg(feature="imxrt")]
    // {
    //
    // }
    // #[cfg(not(feature="imxrt"))]
    {
        use crc::*;

        let crc_engine = Crc::<u32>::new(&CRC_32_CKSUM);
        let mut digest = crc_engine.digest();
        const VPAGE_SIZE: usize = 1024; // a chunk size "virtual page" that aligns with most page boundary multiples
        let num_pages = (app_descriptor.image_size_bytes as usize).div_ceil(VPAGE_SIZE);

        // ...
        for page in 0..num_pages {
            // can't pass memory directly into digest API as requires WIDE pointer type (meta data for slice)
            let mut page_slice = [0u8; VPAGE_SIZE];

            let remaining = (app_descriptor.image_size_bytes as usize).saturating_sub(page * VPAGE_SIZE);

            for (i, local_copy) in page_slice[0..remaining].iter_mut().enumerate() {
                *local_copy = unsafe { *(app_descriptor.stored_address as *const u8).add(i + page * VPAGE_SIZE) };
            }

            digest.update(&page_slice);
        }

        let image_crc = unsafe { *(app_descriptor.stored_crc_address as *const u32) };

        image_crc == digest.finalize()
    }
}

#[cfg(feature = "imxrt")]
unsafe fn raw_copy_to_ram(from: *const u32, to: *mut u32, len_words: usize) {
    core::ptr::copy_nonoverlapping(from, to, len_words);
}

fn copy_image(_app_descriptor: &AppImageDescriptor) {
    #[cfg(feature = "imxrt")]
    {
        // TODO allow other scenarios supported from bootloader aside from copy to RAM
        // TODO smarter := operation
        unsafe {
            raw_copy_to_ram(
                _app_descriptor.stored_address as *const u32,
                _app_descriptor.execution_address as *mut u32,
                _app_descriptor.execution_copy_size_bytes as usize,
            );
        }
    }
}

#[cfg(feature = "defmt")]
fn dump_descriptor_header(descriptor_header_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<BootableRegionDescriptorHeader>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *descriptor_header_address.add(i) };
    }

    defmt::info!("Descriptor Header Bytes: {:x}", memory_copy);
}

#[cfg(feature = "defmt")]
fn dump_app_descriptor(app_descriptor_address: *const u32) {
    let mut memory_copy = [0u32; core::mem::size_of::<AppImageDescriptor>() / core::mem::size_of::<u32>()];
    for (i, b) in memory_copy.iter_mut().enumerate() {
        *b = unsafe { *app_descriptor_address.add(i) };
    }

    defmt::info!("App Descriptor Bytes: {:x}", memory_copy);
}

use ec_slimloader_descriptors::*;

extern "C" {
    static __bootable_region_descriptors_address: u32;
}

#[cortex_m_rt::pre_init]
unsafe fn pre_init() {
    cortex_m::interrupt::disable();
}

#[cortex_m_rt::entry]
fn main() -> ! {
    #[cfg(feature = "defmt")]
    defmt::info!("Bootloader: Initializing Hardware.");

    let boot_descriptors_address = unsafe { &__bootable_region_descriptors_address as *const u32 };

    #[cfg(feature = "defmt")]
    defmt::info!(
        "Bootloader: Fetching App Descriptors from {:X}.",
        boot_descriptors_address
    );

    // TODO error handling
    //      ? should the bootloader be responsible for re-formatting? This may be a security decision
    let descriptors = match BootableRegionDescriptors::from_address(boot_descriptors_address) {
        Ok(descriptors) => descriptors,
        Err(_e) => {
            #[cfg(feature = "defmt")]
            defmt::error!(
                "Invalid boot region descriptors: ParseError |{}|",
                match _e {
                    ParseError::InvalidSignature => "Invalid Header Signature",
                    ParseError::InvalidAppSlot => "Invalid App Slot",
                    ParseError::InvalidSlotCount => "Invalid Slot Count",
                    _ => "CRC Error",
                }
            );

            #[cfg(feature = "defmt")]
            match _e {
                ParseError::InvalidHeaderCrc { found, expected } => {
                    defmt::error!("Header CRC found = {:x}, expected = {:x}", found, expected);
                    dump_descriptor_header(boot_descriptors_address);
                }
                ParseError::InvalidAppCrc {
                    address,
                    found,
                    expected,
                } => {
                    defmt::error!("App @{:x} CRC found = {:x}, expected = {:x}", address, found, expected);
                    dump_app_descriptor(address);
                }
                _ => (),
            }

            loop {
                cortex_m::asm::wfi();
            }
        }
    };

    let active_app_descriptor = descriptors.get_active_slot();

    #[cfg(feature = "defmt")]
    defmt::info!(
        "Bootloader: Active App slot is {}",
        active_app_descriptor.app_slot_number
    );

    let boot_image = if active_app_descriptor.flags & APP_IMAGE_FLAG_SKIP_IMAGE_CRC_CHECK != 0 {
        #[cfg(feature = "defmt")]
        defmt::info!("Bootloader: skipping image CRC due to SKIP_IMAGE_CRC_CHECK flag.");

        true
    } else {
        #[cfg(feature = "defmt")]
        defmt::info!("Bootloader: validating image CRC.");

        validate_crc(&active_app_descriptor)
    };

    // validate integrity of app image
    if boot_image {
        // note that use of this flag requires proper app image address backing for .text section-- ensure you have linked the app against RAM addresses! (Or other required adjustments)
        if active_app_descriptor.flags & APP_IMAGE_FLAG_COPY_TO_EXECUTION_ADDRESS != 0 {
            #[cfg(feature = "defmt")]
            defmt::info!("Bootloader: Performing image copy to execution location.");

            copy_image(&active_app_descriptor);
        }

        #[cfg(feature = "defmt")]
        defmt::info!(
            "Bootloader: Validation complete and branching to application execution address {:x}.",
            active_app_descriptor.execution_address
        );

        // branch to location as described by descriptor
        unsafe {
            // set the VTOR
            cortex_m::Peripherals::steal()
                .SCB
                .vtor
                .write(active_app_descriptor.execution_address);

            // enable interrupts
            cortex_m::interrupt::enable();

            // perform the branch
            let sp = *(active_app_descriptor.execution_address as *const u32) as *const u32;
            let rv = *(active_app_descriptor.execution_address as *const u32).add(1) as *const u32;

            cortex_m::asm::bootstrap(sp, rv);
        }
    } else {
        // TODO any boot recovery logic
        #[cfg(feature = "defmt")]
        defmt::error!("Active App CRC checksum failure!");

        loop {
            cortex_m::asm::wfi();
        }
    }
}
