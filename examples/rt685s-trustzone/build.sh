#!/bin/bash
set -e

pushd application-secure
cargo build --release
popd

pushd application-nonsecure
cargo build --release
popd