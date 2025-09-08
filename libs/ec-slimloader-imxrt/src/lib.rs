#![no_std]

#[cfg(feature = "fcb")]
mod fcb;

mod rkh;
mod rom;
mod shadow;

#[cfg(feature = "empty-otfad")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

mod bootload;
mod mbi;

use core::ops::Range;

use defmt_or_log::{info, warn};
use ec_slimloader_state::flash::FlashJournal;
use ec_slimloader_state::state::Slot;
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_imxrt::{
    clocks::MainClkSrc,
    flexspi::{embedded_storage::FlexSpiNorStorage, nor_flash::FlexSpiNorFlash},
    peripherals::HASHCRYPT,
    Peri,
};

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use heapless::Vec;
use partition_manager::{Partition, PartitionManager, RO, RW};

use static_cell::StaticCell;

use ec_slimloader::{Board, BootError};

use crate::mbi::CertificateBlockHeader;

const SYSTEM_CORE_CLOCK_HZ: u32 = (475 * 1000 * 1000) / 2;

const IMAGE_TYPE_TZ_XIP_SIGNED: u32 = 0x0004;
const READ_ALIGNMENT: u32 = 2;
const WRITE_ALIGNMENT: u32 = 2;
const ERASE_SIZE: u32 = 4096;
const MAX_SLOT_COUNT: usize = 7;

pub type ExternalStorage = BlockingAsync<FlexSpiNorStorage<'static, READ_ALIGNMENT, WRITE_ALIGNMENT, ERASE_SIZE>>;

pub struct Partitions {
    pub state: Partition<'static, ExternalStorage, RW, NoopRawMutex>,
    pub slots: Vec<Partition<'static, ExternalStorage, RO, NoopRawMutex>, MAX_SLOT_COUNT>,
}

pub trait ImxrtConfig {
    /// Minimum and maximum image size contained within a slot.
    const SLOT_SIZE_RANGE: Range<usize>;

    /// The memory range an image is allowed to be copied to.
    const LOAD_RANGE: Range<*mut u32>;

    fn partitions(&self, flash: &'static mut PartitionManager<ExternalStorage, NoopRawMutex>) -> Partitions;
}

pub struct Imxrt<C> {
    journal: FlashJournal<Partition<'static, ExternalStorage, RW>>,
    slots: Vec<Partition<'static, ExternalStorage, RO, NoopRawMutex>, MAX_SLOT_COUNT>,
    hashcrypt: Peri<'static, HASHCRYPT>,
    _config: C,
}

impl<C: ImxrtConfig> Board for Imxrt<C> {
    type Config = C;

    async fn init<const JOURNAL_BUFFER_SIZE: usize>(config: Self::Config) -> Self {
        // Set clock to Pll but with a larger divider, otherwise
        // we get nondeterministic behaviour from the ROM API.
        let mut hal_config = embassy_imxrt::config::Config::default();
        hal_config.clocks.main_clk.src = MainClkSrc::PllMain;
        hal_config.clocks.main_clk.div_int = 2.into();
        hal_config.clocks.main_pll_clk.pfd0 = 20;
        let p = embassy_imxrt::init(hal_config);

        let ext_flash = match unsafe { FlexSpiNorFlash::with_probed_config(p.FLEXSPI, READ_ALIGNMENT, WRITE_ALIGNMENT) }
        {
            Ok(ext_flash) => ext_flash,
            Err(e) => panic!("Failed to initialize FlexSPI peripheral: {:?}", e),
        };

        let ext_flash =
            match unsafe { FlexSpiNorStorage::<READ_ALIGNMENT, WRITE_ALIGNMENT, ERASE_SIZE>::new(ext_flash) } {
                Ok(ext_flash) => ext_flash,
                Err(e) => panic!("Failed to wrap FlexSPI flash in embedded_storage adaptor: {:?}", e),
            };

        static EXT_FLASH: StaticCell<PartitionManager<ExternalStorage, NoopRawMutex>> = StaticCell::new();
        let ext_flash_manager =
            EXT_FLASH.init_with(|| PartitionManager::<_, NoopRawMutex>::new(BlockingAsync::new(ext_flash)));

        let Partitions { state, slots } = config.partitions(ext_flash_manager);

        let journal = match FlashJournal::new::<JOURNAL_BUFFER_SIZE>(state).await {
            Ok(journal) => journal,
            Err(e) => panic!("Failed to initialize the flash state journal: {:?}", e),
        };

        Self {
            journal,
            slots,
            hashcrypt: p.HASHCRYPT,
            _config: config,
        }
    }

    fn journal(&mut self) -> &mut FlashJournal<impl NorFlash> {
        &mut self.journal
    }

