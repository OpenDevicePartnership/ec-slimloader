#![allow(clippy::not_unsafe_ptr_arg_deref)]

use embedded_storage_async::nor_flash::{ErrorType, NorFlash, NorFlashErrorKind, ReadNorFlash};

use crate::error::FlashStatus;
use crate::memory::{INTERNAL_FLASH_PAGE_SIZE, INTERNAL_FLASH_SECTOR_SIZE, JOURNAL_SIZE, JOURNAL_START};
use crate::rom_api::{
    flash_driver, FlashConfig, FlashFfrConfig, FlashModeConfig, FlashRampControlOption, FlashReadDmaccOption,
    FlashReadEccOption, FlashReadMarginOption, FlashReadSingleWordConfig, FlashSetReadModeConfig,
    FlashSetWriteModeConfig, FLASH_API_ERASE_KEY,
};

pub struct InternalFlash {
    pub cfg: FlashConfig,
    initialized: bool,
}

impl InternalFlash {
    pub const fn new() -> Self {
        Self {
            cfg: FlashConfig {
                pflash_block_base: 0,
                pflash_total_size: 0,
                pflash_block_count: 0,
                pflash_page_size: 0,
                pflash_sector_size: 0,
                ffr_config: FlashFfrConfig {
                    ffr_block_base: 0,
                    ffr_total_size: 0,
                    ffr_page_size: 0,
                    sector_size: 0,
                    cfpa_page_version: 0,
                    cfpa_page_offset: 0,
                },
                mode_config: FlashModeConfig::new(
                    0,
                    FlashReadSingleWordConfig::new(
                        FlashReadEccOption::On,
                        FlashReadMarginOption::Normal,
                        FlashReadDmaccOption::Disabled,
                    ),
                    FlashSetWriteModeConfig::new(FlashRampControlOption::Reserved, FlashRampControlOption::Reserved),
                    FlashSetReadModeConfig::new(0, 0, 0),
                ),
                nboot_ctx: core::ptr::null_mut(),
                use_ahb_read: true,
            },
            initialized: false,
        }
    }

    fn ensure_init(&mut self) -> Result<(), NorFlashErrorKind> {
        if self.initialized {
            return Ok(());
        }
        let flash_driver_api = flash_driver();
        let status = flash_driver_api.flash_init(&mut self.cfg);
        if status != FlashStatus::Success {
            return Err(NorFlashErrorKind::Other);
        }
        self.initialized = true;
        Ok(())
    }
}

impl ErrorType for InternalFlash {
    type Error = NorFlashErrorKind;
}

impl ReadNorFlash for InternalFlash {
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.ensure_init()?;
        let read_len = u32::try_from(buf.len()).map_err(|_| NorFlashErrorKind::OutOfBounds)?;
        let end = offset.checked_add(read_len).ok_or(NorFlashErrorKind::OutOfBounds)?;
        if end > JOURNAL_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        let flash_driver_api = flash_driver();
        let abs = JOURNAL_START
            .checked_add(offset)
            .ok_or(NorFlashErrorKind::OutOfBounds)?;
        let status = flash_driver_api.flash_read(&mut self.cfg, abs, buf.as_mut_ptr(), read_len);
        if status == FlashStatus::Success {
            Ok(())
        } else {
            Err(NorFlashErrorKind::Other)
        }
    }

    fn capacity(&self) -> usize {
        JOURNAL_SIZE as usize
    }
}

impl NorFlash for InternalFlash {
    const WRITE_SIZE: usize = INTERNAL_FLASH_PAGE_SIZE as usize; // use page alignment
    const ERASE_SIZE: usize = INTERNAL_FLASH_SECTOR_SIZE as usize;

    async fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), Self::Error> {
        self.ensure_init()?;
        let write_len = u32::try_from(data.len()).map_err(|_| NorFlashErrorKind::OutOfBounds)?;
        let end = offset.checked_add(write_len).ok_or(NorFlashErrorKind::OutOfBounds)?;
        if end > JOURNAL_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        if !offset.is_multiple_of(INTERNAL_FLASH_PAGE_SIZE) {
            return Err(NorFlashErrorKind::NotAligned);
        }
        if data.len() > INTERNAL_FLASH_PAGE_SIZE as usize {
            return Err(NorFlashErrorKind::OutOfBounds);
        }
        let flash_driver_api = flash_driver();
        let abs_start = JOURNAL_START
            .checked_add(offset)
            .ok_or(NorFlashErrorKind::OutOfBounds)?;

        // Page was erased prior to this write — fill rest with 0xFF and program.
        let mut page_buf = [0xFFu8; INTERNAL_FLASH_PAGE_SIZE as usize];
        page_buf[..data.len()].copy_from_slice(data); //safe as we checked bounds above.

        let status =
            flash_driver_api.flash_program_page(&mut self.cfg, abs_start, page_buf.as_ptr(), INTERNAL_FLASH_PAGE_SIZE);
        if status != FlashStatus::Success {
            return Err(NorFlashErrorKind::Other);
        }

        // Verify programmed data via ROM API.
        let mut failed_address = 0u32;
        let mut failed_data = 0u32;
        let status = flash_driver_api.flash_verify_program(
            &mut self.cfg,
            abs_start,
            INTERNAL_FLASH_PAGE_SIZE,
            page_buf.as_ptr(),
            &mut failed_address,
            &mut failed_data,
        );
        if status != FlashStatus::Success {
            return Err(NorFlashErrorKind::Other);
        }
        Ok(())
    }

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.ensure_init()?;
        if from > to || to > JOURNAL_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }

        // Round to sector boundaries rather than rejecting unaligned inputs.
        let from_aligned = (from / INTERNAL_FLASH_SECTOR_SIZE) * INTERNAL_FLASH_SECTOR_SIZE;
        let to_rounded = to
            .checked_add(INTERNAL_FLASH_SECTOR_SIZE - 1)
            .ok_or(NorFlashErrorKind::OutOfBounds)?;
        let to_aligned = (to_rounded / INTERNAL_FLASH_SECTOR_SIZE) * INTERNAL_FLASH_SECTOR_SIZE;
        if to_aligned > JOURNAL_SIZE {
            return Err(NorFlashErrorKind::OutOfBounds);
        }

        let len = to_aligned
            .checked_sub(from_aligned)
            .ok_or(NorFlashErrorKind::OutOfBounds)?;
        if len == 0 {
            return Ok(());
        }

        let flash_driver_api = flash_driver();
        let abs = JOURNAL_START
            .checked_add(from_aligned)
            .ok_or(NorFlashErrorKind::OutOfBounds)?;
        let status = flash_driver_api.flash_erase_sector(&mut self.cfg, abs, len, FLASH_API_ERASE_KEY);
        if status == FlashStatus::Success {
            Ok(())
        } else {
            Err(NorFlashErrorKind::Other)
        }
    }
}
