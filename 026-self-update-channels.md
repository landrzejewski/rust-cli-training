# 026 – Self-update: GitHub Releases, channels, version checks, opt-out

## Goal

Let users of a standalone binary (installed with `curl | sh`, from a release archive, from an
internal share) move to new versions without a package manager. Do it safely: verify downloads,
smoke-test the new binary, and never nag in scripts or CI.

Example: `examples/026-self-update-channels` – `platctl-up`, plus `demo.sh`. The demo builds four
releases, serves a manifest feed locally, then has 0.1.0 update itself through stable and beta to
0.3.0.

## Key concepts

### When a CLI should (not) update itself

| Installed via | Who updates | Self-update? |
|---------------|-------------|--------------|
| `curl \| sh`, release archive, internal share | the tool | yes |
| Homebrew, Scoop, winget, apt/dnf, Nix | the package manager | **no** – refuse and point to `brew upgrade` |
| `cargo install` | `cargo install` again | possible, but `.crates.toml` gets out of sync |
| container image | new image tag | no – images are immutable |

`platctl-up` detects package-manager paths (`/Cellar/`, `/nix/store/`, `scoop\apps`, `/usr/bin/`)
and refuses. Distro packagers often also want a compile-time switch: a Cargo feature
`self-update`, enabled by default and disabled in their builds.

### self_update (1.x)

```rust
let status = self_update::backends::github::Update::configure()
    .repo_owner("example-org").repo_name("platctl")
    .bin_name("platctl-up")                          // binary name inside the archive
    .current_version(env!("CARGO_PKG_VERSION"))
    .auth_token_from_env()                           // GH_TOKEN / GITHUB_TOKEN: 5000 req/h instead of 60
    .release_tag("v1.4.0")                           // optional pin; otherwise picks newest per UpdateStrategy
    .no_confirm(yes)                                 // default prompts on stdin – set it for --yes
    .verify_binary(|new_exe| smoke_test(new_exe))    // last gate before replacing
    .build()?
    .update()?;                                      // download → verify → extract → replace
```

- **Backends:**
  - GitHub, GitLab, Gitea and Gitee Releases;
  - S3, GCS and any S3-compatible bucket;
  - a **manifest** feed: one `manifest.json` on any static host. Simple to mirror internally, and
    no API rate limits.
- **Asset selection:** the asset name must contain the target triple
  (`platctl-up-1.4.0-aarch64-apple-darwin.tar.gz`). These are the archive conventions from
  steps 023/024.
- **Verification pipeline:**
  1. `verify_checksum` (and/or `checksum_from_asset("SHA256SUMS")`);
  2. the release digest (GitHub publishes a sha256 per asset; the manifest has `digest`);
  3. zipsign signatures (`signatures` feature);
  4. `verify_archive` hook, e.g. run `cosign verify-blob` / `gh attestation verify` (step 025);
  5. extract;
  6. `verify_binary` hook;
  7. replace.
- **Replacement:** `self-replace` under the hood. On Unix it renames over the running file
  (the inode stays alive). On Windows the running exe is moved aside and cleaned up later.
  `restart::restart()` re-executes the new binary.
- **Install location not writable** (e.g. `/usr/local/bin` without sudo): you get
  `Error::InstallPathNotWritable`. The crate never escalates privileges. Tell the user what to run.

### Channels

```
releases:  0.1.0  0.2.0  0.3.0-beta.1  0.3.0
stable:    ✓      ✓                    ✓        (no SemVer pre-release part)
beta:      ✓      ✓      ✓             ✓
```

- The channel is a **filter over one release list**. SemVer orders `0.3.0-beta.1 < 0.3.0`, so a
  beta user moves to the final release automatically.
- Switching back to stable never downgrades: the beta stays until a newer stable release ships.
- The channel picks the version; the updater is then **pinned** with `release_tag`, so the
  library's own "newest" logic cannot slip a beta to a stable user.
- **Alternative:** one feed per channel (`stable/manifest.json`, `beta/manifest.json`, like
  browsers and VS Code). This allows staged rollouts: publish to beta first, promote later.
- The selected channel is user **state**, not config: it lives in `$XDG_STATE_HOME/platctl-up/`.
  A beta build defaults to the beta channel.

### Version checks and opt-out

Good citizens (cf. `gh`, `npm`'s update-notifier):

- check **at most once per interval** (`check_interval::UpdateCheckGuard`, a stamp file). Stamp
  the attempt even when it fails, so an offline machine doesn't retry on every run;
- **never** in CI (`CI` env var), never when stderr is not a terminal (pipes, scripts, cron);
- opt-out: `--no-update-check`, `PLATCTL_NO_UPDATE_CHECK=1`, or a config key;
- run the check in a **background thread**, started before the command and collected after with a
  short grace period. It never delays or fails the real work;
- print the notice to **stderr**, so stdout stays clean for `| jq`;
- `update --check` exits with a distinct code (10 here) so scripts can branch;
- privacy: a check is a request with the user's IP and version. Document it, and make it easy to
  disable or point at an internal mirror (`PLATCTL_UP_FEED`).

### Manifest feed (schema 1)

```json
{ "schema": 1,
  "releases": [ { "version": "0.3.0",
                  "assets": [ { "name": "platctl-up-0.3.0-aarch64-apple-darwin.tar.gz",
                                "url": "platctl-up-0.3.0-aarch64-apple-darwin.tar.gz",
                                "digest": "sha256:8ca3…" } ] } ] }
```

Relative `url`s resolve against the manifest location. Generate the manifest in the release
pipeline (step 023/024), next to the archives.

## Example walkthrough

- `src/main.rs`:
  - commands `status`, `update [--check] [--yes] [--channel]`, `channel`, `releases`;
  - `VERSION` from `option_env!("PLATCTL_UP_BUILD_VERSION")`, so the demo can build several versions;
  - refuses to prompt without a TTY.
- `src/source.rs`:
  - `Source::Feed` (manifest backend) / `Source::GitHub`; shared builder settings via a macro;
  - channel filtering and pinning;
  - smoke test (`new_exe --version`);
  - package-manager detection.
- `src/channel.rs` – `Channel` (`accepts` = SemVer pre-release filter), state dir, persisted channel.
- `src/notice.rs` – throttled background check with opt-outs.
- `demo.sh`:
  - local feed served by `python3 -m http.server`;
  - notice on a TTY (`script`), throttling, exit code 10;
  - stable → beta → stable;
  - a corrupted archive is rejected (digest mismatch, binary unchanged), then 0.3.0 replaces the beta.

## Try it

```bash
examples/026-self-update-channels/demo.sh            # PORT=<free port> if 18765 is taken
cargo run -p ex026-self-update-channels -- releases --repo sharkdp/bat --channel beta   # real GitHub API
cargo run -p ex026-self-update-channels -- update --check --repo sharkdp/bat; echo $?
PLATCTL_NO_UPDATE_CHECK=1 cargo run -p ex026-self-update-channels -- status
```

## Exercises

1. Add a `verify_archive` hook that runs `cosign verify-blob` on the downloaded archive with a bundle published next to it (step 025).
2. Persist the background check's result in the state dir and show the notice on the next run when the check was too slow.
3. Add a `self-update` Cargo feature (default on); without it `update` prints how to upgrade with the package manager.
