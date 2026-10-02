#!/usr/bin/env bash
# Builds release archives for 5 targets and renders package manager manifests.
# Requirements: see 021 (rustup targets, cargo-zigbuild).
#   ./package.sh            -> dist/platctl-pkg-v<VERSION>/*.tar.gz|zip + packaging/
set -euo pipefail
cd "$(dirname "$0")/../.."              # workspace root
PKG=ex024-package-managers
BIN=platctl-pkg
VERSION=$(cargo pkgid -p $PKG | sed 's/.*@//')
OUT=dist/$BIN-v$VERSION
rm -rf "$OUT" && mkdir -p "$OUT"

build() {   # build <tool> <target>
  local tool=$1 target=$2 exe=$BIN
  [[ $target == *windows* ]] && exe=$BIN.exe
  if [ "$tool" = zigbuild ]; then
    cargo zigbuild --release -p $PKG --bin $BIN --target "$target"
  else
    cargo build --release -p $PKG --bin $BIN --target "$target"
  fi
  # Archive layout: one top-level directory containing the binary (+ docs/licenses).
  local dir=$BIN-v$VERSION-$target
  mkdir -p "$OUT/$dir"
  cp "target/$target/release/$exe" "$OUT/$dir/"
  cp README.md "$OUT/$dir/" 2>/dev/null || true
  if [[ $target == *windows* ]]; then
    (cd "$OUT" && zip -qr "$dir.zip" "$dir")
  else
    tar -C "$OUT" -czf "$OUT/$dir.tar.gz" "$dir"
  fi
  rm -rf "${OUT:?}/$dir"
}

build cargo aarch64-apple-darwin
build cargo x86_64-apple-darwin
build zigbuild aarch64-unknown-linux-musl
build zigbuild x86_64-unknown-linux-musl
build zigbuild x86_64-pc-windows-gnu

# Checksums file (like cargo-dist's sha256.sum).
(cd "$OUT" && shasum -a 256 *.tar.gz *.zip > sha256.sum)
cargo run -q -p $PKG --bin render-manifests -- "$OUT" "$OUT/packaging" "$@"
ls -l "$OUT"
