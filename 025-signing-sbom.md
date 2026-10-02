# 025 – Signing and SBOM: cosign, cargo-auditable, CycloneDX

## Goal

Let users check that a downloaded binary is the one you built (authenticity) and let anyone
see what it contains (dependency inventory, known vulnerabilities).

Example: `examples/025-signing-sbom`:

- `platctl-verify`: checksum and cosign signature verification;
- `sign-and-verify.sh`: local key-based demo;
- `ci/github-actions-sign.yml`: keyless signing, SBOM and attestations in CI.

## Key concepts

### What each piece proves

| Artifact | Proves | Weakness |
|----------|--------|----------|
| `sha256` checksum | file not corrupted in transit | attacker who replaces the archive replaces the checksum too |
| signature (cosign) | produced by the holder of a key / a specific CI identity | only as good as key management / identity policy |
| SBOM (CycloneDX, SPDX) | which components went in | a claim – trust it only when signed/attested |
| provenance (SLSA) | how/where it was built (repo, commit, workflow) | requires a trusted builder |
| auditable binary | the dependency list *inside* the binary itself | covers Rust crates only |

### cosign (Sigstore) – key-based

```bash
cosign generate-key-pair                       # cosign.key (encrypted, COSIGN_PASSWORD) + cosign.pub
cosign sign-blob --yes --key cosign.key --bundle app.tar.gz.bundle app.tar.gz
cosign verify-blob --key cosign.pub --bundle app.tar.gz.bundle app.tar.gz
```

- The **bundle** is a single JSON file holding the signature, the certificate or key hint, and
  the transparency-log proof. Prefer it over separate `.sig`/`.pem` files.
- By default cosign 3 records every signature in **Rekor**, a public append-only transparency
  log. A local or offline demo needs:
  - `--use-signing-config=false --tlog-upload=false` on `sign-blob`;
  - `--insecure-ignore-tlog=true` on `verify-blob` (`platctl-verify signature --no-tlog`).

  Never use these flags for real releases.
- The private key is a long-lived secret: rotation, leaks, and who holds it are your problems.
  Keep it in a KMS (`--key awskms://…`, `gcpkms://`, `hashivault://`).

### cosign – keyless (CI)

```
GitHub Actions job ──OIDC token──▶ Fulcio (CA) ──short-lived cert (10 min), identity = workflow URL@ref
       │ sign with ephemeral key
       └──────────────────────────▶ Rekor (transparency log) ── timestamped, public entry
```

```bash
# in CI (permissions: id-token: write)
cosign sign-blob --yes --bundle app.tar.gz.bundle app.tar.gz
# user: verify WHO signed it, not which key
cosign verify-blob --bundle app.tar.gz.bundle \
  --certificate-identity-regexp '^https://github.com/example-org/platctl/.github/workflows/release-sign.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com app.tar.gz
```

No secrets to store or rotate. The identity policy (repo + workflow + tag ref) is the trust root.

### GitHub artifact attestations

`actions/attest-build-provenance` and `actions/attest-sbom` sign SLSA provenance and the SBOM
with Sigstore and store them on GitHub. Users verify with:

```bash
gh attestation verify platctl-verify.tar.gz -R example-org/platctl
```

### cargo-auditable + cargo audit

```bash
cargo auditable build --release        # embeds the dependency tree (compressed JSON, ~4 kB) in a linker section
cargo audit bin target/release/app     # check the BINARY against the RustSec advisory DB
```

- Answers "is this deployed binary affected by RUSTSEC-xxxx?" months later, with no sources or
  `Cargo.lock` at hand.
- Also read by Trivy, Grype, Syft and osv-scanner.
- `cargo audit` (on `Cargo.lock`) and `cargo deny` belong in every PR. Update `cargo-audit`
  regularly: an old version can fail to parse new advisories (e.g. CVSS 4.0).

### SBOM with cargo-cyclonedx

```bash
cargo cyclonedx --format json --spec-version 1.5      # <package>.cdx.json next to Cargo.toml
```

- CycloneDX JSON lists components with `purl` (`pkg:cargo/clap@4.5…`), licenses and hashes.
- In a workspace it writes one file per member, even with `--manifest-path`, so move the one
  you need.
- `--target` selects target-specific dependencies (the default is the host), so generate the
  SBOM per release target when dependencies differ.
- Alternatives: `cargo sbom` (SPDX), `syft` (scans binaries and images).
- Publish the SBOM next to the archive and sign or attest it.

### Verifying in your own tool

`platctl-verify` computes SHA-256 natively (`sha2`) and shells out to `cosign` for signatures,
reusing the reference implementation instead of hand-rolling crypto verification.

A native alternative is the `sigstore` crate. A self-updater (step 026) must verify before it
replaces the binary.

## Example walkthrough

- `src/main.rs` – `checksum` (sha256sum format, `*name` binary marker, case-insensitive compare),
  `signature` (`--key` or `--identity` + `--issuer`, `--no-tlog`), tests.
- `sign-and-verify.sh`:
  1. auditable build;
  2. archive and checksum;
  3. `cargo audit bin`;
  4. SBOM;
  5. key pair;
  6. signatures for the archive and the SBOM;
  7. verification, then tampering, which fails both checks.
- `ci/github-actions-sign.yml` – keyless signing, provenance and SBOM attestations, upload to the release.

## Try it

```bash
brew install cosign && cargo install cargo-auditable cargo-audit cargo-cyclonedx --locked
examples/025-signing-sbom/sign-and-verify.sh
cargo audit bin dist/platctl-verify/platctl-verify
jq '.components[] | .purl' dist/platctl-verify/platctl-verify.cdx.json | head
jq . dist/platctl-verify/platctl-verify.cdx.json.bundle            # what a bundle contains
cargo run -p ex025-signing-sbom -- signature --help
```

## Exercises

1. Sign the `sha256.sum` file from step 024 instead of each archive, and verify a download in two steps.
2. Enable `github-attestations = true` in `dist-workspace.toml` (step 023) and check what cargo-dist adds to the workflow.
3. Replace the `cosign` subprocess with the `sigstore` crate and verify a key-based bundle natively.
