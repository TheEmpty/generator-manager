#!/bin/sh

export RUSTC_VERSION="$(rustc -V)"
export DATE_TIME="$(date -u)"
export RUST_LOG=generator_manager=trace,rocket=trace
cargo run --release

