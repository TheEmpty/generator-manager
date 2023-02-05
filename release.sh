#!/bin/bash

set -ex

# for local builds before buildx
export RUSTC_VERSION="$(rustc -V)"
export DATE_TIME="$(date -u)"

cargo clippy -- -D warnings
cargo build
cargo build --release

USER="theempty"
NAME="generator-manager"
VERSION=$(sed -E -n 's/^version = "(.+)"/\1/p' Cargo.toml)
BUILDX="pensive_albattani"
PLATFORMS="linux/amd64"

echo "Building for release, ${NAME}:${VERSION}"

TAGS=(
${USER}/${NAME}:latest
${USER}/${NAME}:${VERSION}
)

function join_tags {
    for tag in "${TAGS[@]}"; do
        printf %s " -t $tag"
    done
}

docker buildx build --builder ${BUILDX} $(join_tags) --push --platform=${PLATFORMS} .

if $(git diff --quiet) ; then
  git push
else
  echo "Dirty git tree, please manually verify and push."
fi
