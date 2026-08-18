# ec-slimloader

A light-weight bootloader written in Rust with a fail-safe NOR-flash backed state journal.
This framework can run on any platform if support for the platform is implemented.
It is only opinionated with regards to how the state is stored.

Currently only supports the NXP IMXRT685S and IMXRT633S where it acts as a stage-two bootloader and copies the program to application RAM.
Also contains a tool for signing images, flashing them to the device, setting fuses (or shadow registers) containing crypto keys,
and an example application to showcase the bootloaders A/B state functionality for this family of chipsets.

## Organisation

This repository is split up into three parts:
* libs: library crates that implement the core functionality.
* examples/rt685s: example binary applications that be run on the RT685S evaluation kit.
* bootloader-tool: a command-line utility only used to perform operations related to the NXP RT685S platform.
  It uses the NXP SPSDK tooling to generate keys, sign images, and flash them to the target device. Also integrates probe-rs and allows for attaching to the RTT buffer for displaying `defmt` output.
  This tool is not relevant if you want to use `ec-slimloader` with any other platform.

The libraries are split out as follows:
* ec-slimloader: general library crate providing a basic structure to build your bootloader binary application.
* ec-slimloader-state: library crate with all code relating to managing the state journal. Used by both the bootloader and the application to change which image slot should be booted.
* ec-slimloader-imxrt: library crate implementing support for the NXP IMXRT685S and IMXRT633S.
* imxrt-rom: library crate implementing Rust support for the NXP ROM API which provides access to fuses and allows calling into a verification routine for images.

## How it works
Assuming your platform is already supported, you can define:
* a region of NOR-flash memory containing at least 2 pages for the bootloader state.
* at least two regions of any memory that will fit an application image.

Using the library crate for your platform (like `ec-slimloader-imxrt`) you can then implement your own bootloader binary by calling the `start` function in the `ec-slimloader` library crate.

The `ec-slimloader` crate will handle for you:
* it will read from the state journal what image slot will be booted.
* on subsequent reboots, it will fall back to your defined backup slot if you do not mark your current application image as `confirmed`.

However, some aspects are handled by the platform support crate (and can differ from project-to-project):
* how application images are loaded. For `ec-slimloader-imxrt` images are copied to RAM in a quite chip-specific way. Typically for other platforms you might want to swap images between on-chip NOR flash and external NOR flash. The latter method is not implemented in this repository (yet).
* how application images are verified. By default the images themselves are not checked at all. `ec-slimloader-imxrt` leverages the native NXP authentication routines to check image integrity.
* how application images are bootloaded, or in other words are jumped to. This differs for cortex-m or RISCV processors.

Even when using `ec-slimloader-imxrt`, you will still have to implement a few details:
* from what memory is the `ec-slimloader` started, and what memory range is used for the bootloader data?
* which memory regions are mapped to be state journal and mapped to be a image slot 0, 1, etc.?
* what is a valid memory range for the application?

Finally, your application needs to also work with the state journal to:
* after writing a new application image to a slot, marking that image slot as to be booted in the state journal.
* after rebooting, mark the current image slot from which the application is running as `confirmed`.
  If the application does not do this, the bootloader will load the old 'backup' image and mark the current boot as `failed`.

For a full tour on how to use this framework, please refer to the `examples/rt685s` folder.

## Quick guide
This guide details how to use this repository on the NXP MIMXRT685S-EVK. First step is compiling the bootloader and application:

```bash
pushd examples/rt685s
cargo build --release --features defmt
popd
```

In general, the bootloader-tool is a `clap` supported CLI application with for each subcommand a full `--help`:
```
popd bootloader-tool
cargo run -- --help
   Compiling bootloader-tool v0.1.0 (ec-slimloader/bootloader-tool)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.22s
     Running `target/debug/bootloader-tool --help`
Usage: bootloader-tool [OPTIONS] [COMMAND]

Commands:
  generate  Generate keys and certificates
  sign      Sign binaries for flashing or OTA
  download  Download binaries to the device
  run       Run binaries by going through the bootloader chain for testing purposes
  fuse      Burn fuse registers with key material and settings
  help      Print this message or the help of the given subcommand(s)

Options:
  -c, --config <FILE>  Configuration file path [default: ./config.toml]
  -h, --help           Print help
  -V, --version        Print version
```

For now we need to prepare our testing setup by generating the key material:
```bash
cargo run -- generate certificates
cargo run -- generate otp
```

This key material is only used for testing right now, and everything is put in the `./artifacts` directory. This can be configured in the `./config.toml` file.

Now we have everything ready to start flashing.
We can use run `run` command to immediately flash and `attach` in the same way you are familiar with from `probe-rs`. However, we need the bootloader to start up the application, and we need the FCB (we call everything in 0x0 to 0x1000 the 'prelude') to start the bootloader. We can extract the FCB from the `ec-slimloader` as it is built with the appropriate feature flags to include a FCB in the ELF file. Extraction happens as a side-product of signing:

```bash
cargo run -- sign bootloader -i ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-bootloader
```

We can now flash the FCB:

```bash
cargo run -- download prelude --prelude-path ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-bootloader/ec-slimloader.prelude.elf
```

And we can flash the application into *both slots*:
```bash
cargo run -- download application -i ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-application --slot 0 --certificate 0
cargo run -- download application -i ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-application --slot 1 --certificate 0
```

To flash & attach to the bootloader now run, whilst setting the OTP shadow registers:
```bash
cargo run -- download bootloader -i ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-bootloader
```

To flash & attach to the application, assuming you have a bootloader and FCB already flashed:
```bash
cargo run -- run application -i ../examples/rt685s/target/thumbv8m.main-none-eabihf/release/example-application
```

You can use the `USER_1` button to change the state journal to either `confirmed` or try the other slot in state `initial` if the current image is already `confirmed`.

You can use the `USER_2` button the reboot into the bootloader, which will set an image to `failed` if it does not verify or if it was in `attempting` without putting the state in `confirmed`.
