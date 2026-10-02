# 012 – Secrets: keyring, encryption, secrecy

## Goal

Handle credentials in a CLI safely: get them in without leaking (prompts, stdin, env), keep them
between runs in the right store, and never print or log them by accident.

Example: `examples/012-secrets-keyring` – `platctl-auth login|token|status|logout` with
`--store keyring|file`.

## Key concepts

### Getting a secret in

| Way | Verdict |
|-----|---------|
| prompt without echo (`rpassword`, dialoguer `Password`) | interactive users |
| `--password-stdin` (`echo "$PW" \| tool login --password-stdin`) | CI/scripts – docker, gh, helm do this |
| env var (`PLATCTL_TOKEN`) | CI secrets; visible to the process tree, fine for short-lived jobs |
| flag `--token X` | **avoid**: visible in `ps`, shell history, CI logs – warn if used |
| config file | **never** – files get committed and shared |
| OAuth device flow / browser login | best: the CLI never sees the password (`gh auth login`, `az login`) |

Refuse to prompt without a terminal (`stderr().is_terminal()`) – a hanging CI job is worse than a clear error.

### Keeping a secret – credential stores

```rust
// keyring – OS keychain: macOS Keychain, Windows Credential Manager, Linux Secret Service
let entry = keyring::Entry::new("platctl-training", "default")?;
entry.set_password(token.expose_secret())?;
match entry.get_password() { Ok(t) => ..., Err(keyring::Error::NoEntry) => not logged in, ... }
entry.delete_credential()?;
```

- encrypted by the OS, unlocked with the user's login, visible/revocable in OS UI,
- not available in containers/headless Linux without a Secret Service daemon → fallback needed,
- keyring 4: `v1` feature = simple `Entry` API; for full control use `keyring-core` + chosen stores.

```rust
// age – passphrase-encrypted file (scrypt + ChaCha20-Poly1305), fallback for headless systems
let ciphertext = age::encrypt(&age::scrypt::Recipient::new(passphrase.clone()), plaintext)?;
let plaintext  = age::decrypt(&age::scrypt::Identity::new(passphrase), &ciphertext)?;   // fails on wrong passphrase / tampering
```

- create the file with mode `0600` **before** writing (`OpenOptionsExt::mode`),
- alternatives: `chacha20poly1305` + `argon2` (lower level), a `0600` plain file (common, weakest),
- store location: `$XDG_CONFIG_HOME/platctl/` / `%APPDATA%` (here a local dir for the demo).

A `CredentialStore` trait lets commands ignore where the token lives.

### Keeping secrets out of output and logs – `secrecy`

```rust
let token: SecretString = ...;
println!("{token:?}");                 // SecretBox<str>([REDACTED])
token.expose_secret()                  // explicit, easy to grep for in code review
```

- memory is zeroed on drop (`zeroize`),
- clap: `Option<SecretString>` works as an argument type; `hide_env_values = true` keeps the env
  value out of `--help`,
- show fingerprints, not secrets: `token ****3d6e`,
- print the secret only on explicit request to stdout (`platctl-auth token`) for `$(...)` use,
- exit 77 (EX_NOPERM) when not logged in – scripts can detect it.

### Precedence

`--token` / `PLATCTL_TOKEN` (per invocation, CI) > stored credential (interactive login) – the same
"more specific wins" rule as step 005.

## Example walkthrough

- `src/store.rs` – `CredentialStore` trait, `KeyringStore`, `EncryptedFileStore` (age, `0600`);
  tests: round trip, wrong passphrase, no plaintext on disk, file permissions.
- `src/main.rs` – login (prompt / stdin), token resolution with precedence and warnings,
  fingerprint in `status`, TTY check before prompting, simulated token exchange.

## Try it

```bash
cargo test -p ex012-secrets-keyring
cargo build -p ex012-secrets-keyring && P=target/debug/platctl-auth
$P login                                         # prompt without echo, stored in the OS keychain
$P status; $P token; $P logout
export PLATCTL_PASSPHRASE=demo-passphrase
echo "s3cret-password" | $P --store file login --password-stdin
ls -l .platctl-credentials; cat .platctl-credentials/default.age | head -c 80
PLATCTL_TOKEN=plt_from_env $P --store file status   # env beats stored
$P --store file --token plt_flag status             # warning
PLATCTL_PASSPHRASE=nope $P --store file status      # decryption fails
$P --store file login < /dev/null                    # no terminal: clear error
```

## Exercises

1. Fall back automatically from keyring to the encrypted file when the keychain is unavailable (e.g. in Docker).
2. Store the token with an expiry and refresh it transparently before it expires.
3. Add `platctl-auth login --web`: print a device code + URL (OAuth 2.0 Device Authorization Grant, RFC 8628) and poll for the token (simulated).
