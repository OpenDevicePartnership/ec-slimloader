#[repr(C)]
pub struct VectorAndHeaderRaw {
    // ARM CortexM vector table interleaved with NXP bootloader fields
    pub initial_sp: u32,             // 0x00 Stack pointer
    pub reset: u32,                  // 0x04 Reset handler
    pub nmi: u32,                    // 0x08 NMI
    pub hard_fault: u32,             // 0x0C HardFault
    pub mem_manage: u32,             // 0x10 MemManageFault
    pub bus_fault: u32,              // 0x14 BusFault
    pub usage_fault: u32,            // 0x18 UsageFault
    pub secure_fault: u32,           // 0x1C SecureFault
    pub image_length: u32,           // 0x20 Image length (total length - including signature)
    pub image_type: u32,             // 0x24 Image type: 0x0=plain XIP, 0x4=signed XIP, 0x5=CRC XIP
    pub extended_header_offset: u32, // 0x28 Offset to extended header (AHAB container)
    pub svc: u32,                    // 0x2C SVCall
    pub debug_mon: u32,              // 0x30 DebugMonitor
    pub load_address: u32,           // 0x34 Load address (image link address)
    pub pendsv: u32,                 // 0x38 PendSV
    pub systick: u32,                // 0x3C SysTick
}

impl VectorAndHeaderRaw {
    pub const SIZE: usize = core::mem::size_of::<Self>(); // 0x40 bytes (16 x u32)
}

pub struct ImageHeader<'a> {
    pub raw: &'a VectorAndHeaderRaw,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeaderError {
    LengthZero,
    LengthTooSmall,
    LengthTooLarge,
    CertOffset,
    Alignment,
    Type,
}

impl<'a> ImageHeader<'a> {
    /// # Safety
    ///
    /// `ptr` must be a valid pointer to a readable, 4-byte-aligned region of at least `slot_size` bytes.
    pub unsafe fn from_ptr(ptr: *const u8, slot_size: u32) -> Result<Self, HeaderError> {
        if !(ptr as usize).is_multiple_of(4) {
            return Err(HeaderError::Alignment);
        }
        if (slot_size as usize) < (core::mem::size_of::<VectorAndHeaderRaw>()) {
            return Err(HeaderError::LengthTooSmall);
        }
        let raw = &*(ptr as *const VectorAndHeaderRaw);
        if raw.image_length == 0 {
            return Err(HeaderError::LengthZero);
        }
        if raw.image_length > slot_size {
            return Err(HeaderError::LengthTooLarge);
        }
        if raw.extended_header_offset >= raw.image_length {
            return Err(HeaderError::CertOffset);
        }
        // Enforce signed XIP image type (0x04) for cold boot (lower 6 bits == 0x04)
        let img_type_low = raw.image_type & 0x3F;
        if img_type_low != 0x04 {
            return Err(HeaderError::Type);
        }
        Ok(Self { raw })
    }

    pub fn image_length(&self) -> u32 {
        self.raw.image_length
    }
    pub fn cert_block_offset(&self) -> u32 {
        self.raw.extended_header_offset
    }
    pub fn load_address(&self) -> u32 {
        self.raw.load_address
    }
    pub fn container_header_offset(&self) -> u32 {
        self.raw.extended_header_offset
    }
    pub fn extended_header_offset(&self) -> u32 {
        self.raw.extended_header_offset
    }
    pub fn certificate_offset(&self) -> u32 {
        self.cert_block_offset()
    }
    pub fn manifest_offset(&self) -> u32 {
        // assume manifest immediately after certificate block
        self.cert_block_offset() // caller will add certificate size once parsed
    }
}

// TODO: Active image protection (GLBAC/XOM): determine start/end addresses to protect
//       after successful verification. Likely protect Slot A region or [image_base .. image_base+image_length].
