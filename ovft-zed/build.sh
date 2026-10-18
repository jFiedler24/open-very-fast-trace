#!/usr/bin/env bash
# Build the importable Zed extension artefact: extension.wasm.
#
# Usage: ./build.sh           # produces ./extension.wasm (gitignored build artefact)
set -euo pipefail
cd "$(dirname "$0")"

TARGET=wasm32-wasip2
if ! rustup target list --installed | grep -q "^${TARGET}$"; then
    echo "Installing Rust target ${TARGET} ..."
    rustup target add "${TARGET}"
fi

cargo build --release --target "${TARGET}"
cp "target/${TARGET}/release/ovft_zed.wasm" extension.wasm
echo "Built $(pwd)/extension.wasm"
