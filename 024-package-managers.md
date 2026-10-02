# 024 – Publishing: Homebrew, Winget, Scoop, AUR, cargo install, Docker

## Goal

Get a CLI onto users' machines the way they already install software – per platform package
manager, Rust toolchain, or container – from one set of release archives.

Example: `examples/024-package-managers` – `platctl-pkg`, `package.sh` (5 archives),
`render-manifests` (fills manifest templates with URLs and SHA-256), `Dockerfile`; rendered
output in `generated/`.

## Key concepts

### Channels

| Channel | Users | What you publish | Where |
|---------|-------|------------------|-------|
| Homebrew | macOS, Linux | Ruby formula with per-platform URL + sha256 | your tap repo `homebrew-<name>` (or homebrew-core) |
| Scoop | Windows devs | JSON manifest (`url`, `hash`, `bin`, `checkver`, `autoupdate`) | your bucket repo |
| Winget | Windows | 3 YAML manifests (version, installer, locale) | PR to `microsoft/winget-pkgs` |
| AUR | Arch Linux | `PKGBUILD` + `.SRCINFO` (`-bin` = prebuilt) | `aur.archlinux.org` git |
| `cargo install` | Rust developers | crate on crates.io (compiles locally) | crates.io |
| `cargo binstall` | Rust developers | `[package.metadata.binstall]` → downloads your archives | GitHub Releases |
| Docker / OCI | CI pipelines, k8s jobs | multi-arch image | GHCR, Docker Hub |
| shell/ps1 installer | everyone | `curl … \| sh` (cargo-dist, step 023) | GitHub Releases |

All of them point at the **same release archives** (`platctl-pkg-v1.2.0-<target>.tar.gz|zip`)
and verify them with **SHA-256** – the manifest pins the exact bytes.

### Archive conventions

```
platctl-pkg-v1.2.0-aarch64-apple-darwin.tar.gz
└── platctl-pkg-v1.2.0-aarch64-apple-darwin/
    ├── platctl-pkg
    └── README.md            (+ LICENSE, completions, man pages – steps 010/022)
```

`.tar.gz`/`.tar.xz` for Unix, `.zip` for Windows; predictable names (target triple) make
templates and `binstall` URLs trivial.

### Homebrew (tap)

```ruby
class PlatctlPkg < Formula
  desc "..."; homepage "..."; version "1.2.0"; license "MIT"
  on_macos do
    on_arm   do url "…aarch64-apple-darwin.tar.gz"; sha256 "…" end
    on_intel do url "…x86_64-apple-darwin.tar.gz";  sha256 "…" end
  end
  on_linux do … end
  def install; bin.install "platctl-pkg"; end
  test do assert_match version.to_s, shell_output("#{bin}/platctl-pkg --version") end
end
```

`brew install example-org/tap/platctl-pkg`; cargo-dist can publish the formula automatically
(`publish-jobs = ["homebrew"]`). Add `generate_completions_from_executable` / `man1.install`
to ship completions and man pages.

### Scoop / Winget

- Scoop: `checkver: "github"` + `autoupdate` let bots bump versions automatically,
- Winget: portable zip install (`InstallerType: zip`, `NestedInstallerType: portable`,
  `PortableCommandAlias`); submit with `wingetcreate update ExampleOrg.PlatctlPkg -v 1.2.0 -u <url>`
  or the `winget-releaser` GitHub Action; hashes in uppercase hex; validated by Microsoft's pipeline.

### AUR

`platctl-pkg-bin`: `source_<arch>` + `sha256sums_<arch>`, `package()` installs into `$pkgdir`;
`provides`/`conflicts` with the from-source package. Generate `.SRCINFO` with `makepkg --printsrcinfo`.

### cargo install / binstall

```bash
cargo install platctl-pkg --locked          # builds from source; --locked = use Cargo.lock
cargo binstall platctl-pkg                  # prebuilt binary via [package.metadata.binstall]
```

`--locked` matters: without it dependency versions may drift from what you tested.

### Docker image from prebuilt binaries

```dockerfile
FROM alpine:3 AS pick         # choose the archive for $TARGETARCH (buildx)
...
FROM scratch                  # static musl binary only – 354 kB image here
COPY --from=pick /platctl-pkg /usr/local/bin/platctl-pkg
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/platctl-pkg"]
```

`docker buildx build --platform linux/amd64,linux/arm64 --push` → one multi-arch tag.
Same artifacts as the archives (no second build), tiny attack surface. Add CA certificates
(`COPY --from=alpine /etc/ssl/certs ...`) if the CLI talks HTTPS.

### Automate it

Manifests are generated, never hand-edited per release: templates + checksums in CI
(`render-manifests` here; cargo-dist for Homebrew/installers; dedicated actions for winget,
Scoop bucket bots, `aur-publish` actions).

## Example walkthrough

- `Cargo.toml` – version 1.2.0, repository URL, `[package.metadata.binstall]`.
- `package.sh` – builds 5 targets (step 021 tooling), archives, `sha256.sum`, runs `render-manifests`.
- `src/bin/render-manifests.rs` – embedded templates (`include_str!`), SHA-256 (`sha2`), strict placeholder rendering; tests.
- `templates/` – Homebrew, Scoop, Winget (3 files), AUR; `generated/` – rendered result.
- `Dockerfile` – image from prebuilt musl archives.

Verified: Homebrew formula installed from a local tap (`file://` URLs), `brew test` passed;
Scoop JSON and Winget YAML parse; PKGBUILD syntax; Docker image built and run (arm64).

## Try it

```bash
examples/024-package-managers/package.sh
cat dist/platctl-pkg-v1.2.0/sha256.sum
# Homebrew end-to-end with a temporary local tap
cargo run -p ex024-package-managers --bin render-manifests -- dist/platctl-pkg-v1.2.0 /tmp/pkg --base-url "file://$PWD/dist/platctl-pkg-v1.2.0"
brew tap-new --no-git example-org/training
cp /tmp/pkg/homebrew/platctl-pkg.rb "$(brew --repository)/Library/Taps/example-org/homebrew-training/Formula/"
brew install example-org/training/platctl-pkg && brew test example-org/training/platctl-pkg
brew uninstall platctl-pkg && brew untap example-org/training
# container
docker buildx build --platform linux/arm64 --load -f examples/024-package-managers/Dockerfile -t platctl-pkg:1.2.0 dist/platctl-pkg-v1.2.0
docker run --rm platctl-pkg:1.2.0 --version
```

## Exercises

1. Add shell completions and a man page (step 010) to the archives and install them in the formula and PKGBUILD.
2. Add a `.deb` package with `cargo-deb` and an `.rpm` with `cargo-generate-rpm`.
3. Write a GitHub Actions job that opens a PR in the tap repository with the rendered formula after each release.
