mod fcb;

// auto-generated version information from Cargo.toml
#[cfg(feature = "imxrt")]
include!(concat!(env!("OUT_DIR"), "/biv.rs"));

#[cfg(feature = "imxrt")]
#[link_section = ".otfad"]
#[used]
static OTFAD: [u8; 256] = [0x00; 256];

use ec_slimloader_descriptors::*;

pub fn validate_crc(app_descriptor: &AppImageDescriptor) -> bool {
    // TODO - integrate imxrt CRC hardware engine for computation
    use crc::*;

    let crc_engine = Crc::<u32>::new(&CRC_32_ISO_HDLC);
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

pub unsafe fn raw_copy_to_ram(from: *const u32, to: *mut u32, len_words: usize) {
    core::ptr::copy_nonoverlapping(from, to, len_words);
}
