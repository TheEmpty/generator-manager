#!/bin/bash

set -ex

cargo fmt
cargo clippy -- -D warnings
cargo build
cargo build --release

USER="theempty"
NAME="generator-manager"
TEST_REPO="192.168.7.7:5000"

docker build -t ${TEST_REPO}/${USER}/${NAME} .
docker push ${TEST_REPO}/${USER}/${NAME}
kubectl rollout restart deployment/${NAME}
sleep 45
kubectl logs -f -l app=${NAME}