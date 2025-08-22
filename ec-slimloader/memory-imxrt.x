/*
 *  NOTE: The NXP ROM bootloader uses 0x00000--0x1BFFF. Therefore TEXT cannot be located
 *  there or on any of the overlays. 
 */
MEMORY {
  RAM                : ORIGIN = 0x20026000, LENGTH = 8K
  CONFIG_FLASH_OTFAD : ORIGIN = 0x08000000, LENGTH = 1024
  CONFIG_FLASH_FCB   : ORIGIN = 0x08000400, LENGTH = 512
  CONFIG_FLASH_BIV   : ORIGIN = 0x08000600, LENGTH = 4
  FLASH              : ORIGIN = 0x08001000, LENGTH = 24K /* running in XiP mode */
  DESCRIPTORS        : ORIGIN = 0x08009000, LENGTH = 4K
  ROM_TABLE (r)      : ORIGIN = 0x1303F000, LENGTH = 64
}

/* link descriptors at FLASH address after 32KB Bootloader Range */
__bootable_region_descriptors_address = 0x08009000;

__bootloader_ivec_size = 0x130;

SECTIONS {
  .otfad : {
    . = ALIGN(4);
    KEEP(* (.otfad))
    . = ALIGN(4);
  } > CONFIG_FLASH_OTFAD

  .fcb : {
    . = ALIGN(4);
    KEEP(* (.fcb))
    . = ALIGN(4);
  } > CONFIG_FLASH_FCB

  .biv : {
    . = ALIGN(4);
    KEEP(* (.biv))
    . = ALIGN(4);
  } > CONFIG_FLASH_BIV

  .descriptors : {
    . = ALIGN(4);
    KEEP(* (.biv))
    . = ALIGN(4);
  } > DESCRIPTORS

  .rom_table ORIGIN(ROM_TABLE) (NOLOAD): {
    API_TABLE = .;
    . += LENGTH(ROM_TABLE);
  } >ROM_TABLE
}

