# RT685S example with Trustzone
This example uses the nightly compiler the `abi_cmse_nonsecure_call` and `cmse_nonsecure_entry` features.

It is split into three parts:
* a Secure mode true bootloader as part of the ec-slimloader ecosystem.
* a Secure mode mini 'bootloader' that configures all busses and the core. It has a small veneer with the `do_stuff_secure` function that is callable from the NonSecure mode. All peripherals are set to NonSecure, with the notable exception being the OTP peripheral.
* a collection of NonSecure binaries that showcase TrustZone.

The latter two binaries are merged by the bootloader-tool into a single MBI container for ec-slimloader to verify and boot.

It is imperative that the secure firmware is compiled first.
It places a `veneers.o` in the target folder.
The nonsecure links that in to be able to call secure functions.

In order to quickly run this example, call `run.sh secure_function`, or use any other binary in `application-nonsecure/src/bin`.
