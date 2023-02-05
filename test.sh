#!/bin/bash

set -ex

# for local builds before buildx
export RUSTC_VERSION="$(rustc -V)"
export DATE_TIME="$(date -u)"

cargo fmt
cargo clippy -- -D warnings
cargo build
cargo build --release

USER="theempty"
NAME="generator-manager"
TEST_REPO="192.168.50.3:5000"
BUILDX="pensive_albattani"
PLATFORMS="linux/amd64"

docker buildx build --builder ${BUILDX} -t ${TEST_REPO}/${USER}/${NAME} --push --platform=${PLATFORMS} .

