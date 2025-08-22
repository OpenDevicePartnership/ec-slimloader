/*
 *  NOTE: The NXP ROM bootloader uses 0x00000--0x1BFFF. Therefore TEXT cannot be located
 *  there or on any of the overlays. 
 */
MEMORY {
  RAM                : ORIGIN = 0x20114000, LENGTH = 32K
  CONFIG_FLASH_OTFAD : ORIGIN = 0x08000000, LENGTH = 256
  CONFIG_FLASH_FCB   : ORIGIN = 0x08000400, LENGTH = 512
  CONFIG_FLASH_BIV   : ORIGIN = 0x08000600, LENGTH = 4
  FLASH              : ORIGIN = 0x0010C000, LENGTH = 32K /* running in load-to-ram mode */
  ROM_TABLE (r)      : ORIGIN = 0x1303F000, LENGTH = 64
}

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

  .rom_table ORIGIN(ROM_TABLE) (NOLOAD): {
    API_TABLE = .;
    . += LENGTH(ROM_TABLE);
  } >ROM_TABLE
} INSERT AFTER .uninit;

