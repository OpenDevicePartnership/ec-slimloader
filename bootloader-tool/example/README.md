# Example bootloader & application

These are minimal examples that should work on the IMXRT685S EVK. For build both projects with

```
cargo build --release
```

And then in the `ms-bootloader-tool` folder run:

```
cargo run -- generate certificates
cargo run -- generate otp
cargo run -- download application --input-path ./example/application/target/thumbv8m.main-none-eabihf/release/example-application
cargo run -- run bootloader --input-path ./example/bootloader/target/thumbv8m.main-none-eabihf/release/example-bootloader
```

**Note**: initially flashing the application causes the target to lock up, and you might need to powercycle before
running the bootloader.
