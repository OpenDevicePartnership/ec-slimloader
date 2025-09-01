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
export DEFMT_LOG=trace,embassy_hal_internal=debug
if [[ -z "${CARGO_TARGET_DIR}" ]]; then
    export CARGO_TARGET_DIR=target_ci
fi

TARGET="thumbv8m.main-none-eabihf"

BUILD_EXTRA=""

FEATURE_COMBINATIONS=(
  "mimxrt633s"
  "mimxrt633s,defmt"
  "mimxrt685s"
  "mimxrt685s,defmt"
)
cargo batch \
      $(for features in "${FEATURE_COMBINATIONS[@]}"; do
	  echo "--- build --manifest-path Cargo.toml --target thumbv8m.main-none-eabihf --features $features "
	  echo "--- build --release --manifest-path Cargo.toml --target thumbv8m.main-none-eabihf --features $features "
	done) $BUILD_EXTRA

cargo clippy --locked --manifest-path Cargo.toml --target thumbv8m.main-none-eabihf --features "mimxrt633s" -- -Dwarnings -F clippy::suspicious -F clippy::correctness -F clippy::perf -F clippy::style
cargo clippy --locked --manifest-path Cargo.toml --target thumbv8m.main-none-eabihf --features "mimxrt685s" -- -Dwarnings -F clippy::suspicious -F clippy::correctness -F clippy::perf -F clippy::style