// Internal flash layout for the MCXA bootloader path.
// Secure alias: 0x1000_0000 – 0x101F_FFFF (2 MB, Matrix0 Target Port0, All Initiators, Secure).
// The bootloader always accesses flash through the secure alias.
pub const INTERNAL_FLASH_START: u32 = 0x0000_0000;
pub const INTERNAL_FLASH_SIZE: u32 = 0x0020_0000;
pub const INTERNAL_FLASH_SECTOR_SIZE: u32 = 0x2000; // 8 KB
pub const INTERNAL_FLASH_PAGE_SIZE: u32 = 128; // 128 B

// Bootloader region (64 KB)
pub const BOOTLOADER_START: u32 = INTERNAL_FLASH_START;
pub const BOOTLOADER_SIZE: u32 = 0x0001_0000;
pub const BOOTLOADER_END: u32 = BOOTLOADER_START + BOOTLOADER_SIZE - 1;

// Slot A (internal flash application image)
pub const SLOT_A_START: u32 = BOOTLOADER_START + BOOTLOADER_SIZE;
pub const SLOT_A_SIZE: u32 = INTERNAL_FLASH_SIZE - BOOTLOADER_SIZE - JOURNAL_SIZE; // remainder minus journal
pub const SLOT_A_END: u32 = SLOT_A_START + SLOT_A_SIZE - 1;

// Journal (last 2 sectors)
pub const JOURNAL_SIZE: u32 = INTERNAL_FLASH_SECTOR_SIZE * 2; // 16 KB
pub const JOURNAL_START: u32 = INTERNAL_FLASH_START + INTERNAL_FLASH_SIZE - JOURNAL_SIZE; // 0x101F_C000
pub const JOURNAL_END: u32 = JOURNAL_START + JOURNAL_SIZE - 1;

// Ensure Slot A does not overlap journal
const _: () = assert!(SLOT_A_END < JOURNAL_START);

// Slot B (external secure FlexSPI window)
pub const SLOT_B_START: u32 = 0x9000_0000;
pub const SLOT_B_SIZE: u32 = SLOT_A_SIZE; // symmetric
pub const SLOT_B_END: u32 = SLOT_B_START + SLOT_B_SIZE - 1;

// Image header constants
pub const IMAGE_MAGIC: u32 = 0x534C_4D43; // 'SLMC'

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotId {
    A,
    B,
}

impl SlotId {
    pub fn as_index(self) -> u8 {
        match self {
            SlotId::A => 0,
            SlotId::B => 1,
        }
    }
}

pub fn slot_a_sector_count() -> u32 {
    SLOT_A_SIZE / INTERNAL_FLASH_SECTOR_SIZE
}
pub fn slot_a_sector_start(i: u32) -> u32 {
    SLOT_A_START + i * INTERNAL_FLASH_SECTOR_SIZE
}
pub fn is_page_aligned(addr: u32) -> bool {
    addr.is_multiple_of(INTERNAL_FLASH_PAGE_SIZE)
}
