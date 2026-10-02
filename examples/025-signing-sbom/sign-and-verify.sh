#!/usr/bin/env bash
# Local demo of the supply-chain steps for one artifact (key-based cosign signing).
# CI uses KEYLESS signing instead – see ci/github-actions-sign.yml.
# Requirements: cosign (brew install cosign), cargo-auditable, cargo-audit, cargo-cyclonedx.
set -euo pipefail
cd "$(dirname "$0")/../.."
PKG=ex025-signing-sbom
BIN=platctl-verify
OUT=dist/$BIN
rm -rf "$OUT" && mkdir -p "$OUT"

# 1. Auditable build: embeds the dependency list (compressed JSON) into the binary.
cargo auditable build --release -p $PKG
cp target/release/$BIN "$OUT/"
tar -C "$OUT" -czf "$OUT/$BIN.tar.gz" $BIN
(cd "$OUT" && shasum -a 256 $BIN.tar.gz > $BIN.tar.gz.sha256)

# 2. Vulnerability scan of the BINARY (works on any auditable binary, even without sources).
cargo audit bin "$OUT/$BIN"

# 3. SBOM (CycloneDX 1.5 JSON) from Cargo metadata. In a workspace cargo-cyclonedx writes
#    <package>.cdx.json next to EVERY member's Cargo.toml – keep ours, drop the rest.
cargo cyclonedx --manifest-path examples/025-signing-sbom/Cargo.toml --format json --spec-version 1.5 -q
mv examples/025-signing-sbom/$PKG.cdx.json "$OUT/$BIN.cdx.json"
find examples -maxdepth 2 -name '*.cdx.json' -delete

# 4. Signing with a key pair (demo). The private key is encrypted with COSIGN_PASSWORD.
#    Offline demo: no upload to the public transparency log (Rekor). cosign 3 needs both flags.
export COSIGN_PASSWORD=${COSIGN_PASSWORD:-demo-password}
(cd "$OUT" && cosign generate-key-pair >/dev/null)
for artifact in "$BIN.tar.gz" "$BIN.cdx.json"; do
  cosign sign-blob --yes --key "$OUT/cosign.key" --use-signing-config=false --tlog-upload=false \
    --bundle "$OUT/$artifact.bundle" "$OUT/$artifact"
done

# 5. Verification – what users (or platctl-verify) do after downloading.
"$OUT/$BIN" checksum "$OUT/$BIN.tar.gz"
"$OUT/$BIN" signature "$OUT/$BIN.tar.gz" --bundle "$OUT/$BIN.tar.gz.bundle" --key "$OUT/cosign.pub" --no-tlog
# Tampering is detected:
echo "tampered" >> "$OUT/$BIN.tar.gz"
"$OUT/$BIN" checksum "$OUT/$BIN.tar.gz" || echo "-> checksum check failed as expected"
"$OUT/$BIN" signature "$OUT/$BIN.tar.gz" --bundle "$OUT/$BIN.tar.gz.bundle" --key "$OUT/cosign.pub" --no-tlog \
  || echo "-> signature check failed as expected"
ls -l "$OUT"
