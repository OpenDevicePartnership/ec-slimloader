#!/bin/bash

set -eo pipefail

if ! command -v cargo-batch &> /dev/null; then
    echo "cargo-batch could not be found. Install it with the following command:"
    echo ""
    echo "    cargo install --git https://github.com/embassy-rs/cargo-batch cargo --bin cargo-batch --locked"
    echo ""
    exit 1
fi

export RUSTFLAGS=-Dwarnings
export DEFMT_LOG=trace
if [[ -z "${CARGO_TARGET_DIR}" ]]; then
    export CARGO_TARGET_DIR=target_ci
fi

TARGET="thumbv8m.main-none-eabihf"

BUILD_EXTRA=""

FEATURE_COMBINATIONS=(
  "mcxa5xx,mimxrt633s,defmt"
  "mcxa5xx,mimxrt633s,log"
  "mcxa5xx,mimxrt633s,defmt,non-secure"
  "mcxa5xx,mimxrt685s,defmt"
  "mcxa5xx,mimxrt685s,log"
  "mcxa5xx,mimxrt685s,defmt,non-secure"
)
DEFMT_LOG="off" cargo batch \
      $(for features in "${FEATURE_COMBINATIONS[@]}"; do
	echo "--- build --manifest-path Cargo.toml --target thumbv8m.main-none-eabihf --features $features --no-default-features"
	done) $BUILD_EXTRA

cargo test --locked --workspace --manifest-path Cargo.toml --target x86_64-unknown-linux-gnu --features "mimxrt633s" --exclude ec-slimloader-mcxa --exclude mcxa-security-provisioning --no-default-features
cargo test --locked --workspace --manifest-path Cargo.toml --target x86_64-unknown-linux-gnu --features "mimxrt685s" --exclude ec-slimloader-mcxa --exclude mcxa-security-provisioning --no-default-features
