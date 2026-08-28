MEMORY {
    FLASH     : ORIGIN = 0x00026800, LENGTH = 32K
    RAM       : ORIGIN = 0x2002E800, LENGTH = 32K
    SHAREDRTT : ORIGIN = 0x20026000, LENGTH = 2K
}

SECTIONS {
    .shared_rtt (NOLOAD) : ALIGN(4)
    {
        . = ALIGN(4);
        KEEP(* (.shared_rtt.header))
        KEEP(* (.shared_rtt.buffer))
        . = ALIGN(4);
    } > SHAREDRTT AT>FLASH
    . = ALIGN(4);
}