    async fn check_and_boot(&mut self, slot: &Slot) -> BootError {
        let slot_partition = match self.slots.get_mut(u8::from(*slot) as usize) {
            Some(slot) => slot,
            None => return BootError::SlotUnknown,
        };

        // Copy the image to RAM from flash, and ensure that everything from flash is no longer available.
        let ram_ivt = {
            let slot_size = slot_partition.capacity();

            // Check if the image_len fits within the slot.
            if slot_size >= C::SLOT_SIZE_RANGE.end {
                return BootError::TooLarge;
            }

            // Verify IVT fields.
            let ivt = match mbi::Ivt::read(slot_partition).await {
                Ok(ivt) => ivt,
                Err(_) => return BootError::IO,
            };

            // Note: skboot_authenticate only supports checking XIP_SIGNED, even though we are loading it to RAM here.
            if ivt.image_type != IMAGE_TYPE_TZ_XIP_SIGNED {
                return BootError::Markers;
            }
            if ivt.image_len > slot_size {
                return BootError::TooLarge;
            }
            if ivt.image_len < C::SLOT_SIZE_RANGE.start {
                return BootError::TooSmall;
            }

            // Check if the target_ptr is within the allowed range.
            // In MBI this is called the 'load_addr', which is located in 0x34 of IVT.
            let image_target_end_ptr = match ivt.target_end_ptr() {
                Some(ptr) => ptr,
                None => return BootError::TooLarge,
            };

            if !C::LOAD_RANGE.contains(&ivt.target_ptr) || !C::LOAD_RANGE.contains(&image_target_end_ptr) {
                return BootError::MemoryRegion;
            }

            info!("Starting copy");
            let target_slice = unsafe { core::slice::from_raw_parts_mut(ivt.target_ptr as *mut u8, ivt.image_len) };
            if let Err(_e) = slot_partition.read(0, target_slice).await {
                return BootError::IO;
            }

            // Invalidate icache as we are writing to Code RAM, which is cached.
            unsafe {
                let mut p = cortex_m::Peripherals::steal();
                p.SCB.invalidate_icache();
            }
            info!("Copy done");

            let ram_ivt = match mbi::Ivt::read_from_slice(target_slice) {
                Ok(ram_ivt) => ram_ivt,
                Err(mbi::BufferTooSmall) => return BootError::TooSmall,
            };

            if ivt != ram_ivt {
                return BootError::ChangeAfterRead;
            }

            ram_ivt
        };

        // Compute RKTH from image.
        let image_rkth = {
            // Safety: whilst we do not know if the image is valid by itself,
            // this slice at least is what we just copied. (should be identical to target_slice)
            let ram_image_slice =
                unsafe { core::slice::from_raw_parts(ram_ivt.target_ptr as *const u8, ram_ivt.image_len) };
            let cert_block_header_offset = ram_ivt.header_offset as usize;

            // Fetch certificate block
            let cert_block_header = if let Some(cert_block_header) =
                CertificateBlockHeader::read_from_slice(&ram_image_slice[cert_block_header_offset..])
            {
                cert_block_header
            } else {
                return BootError::TooLarge;
            };

            if cert_block_header.header_length != 0x20 {
                defmt_or_log::warn!("Certificate block header is not expected length");
            }

            let rkhs_offset = cert_block_header_offset
                + cert_block_header.header_length as usize
                + cert_block_header.certificate_table_length as usize;

            let rkhs = if let Some(rkhs) = rkh::Rkh::read_all_from_slice(&ram_image_slice[rkhs_offset..]) {
                rkhs
            } else {
                return BootError::TooLarge;
            };

            rkh::Rkth::from_rkhs(&rkhs, self.hashcrypt.reborrow())
        };

        // Reload shadow registers.
        defmt_or_log::unwrap!(rom::otp_reload());

        // Whether the hardware is in 'development mode' is dependent on the secure_boot_en bit being asserted.
        let dev_mode = !shadow::Boot0::read_shadow().secure_boot();

        if image_rkth != rkh::Rkth::read_shadow() {
            if dev_mode {
                // If no SECURE_BOOT fuse set => overwrite shadow RKTH with image RKTH
                defmt_or_log::warn!("Development mode detected, using new image RKTH");
                image_rkth.write_shadow();
            } else {
                // If SECURE_BOOT fuse set => do nothing as skboot_authenticate should be annoyed (perhaps assert afterwards)
                defmt_or_log::error!(
                    "Shadow and image RKTH do not concur, but we call skboot_authenticate in any case"
                );
            }
        } else {
            defmt_or_log::info!("Shadow and image RKTH concur!")
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "non-secure")] {
                warn!("Authentication skipped");
            } else {
                info!("Starting authenticate");
                // Call the ROM API to ensure that the image is signed and not broken or tampered with.
                // Note: skboot_authenticate will show false-negatives if your clock jitter is too high.
                // We noticed this with FFROdiv2 and MainClk > 475MHz.
                match rom::skboot_authenticate(ram_ivt.target_ptr, ram_ivt.image_len as u32, None) {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Failed to authenticate {:?}", e);
                        return BootError::Authenticate;
                    }
                }
            }
        }

        info!("Booting into application @ {:x}...", ram_ivt.target_ptr);

        // Boot to application, and we do not return from this function.
        unsafe { bootload::boot_application(ram_ivt.target_ptr) }
    }

    fn abort(&mut self) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }
}
