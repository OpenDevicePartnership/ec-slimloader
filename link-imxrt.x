/*
 *  NOTE: The NXP ROM bootloader uses 0x00000--0x1BFFF. Therefore TEXT cannot be located
 *  there or on any of the overlays. 
 */
MEMORY {
  IRAM               : ORIGIN = 0x00020000, LENGTH = 24K
  RAM                : ORIGIN = 0x20040000, LENGTH = 8K
  CONFIG_FLASH_OTFAD : ORIGIN = 0x08000000, LENGTH = 1024
  CONFIG_FLASH_FCB   : ORIGIN = 0x08000400, LENGTH = 512
  CONFIG_FLASH_BIV   : ORIGIN = 0x08000600, LENGTH = 4
  FLASH              : ORIGIN = 0x08001000, LENGTH = 1M
}

/* link descriptors at FLASH address after __bootloader_max_size */
__bootable_region_descriptors_address = ORIGIN(FLASH) + __bootloader_max_size;

__bootloader_max_size  = LENGTH(IRAM); /* for debug image loading */
__bootloader_data_size = LENGTH(RAM);  /* for RAM buffering as needed */
__bootloader_ivec_size = 0x130;        /*  */

/* # Entry point = reset vector */
EXTERN(__RESET_VECTOR);
EXTERN(Reset);
ENTRY(Reset);

EXTERN(DefaultHandler);

PROVIDE(NonMaskableInt = DefaultHandler);
EXTERN(HardFaultTrampoline);
PROVIDE(MemoryManagement = DefaultHandler);
PROVIDE(BusFault = DefaultHandler);
PROVIDE(UsageFault = DefaultHandler);
PROVIDE(SecureFault = DefaultHandler);
PROVIDE(SVCall = DefaultHandler);
PROVIDE(DebugMonitor = DefaultHandler);
PROVIDE(PendSV = DefaultHandler);
PROVIDE(SysTick = DefaultHandler);

PROVIDE(DefaultHandler = DefaultHandler_);
PROVIDE(HardFault = HardFault_);

/* # Interrupt vectors */
EXTERN(__INTERRUPTS); /* `static` variable similar to `__EXCEPTIONS` */

/* # Pre-initialization function */
/* If the user overrides this using the `pre_init!` macro or by creating a `__pre_init` function,
   then the function this points to will be called before the RAM is initialized. */
PROVIDE(__pre_init = DefaultPreInit);

