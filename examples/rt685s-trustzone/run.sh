#!/bin/bash
set -e

./build.sh

pushd ../../bootloader-tool > /dev/null
cargo run -- download bootloader \
    -i ../examples/rt685s-trustzone/target/thumbv8m.main-none-eabihf/release/example-bootloader
cargo run -- run application \
    -i ../examples/rt685s-trustzone/target/thumbv8m.main-none-eabihf/release/secure-app \
    -i ../examples/rt685s-trustzone/target/thumbv8m.main-none-eabihf/release/$1
popd
