MEMORY {
  PRELUDE_OTFAD : ORIGIN = 0x08000000, LENGTH = 256
  PRELUDE_FCB   : ORIGIN = 0x08000400, LENGTH = 512
  PRELUDE_BIV   : ORIGIN = 0x08000600, LENGTH = 4

  RAM                : ORIGIN = 0x20114000, LENGTH = 32K
  FLASH              : ORIGIN = 0x0010C000, LENGTH = 24K /* running in load-to-ram mode */
  ROM_TABLE (r)      : ORIGIN = 0x1303F000, LENGTH = 64
}

SECTIONS {
  .otfad : {
    . = ALIGN(4);
    KEEP(* (.otfad))
    . = ALIGN(4);
  } > PRELUDE_OTFAD

  .fcb : {
    . = ALIGN(4);
    KEEP(* (.fcb))
    . = ALIGN(4);
  } > PRELUDE_FCB

  .biv : {
    . = ALIGN(4);
    KEEP(* (.biv))
    . = ALIGN(4);
  } > PRELUDE_BIV

  .rom_table ORIGIN(ROM_TABLE) (NOLOAD): {
    API_TABLE = .;
    . += LENGTH(ROM_TABLE);
  } >ROM_TABLE
} INSERT AFTER .uninit;