/* # Sections */
SECTIONS
{
  PROVIDE(_ram_start = ORIGIN(RAM));
  PROVIDE(_ram_end = ORIGIN(RAM) + LENGTH(RAM));
  PROVIDE(_stack_start = _ram_end);

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

  /* ## Sections in IRAM */
  /* ### Vector table */
  .vector_table ORIGIN(IRAM) :
  {
    __vector_table = .;

    /* Initial Stack Pointer (SP) value.
     * We mask the bottom three bits to force 8-byte alignment.
     * Despite having an assert for this later, it's possible that a separate
     * linker script could override _stack_start after the assert is checked.
     */
    LONG(_stack_start & 0xFFFFFFF8);

    /* Reset vector */
    KEEP(*(.vector_table.reset_vector)); /* this is the `__RESET_VECTOR` symbol */

    /* NMI vector */
    KEEP(*(.vector_table.nmi_vector));
    
    /* HardFault vector */
    KEEP(*(.vector_table.hardfault_vector));
    
    /* MemManage vector */
    KEEP(*(.vector_table.memmanage_vector));
    
    /* BusFault vector */
    KEEP(*(.vector_table.busfault_vector));
    
    /* UsageFault vector */
    KEEP(*(.vector_table.usagefault_vector));
    
    /* SecureFault vector */
    KEEP(*(.vector_table.securefault_vector));
    
    /* === image length === */
    LONG(__ro_image_size);

    /* === image type === */
    /* NOTE: this must be 0x000 (Plain image), as we're using the alternative "Plain Image" 
             layout for the IRAM loader here (by embedding in vtable) */
    LONG(0x0000); /* Image Type: 0x0000 - Plain image
                                 0x0001 - Plain signed image
                                 0x0002 - Plain CRC image 
                                 0x0004 - Plain signed XIP image
                                 0x0005 - Plain CRC XIP image
                                 0x8001 - Plain signed image with KeyStore included */

    /* === auth block/crc checksum === */
    LONG(0);
    
    /* svc vector */
    KEEP(*(.vector_table.svc_vector));
    
    /* DebugMon vector */
    KEEP(*(.vector_table.debugmon_vector));
    
    /* === image load address === */
    /* NOTE: IRAM bootloader will copy fIRAM 0x8001000 to this address */
    LONG(__vector_table);

    /* PendSV vector */
    KEEP(*(.vector_table.pendsv_vector));
    
    /* SysTick vector */
    KEEP(*(.vector_table.systick_vector));
    
    /* remaining iMXRT interrupts */
    /*KEEP(*(.vector_table.interrupts)); */ /* this is the `__INTERRUPTS` symbol */

    . = ORIGIN(IRAM) + __bootloader_ivec_size;
    . = ALIGN(4);
  } > IRAM AT>FLASH =0x00

  PROVIDE(_stext = ADDR(.vector_table) + SIZEOF(.vector_table));

  /* ### .text */
  .text _stext :
  {
    __stext = .;
    *(.Reset);

    *(.text .text.*);

    /* The HardFaultTrampoline uses the `b` instruction to enter `HardFault`,
       so must be placed close to it. */
    *(.HardFaultTrampoline);
    *(.HardFault.*);

    . = ALIGN(4); /* Pad .text to the alignment to workaround overlapping load section bug in old lld */
    __etext = .;
  } > IRAM AT>FLASH

  /* ### .rodata */
  .rodata : ALIGN(4)
  {
    . = ALIGN(4);
    __srodata = .;
    *(.rodata .rodata.*);

    /* 4-byte align the end (VMA) of this section.
       This is required by LLD to ensure the LMA of the following .data
       section will have the correct alignment. */
    . = ALIGN(4);
    __erodata = .;
  } > IRAM AT>FLASH

  /* RO image size */
  __ro_image_size = (__edata - __sdata) + (__erodata - __srodata) + (__etext - __stext) + __bootloader_ivec_size;

  /* ## Sections in RAM */
  /* ### .data */
  .data : ALIGN(4)
  {
    . = ALIGN(4);
    __sdata = .;
    *(.data .data.*);
    . = ALIGN(4); /* 4-byte align the end (VMA) of this section */
  } > RAM AT>FLASH
  
  . = ALIGN(4);
  __edata = .;

  /* LMA of .data */
  __sidata = LOADADDR(.data);

  /* ### .gnu.sgstubs
     This section contains the TrustZone-M veneers put there by the Arm GNU linker. */
  /* Security Attribution Unit blocks must be 32 bytes aligned. */
  /* Note that this pads the FLASH usage to 32 byte alignment. */
  .gnu.sgstubs : ALIGN(32)
  {
    . = ALIGN(32);
    __veneer_base = .;
    *(.gnu.sgstubs*)
    . = ALIGN(32);
  } > FLASH
  /* Place `__veneer_limit` outside the `.gnu.sgstubs` section because veneers are
   * always inserted last in the section, which would otherwise be _after_ the `__veneer_limit` symbol.
   */
  . = ALIGN(32);
  __veneer_limit = .;

  /* ### .bss */
  .bss (NOLOAD) : ALIGN(4)
  {
    . = ALIGN(4);
    __sbss = .;
    *(.bss .bss.*);
    *(COMMON); /* Uninitialized C statics */
    . = ALIGN(4); /* 4-byte align the end (VMA) of this section */
  } > RAM
  /* Allow sections from user `memory.x` injected using `INSERT AFTER .bss` to
   * use the .bss zeroing mechanism by pushing __ebss. Note: do not change
   * output region or load region in those user sections! */
  . = ALIGN(4);
  __ebss = .;

  /* ### .uninit */
  .uninit (NOLOAD) : ALIGN(4)
  {
    . = ALIGN(4);
    __suninit = .;
    *(.uninit .uninit.*);
    . = ALIGN(4);
    __euninit = .;
  } > RAM

  /* Place the heap right after `.uninit` in RAM */
  PROVIDE(__sheap = __euninit);

  /* Place stack end at the end of allocated RAM */
  PROVIDE(_stack_end = __euninit);

  /* ## .got */
  /* Dynamic relocations are unsupported. This section is only used to detect relocatable code in
     the input files and raise an error if relocatable code is found */
  .got (NOLOAD) :
  {
    KEEP(*(.got .got.*));
  }

  /* ## Discarded sections */
  /DISCARD/ :
  {
    /* Unused exception related info that only wastes space */
    *(.ARM.exidx);
    *(.ARM.exidx.*);
    *(.ARM.extab.*);
  }
}

/* # Other checks */
ASSERT(SIZEOF(.got) == 0, "
ERROR(cortex-m-rt): .got section detected in the input object files
Dynamic relocations are not supported. If you are linking to C code compiled using
the 'cc' crate then modify your build script to compile the C code _without_
the -fPIC flag. See the documentation of the `cc::Build.pic` method for details.");
/* Do not exceed this mark in the error messages above                                    | */