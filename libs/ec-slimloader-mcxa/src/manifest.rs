//! Image manifest parsing (TZM omitted, CRC present for future CMAC wake path).
use core::mem::size_of;

#[derive(Debug)]
pub enum ManifestError { Magic, Version, Size, Bounds }

#[repr(C)]
pub struct ManifestHeaderRaw {
    pub magic: u32,          // "imgm" 0x6D676D69
    pub format_version: u32, // 0x00010000
    pub firmware_version: u32,
    pub manifest_size: u32,
    pub flags: u32,
}

pub struct Manifest<'a> {
    pub raw: &'a ManifestHeaderRaw,
    pub crc32: u32,
}

pub unsafe fn parse_manifest(base: *const u8, offset: u32, image_len: u32) -> Result<Manifest<'static>, ManifestError> {
    if offset >= image_len { return Err(ManifestError::Bounds); }
    let start = base.add(offset as usize);
    let raw = &*(start as *const ManifestHeaderRaw);
    if raw.magic != 0x6D676D69 { return Err(ManifestError::Magic); }
    if raw.format_version != 0x0001_0000 { return Err(ManifestError::Version); }
    if raw.manifest_size < size_of::<ManifestHeaderRaw>() as u32 { return Err(ManifestError::Size); }
    if offset + raw.manifest_size > image_len { return Err(ManifestError::Bounds); }
    // CRC32 is last 4 bytes of manifest
    let crc_off = raw.manifest_size as usize - 4;
    let crc_ptr = start.add(crc_off) as *const u32;
    let crc32 = *crc_ptr;
    Ok(Manifest { raw, crc32 })
}
