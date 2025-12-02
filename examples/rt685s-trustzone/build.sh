#!/bin/bash
set -e

pushd bootloader > /dev/null
cargo build --release
popd > /dev/null

pushd application-secure > /dev/null
cargo build --release
popd > /dev/null

pushd application-nonsecure > /dev/null
cargo build --release
popd > /dev/null
