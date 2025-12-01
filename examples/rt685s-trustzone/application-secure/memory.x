MEMORY {
    FLASH         : ORIGIN = 0x10020000, LENGTH = 20K
    RAM           : ORIGIN = 0x30025000, LENGTH = 4K
    SHAREDRTT     : ORIGIN = 0x20026000, LENGTH = 2K
    # APPLICATION   : ORIGIN = 0x00026800
    ROM_TABLE (r) : ORIGIN = 0x1303F000, LENGTH = 64
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
