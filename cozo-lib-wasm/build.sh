#!/usr/bin/env bash
#
# Build the cozo-lib-wasm package with the `fts` feature enabled.
#
# tantivy's `tantivy-sstable` crate pulls in `zstd-sys`, which compiles a
# small C shim (`wasm-shim/stdlib.h` + `string.h`) when targeting
# `wasm32-unknown-unknown`. That requires a clang capable of emitting wasm
# object files. Debian/Ubuntu users can `apt install clang-19`.
#
# Override CC_WASM / AR_WASM if you have differently-named binaries.

set -euo pipefail

CC_WASM="${CC_WASM:-clang-19}"
AR_WASM="${AR_WASM:-llvm-ar-19}"

if ! command -v "$CC_WASM" >/dev/null 2>&1; then
    echo "error: $CC_WASM not found in PATH (needed to cross-compile zstd-sys for wasm32)." >&2
    echo "       install it (e.g. 'apt install clang-19') or set CC_WASM to your clang binary." >&2
    exit 1
fi
if ! command -v "$AR_WASM" >/dev/null 2>&1; then
    echo "error: $AR_WASM not found in PATH." >&2
    echo "       install it (e.g. 'apt install llvm-19') or set AR_WASM to your llvm-ar binary." >&2
    exit 1
fi

CC_wasm32_unknown_unknown="$CC_WASM" \
AR_wasm32_unknown_unknown="$AR_WASM" \
CARGO_PROFILE_RELEASE_LTO=fat \
    wasm-pack build --target web --release
