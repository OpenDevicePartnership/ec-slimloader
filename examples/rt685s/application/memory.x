MEMORY {
    FLASH              : ORIGIN = 0x10020000, LENGTH = 1M
    RAM                : ORIGIN = 0x30120000, LENGTH = 32K
    ROM_TABLE (r)      : ORIGIN = 0x1303F000, LENGTH = 64
}

SECTIONS {
  .rom_table ORIGIN(ROM_TABLE) (NOLOAD): {
    API_TABLE = .;
    . += LENGTH(ROM_TABLE);
  } > ROM_TABLE
} INSERT AFTER .uninit;