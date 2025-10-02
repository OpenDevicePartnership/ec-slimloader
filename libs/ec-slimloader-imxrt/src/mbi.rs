#![allow(dead_code)]

use embedded_storage_async::nor_flash::ReadNorFlash;

#[derive(Debug, PartialEq)]
pub struct Ivt {
    pub image_len: usize,
    pub image_type: u32,
    pub header_offset: u32,
    pub target_ptr: *mut u32,
}

pub struct BufferTooSmall;

impl Ivt {
    pub async fn read<F: ReadNorFlash>(slot: &mut F) -> Result<Self, F::Error> {
        let mut buf = [0u8; 64];
        slot.read(0, &mut buf).await?;

        // Note(unsafe): our buffer is 64 bytes large.
        Ok(unsafe { Self::read_from_slice(&buf).unwrap_unchecked() })
    }

    pub fn read_from_slice(data: &[u8]) -> Result<Self, BufferTooSmall> {
        if data.len() < 64 {
            return Err(BufferTooSmall);
        }

        // Note(unsafe): we are taking byte slices 4 bytes long, so they should map perfectly to 4 byte arrays.
        Ok(Self {
            image_len: u32::from_le_bytes(unsafe { data[0x20..0x24].try_into().unwrap_unchecked() }) as usize,
            image_type: u32::from_le_bytes(unsafe { data[0x24..0x28].try_into().unwrap_unchecked() }),
            header_offset: u32::from_le_bytes(unsafe { data[0x28..0x2C].try_into().unwrap_unchecked() }),
            target_ptr: u32::from_le_bytes(unsafe { data[0x34..0x38].try_into().unwrap_unchecked() }) as *mut u32,
        })
    }

    pub fn target_end_ptr(&self) -> Option<*mut u32> {
        (self.target_ptr as usize)
            .checked_add(self.image_len)
            .map(|ptr| ptr as *mut u32)
    }
}

#[repr(C)]
pub struct CertificateBlockHeader {
    pub signature: u32,
    pub header_major_version: u16,
    pub header_minor_version: u16,
    pub header_length: u32,
    pub flags: u32,
    pub build_number: u32,
    pub total_image_length: u32,
    pub certificate_count: u32,
    pub certificate_table_length: u32,
}

impl CertificateBlockHeader {
    pub fn read_from_slice(data: &[u8]) -> Option<CertificateBlockHeader> {
        if data.len() < core::mem::size_of::<CertificateBlockHeader>() {
            return None;
        }

        Some(unsafe { (data.as_ptr() as *const CertificateBlockHeader).read_unaligned() })
    }
}
