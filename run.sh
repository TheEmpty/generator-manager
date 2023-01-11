#!/bin/bash

set -ex

export RUST_LOG=rocket=trace,generator_manager=trace
export RUSTC_VERSION="$(rustc -V)"
export DATE_TIME="$(date -u)"
cargo clippy -- -D warnings
cargo run --release

