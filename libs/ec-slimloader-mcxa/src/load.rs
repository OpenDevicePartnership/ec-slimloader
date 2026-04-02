use crate::flash_internal::InternalFlash;
use crate::memory::{INTERNAL_FLASH_PAGE_SIZE, SLOT_A_SIZE, SLOT_A_START, SLOT_B_START};
use crate::rom_api::{flash_driver, FLASH_API_ERASE_KEY};

// Currently this file contains legacy loader stubs that are not wired into the MCXA crate. It is retained for future use as we implement slot-copying functionality, 
// but the implementations below are not up to date with the current ROM flash API bindings and are not currently used in the MCXA bootloader path.

pub enum LoadError {
    Init,
    Read,
    Erase,
    Program,
}

// Legacy loader stub. This file is not currently wired into the MCXA crate,
// and the implementation below predates the current ROM flash API bindings.
pub fn load_slot_b_to_a_internal_only(internal: &mut InternalFlash) -> Result<(), LoadError> {
    // This routine assumes Slot B is readable via direct memory access and copies it into
    // internal flash a page at a time. It has not been updated to the current FlashDriver API.
    
    // Erase internal slot A completely (assumes alignment of SLOT_A_START & size)
    unsafe {
        let drv = flash_driver();
        let cfg = &mut internal.cfg as *mut _; // Access internal config
        let status = (drv.flash_erase)(cfg, SLOT_A_START, SLOT_A_SIZE, FLASH_API_ERASE_KEY);
        if status != 0 {
            return Err(LoadError::Erase);
        }
    }

    // Copy page by page
    let mut offset: u32 = 0;
    let mut page_buf = [0u8; INTERNAL_FLASH_PAGE_SIZE as usize];
    while offset < SLOT_A_SIZE {
        // Read from external (XIP memory copy)
        let src = (SLOT_B_START + offset) as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, page_buf.as_mut_ptr(), page_buf.len());
        }
        // Program into internal
        unsafe {
            let drv = flash_driver();
            let cfg = &mut internal.cfg as *mut _;
            let status = (drv.flash_program)(
                cfg,
                SLOT_A_START + offset,
                page_buf.as_mut_ptr(),
                INTERNAL_FLASH_PAGE_SIZE,
            );
            if status != 0 {
                return Err(LoadError::Program);
            }
        }
        offset += INTERNAL_FLASH_PAGE_SIZE;
    }
    Ok(())
}

// Legacy stub retained for future slot-copy work. It is currently unused.
pub fn load_slot_b_to_a_len(
    _internal: &mut InternalFlash,
    _length: u32,
) -> Result<(), LoadError> {
    // A real implementation would need to match the current MCXA external-flash path.
    Err(LoadError::Init)
}
