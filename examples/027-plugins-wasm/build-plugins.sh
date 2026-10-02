#!/usr/bin/env bash
# Builds the example plugins as WebAssembly components and installs them into target/plugins
# (the default --plugin-dir). Requires: rustup target add wasm32-wasip2
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build --release --target wasm32-wasip2 \
  --manifest-path examples/027-plugins-wasm/plugins/Cargo.toml --target-dir target/wasm-plugins
mkdir -p target/plugins
cp target/wasm-plugins/wasm32-wasip2/release/{redact,rogue}.wasm target/plugins/
ls -l target/plugins
