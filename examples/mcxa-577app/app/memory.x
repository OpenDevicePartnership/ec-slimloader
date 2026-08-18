MEMORY
{
  /* MCXA577 app memory map */
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* Bootloader uses 0x0000_0000..0x0000_FFFF (64KiB). App starts at slot_a = 0x0001_0000. */
  FLASH  (rx) : ORIGIN = 0x00010000, LENGTH = 0x00019000  /* section 1: 100KB */
  FLASH1 (rx) : ORIGIN = 0x00180000, LENGTH = 0x00019000  /* section 2: ~1.5MB offset, 100KB */
  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}

/* Stack grows down from end of RAM */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);
