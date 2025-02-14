#[link_section = ".vector_table.nmi_vector"]
#[used]
#[no_mangle]
static NMI_VECTOR: unsafe extern "C" fn() = nmi_vector;

#[no_mangle]
unsafe extern "C" fn nmi_vector() {
    loop {}
}

/* HardFault vector */
#[link_section = ".vector_table.hardfault_vector"]
#[used]
#[no_mangle]
static HARDFAULT_VECTOR: unsafe extern "C" fn() = hardfault_vector;

#[no_mangle]
unsafe extern "C" fn hardfault_vector() {
    loop {}
}

/* MemManage vector */
#[link_section = ".vector_table.memmanage_vector"]
#[used]
#[no_mangle]
static MEMMANAGE_VECTOR: unsafe extern "C" fn() = memmanage_vector;

#[no_mangle]
unsafe extern "C" fn memmanage_vector() {
    loop {}
}

/* BusFault vector */
#[link_section = ".vector_table.busfault_vector"]
#[used]
#[no_mangle]
static BUSFAULT_VECTOR: unsafe extern "C" fn() = busfault_vector;

#[no_mangle]
unsafe extern "C" fn busfault_vector() {
    loop {}
}

/* UsageFault vector */
#[link_section = ".vector_table.usagefault_vector"]
#[used]
#[no_mangle]
static USAGEFAULT_VECTOR: unsafe extern "C" fn() = usagefault_vector;

#[no_mangle]
unsafe extern "C" fn usagefault_vector() {
    loop {}
}

/* SecureFault vector */
#[link_section = ".vector_table.securefault_vector"]
#[used]
#[no_mangle]
static SECUREFAULT_VECTOR: unsafe extern "C" fn() = securefault_vector;

#[no_mangle]
unsafe extern "C" fn securefault_vector() {
    loop {}
}

/* svc vector */
#[link_section = ".vector_table.svc_vector"]
#[used]
#[no_mangle]
static SVC_VECTOR: unsafe extern "C" fn() = svc_vector;

#[no_mangle]
unsafe extern "C" fn svc_vector() {
    loop {}
}

/* DebugMon vector */
#[link_section = ".vector_table.debugmon_vector"]
#[used]
#[no_mangle]
static DEBUGMON_VECTOR: unsafe extern "C" fn() = debugmon_vector;

#[no_mangle]
unsafe extern "C" fn debugmon_vector() {
    loop {}
}

/* PendSV vector */
#[link_section = ".vector_table.pendsv_vector"]
#[used]
#[no_mangle]
static PENDSV_VECTOR: unsafe extern "C" fn() = pendsv_vector;

#[no_mangle]
unsafe extern "C" fn pendsv_vector() {
    loop {}
}

/* SysTick vector */
#[link_section = ".vector_table.systick_vector"]
#[used]
#[no_mangle]
static SYSTICK_VECTOR: unsafe extern "C" fn() = systick_vector;

#[no_mangle]
unsafe extern "C" fn systick_vector() {
    loop {}
}
