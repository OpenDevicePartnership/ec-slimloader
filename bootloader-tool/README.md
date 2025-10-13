# RT6xx Bootloader signing tool **EXPERIMENTAL**

This binary provides support for signing, downloading and running bootloaders via the secure pathway on the NXP RT6xx family of chips. It automates the signing process, as well as setting up the chip temporarily for running signed binaries. Note: It does not blow any fuses, unless using the `fuse` command, and any boot configuration changes made are erased on a power cycle.

**Note**: the RT6xx chipset enters a FAULT condition when image verification fails in the ROM bootloader. You might need to periodically powercycle your board until the complete chain runs successfully.

## Installation

This binary assumes that probe-rs and the nxp spsdk are available in the system path. Probe-rs has [installation instructions online](https://probe.rs/docs/getting-started/installation/). Spsdk can be installed on Linux platforms using:

```
pip3 install spsdk
```

| Package | Version tested |
| ------- | -------------- |
| probe-rs | 29.1 |
| spsdk | 3.1.0 |

## Usage

For creating the signed binary, certificates and the devices root symmetric key are needed. The details of the bootloader and target device memory mapping are controlled via the `config.toml` provided in this source tree.

You can view the general instructions on how to invoke the bootloader tool as follows:
```
cargo run -- --help
```

In general you first need to get the key material to be used. You can use this tool to generate it, or use some other tooling like secure hardware certificate tools to get your key material. This tool does not overwrite any pre-existing key material. You can generate it using:

```
cargo run -- generate certificates
cargo run -- generate otp
```

Then, assuming you have a bootloader and application ready (see the example folder to quickly build something that runs on the RT685S EVK), you can use the following to flash an application to slot 0:

```bash
cargo run -- run application --input-path ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-application
```

And then flash the bootloader to test that it works:
```bash
cargo run -- run bootloader --input-path ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-bootloader
```

**Note**: initially flashing the application causes the target to lock up, and you might need to powercycle before
running the bootloader.

### Signing an image using an HSM

```bash
# Copy in your image
mkdir sign_me
cp ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-bootloader sign_me/

# Prepare image for signing
cargo run -- sign bootloader --input-path sign_me/example-bootloader --dont-sign

# This will generate example-bootloader.mbi-proto.bin which you can pass to your HSM
openssl dgst -sign artifacts/cert-img1-user-key.pem -sha256 -out sign_me/signature.bin -binary sign_me/example-bootloader.mbi-proto.bin

# Lastly merge the signature into the image (this also verifies that the signature is correct)
cargo run -- sign bootloader --input-path sign_me/example-bootloader --signature-path sign_me/signature.bin

# The final signed image for flashing is then in sign_me/example-bootloader.signed.bin
```

## Binary layout

Binaries for flashing with this tool should be designed to be loaded into RAM. They should not be linked to have sections loaded into flash, as flash layout is changed somewhat by the signing process. No additional sections like keyblobs or keystores should be present. When using `cortex-m-rt`, the `example` folder can be investigated for a suggestion of the memory layout for respectively the bootloader and application corresponding to the in-tree `config.toml`.

## Method of operation
This tool takes an input ELF image and:
1. extracts all relevant sections from the given ELF
2. performs checks, such as that the sections are consecutive, not too large, and that the vector table exists on the expected memory address
3. converts the ELF sections into a consecutive binary image
4. calls the SPSDK tooling to generate a certificate block
5. packages the image as a Master Boot Image in signed and encrypted mode, set to be loaded into RAM (XIP mode but in RAM range)
6. checks the integrity of this image
7. (optionally) loads the relevant shadow registers for RTKH (certificate hashes) and OTP (decryption)
8. (optionally) uploads the signed binary to external NOR flash on the address that the 1st stage ROM bootloader
9. (optionally TODO) loads the other sections required like the FCB into external NOR flash
10. (optionally) resets the target device, causing the 2nd stage bootloader to be executed

### Other sections
The following sections need to also be set (TODO support) before the bootloader can be run:
* OTFAD: KeyBlob used for external NOR flash encryption/decryption.
* FCB: Flash Configuration Block: used to configure access to external NOR flash, is read using 1-bit mode to bootstrap better modes.
* BIV: Boot Image Version, used for ping-pong boot. We are not using this at all.