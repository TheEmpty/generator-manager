#!/bin/sh

export RUSTC_VERSION="$(rustc -V)"
export DATE_TIME="$(date -u)"
export RUST_LOG=generator_manager=debug,rocket=trace
export ROCKET_STATIC_DIR="$(pwd)/resources/static/"
export ROCKET_TEMPLATE_DIR="$(pwd)/resources/templates/"
cargo run --release

