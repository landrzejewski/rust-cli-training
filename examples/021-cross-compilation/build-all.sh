#!/usr/bin/env bash
# Builds platctl-sysinfo for several targets from one machine (tested on macOS arm64).
#
# Requirements:
#   rustup target add x86_64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl x86_64-pc-windows-gnu
#   pip install ziglang && cargo install cargo-zigbuild --locked     (zig = C toolchain + linker for any target)
set -euo pipefail
cd "$(dirname "$0")/../.."          # workspace root
PKG=ex021-cross-compilation
BIN=platctl-sysinfo
OUT=dist/$BIN
mkdir -p "$OUT"

# 1. macOS: both architectures with Apple's toolchain, then one universal ("fat") binary.
cargo build --release -p $PKG --target aarch64-apple-darwin
cargo build --release -p $PKG --target x86_64-apple-darwin
lipo -create -output "$OUT/$BIN-macos-universal" \
  target/aarch64-apple-darwin/release/$BIN \
  target/x86_64-apple-darwin/release/$BIN

# 2. Linux, fully static (musl) – runs on any distribution, also in `scratch` containers.
#    cargo-zigbuild uses zig as the C compiler/linker for the target.
for target in aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  cargo zigbuild --release -p $PKG --target $target
  cp target/$target/release/$BIN "$OUT/$BIN-$target"
done

# 3. Windows (GNU ABI) – also via zig; the MSVC ABI needs Windows (or cargo-xwin).
cargo zigbuild --release -p $PKG --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/$BIN.exe "$OUT/$BIN-x86_64-pc-windows-gnu.exe"

file "$OUT"/*
