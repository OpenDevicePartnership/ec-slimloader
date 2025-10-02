# ec-slimloader **OUTDATED**

A light-weight stage-two bootloader written in Rust for loading an app image as configured by ec-slimloader-descriptors. Also contains a tool for signing images, flashing them to the device, setting fuses (or shadow registers) containing crypto keys, and an example application to showcase the bootloaders A/B state functionality.

Currently this bootloader can only be used on the IMXRT600 series of chipsets from NXP.

## Organisation

This repository is split up into four parts:
* ec-slimloader: the binary project which forms the second stage bootloader
* ec-slimloader-descriptors: the library crate containing a descriptor of where each image slot exists, as well as a persistent fail-safe state journal for recording the A/B bootloading state.
* bootloader-tool: a command-line utility using the NXP SPSDK tooling to generate keys, sign images, and flash them to the target device. Also integrates probe-rs and allows for attaching to the RTT buffer for displaying `defmt` output.
* example: an example application image that uses the state-journal to select alternative images to execute.

## Memory layout
This repository has default configuration files detailing the used memory layout. This layout will probably will need to be adapted for your specific usecase.

## Quick guide
This guide details how to use this repository on the NXP MIMXRT685S-EVK. First step is compiling the bootloader and application:

```bash
pushd ec-slimloader
cargo build --release --features defmt
popd
pushd examples/rt685s-application
cargo build --release
popd
```

In general, the bootloader-tool is a `clap` supported CLI application with for each subcommand a full `--help`:
```
cargo run -- --help
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running `target/debug/bootloader-tool --help`
Usage: bootloader-tool [OPTIONS] [COMMAND]

Commands:
  generate  Generate keys and certificates
  sign      Sign binaries for flashing or OTA
  download  Download binaries to the device
  run       Run binaries, setting the shadow registers, by going through the bootloader chain for testing purposes
  fuse      Burn fuse registers with key material and settings
  help      Print this message or the help of the given subcommand(s)

Options:
  -c, --config <FILE>  Configuration file path [default: ./config.toml]
  -h, --help           Print help
  -V, --version        Print version
```

For now we need to prepare our testing setup by generating the key material:
```bash
cd bootloader-tool
cargo run -- generate certificates
cargo run -- generate otp
```

This key material is only used for testing right now, and everything is put in the `./artifacts` directory. This can be configured in the `./config.toml` file.
We are working on a setup to also support external HSM integration.

Now we have everything ready to start flashing.
We can use run `run` command to immediately flash and `attach` in the same way you are familiar with from `probe-rs`. However, we need the bootloader to start up the application, and we need the FCB (we call everything in 0x0 to 0x1000 the 'prelude') to start the bootloader. We can extract the FCB from the `ec-slimloader` as it is built with the appropriate feature flags to include a FCB in the ELF file. Extraction happens as a side-product of signing:

```bash
cargo run -- sign bootloader -i ../target/thumbv8m.main-none-eabihf/release/ec-slimloader
```

We can now flash the FCB:

```bash
cargo run -- download prelude --prelude-path ../target/thumbv8m.main-none-eabihf/release/ec-slimloader.prelude.elf
```

And we can flash the application into *both slots*:
```bash
cargo run -- download application -i ../examples/rt685s-application/target/thumbv8m.main-none-eabihf/release/example-application --slot 0
cargo run -- download application -i ../examples/rt685s-application/target/thumbv8m.main-none-eabihf/release/example-application --slot 1
```

To flash & attach to the bootloader now run, whilst setting the OTP shadow registers:
```bash
cargo run -- download bootloader -i ../target/thumbv8m.main-none-eabihf/release/ec-slimloader
```

To flash & attach to the application (TODO it now is not resetting the state journal so take care), assuming you have a and FCB bootloader already flashed:
```bash
cargo run -- run application -i ../examples/rt685s-application/target/thumbv8m.main-none-eabihf/release/example-application
```

You can use the `USER_1` button to change the state journal to either `confirmed` or try the other slot in state `initial` if the current image is already `confirmed`.

You can use the `USER_2` button the reboot into the bootloader, which will set an image to `failed` if it does not verify or if it was in `attempting` without putting the state in `confirmed`.
