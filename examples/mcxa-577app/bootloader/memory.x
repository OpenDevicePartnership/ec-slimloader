MEMORY
{
  /* MCXA577 has 2MB flash total.
   * This example reserves the first 64KB for the bootloader.
   * Secure alias: 0x1000_0000 (Matrix0 Target Port0, Secure, All Initiators).
   */
  FLASH (rx) : ORIGIN = 0x00000000, LENGTH = 64K

  /* Use the same SRAM base as the working direct-run apps so RTT is discoverable.
   */
  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 64K
}

/* Stack grows down from end of RAM */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);
