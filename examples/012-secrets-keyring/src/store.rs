//! Credential stores: where a CLI keeps a long-lived secret (API token) between runs.
//!
//! | Store | When |
//! |-------|------|
//! | OS keychain (`keyring`) | desktops: macOS Keychain, Windows Credential Manager, Linux Secret Service |
//! | encrypted file (`age`)  | headless servers, containers, WSL – no keychain daemon available |
//! | plain file `0600`       | what many tools do (`~/.netrc`, `~/.docker/config.json`) – last resort |
//!
//! Never store secrets in the regular config file – it gets committed, shared, pasted.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::Context;
use secrecy::{ExposeSecret, SecretString};

/// Abstraction over the storage backend – commands don't care where the token lives.
pub trait CredentialStore {
    fn name(&self) -> String;
    fn save(&self, account: &str, secret: &SecretString) -> anyhow::Result<()>;
    /// `Ok(None)` – nothing stored for this account.
    fn load(&self, account: &str) -> anyhow::Result<Option<SecretString>>;
    fn delete(&self, account: &str) -> anyhow::Result<()>;
}

/// OS keychain via `keyring` (v1 API: `Entry::new(service, user)`).
/// The OS encrypts the secret, protects it with the user's login, and may show it in its UI
/// (Keychain Access, Credential Manager) – users can inspect and revoke it.
pub struct KeyringStore {
    /// "Service" name under which entries are grouped in the keychain.
    pub service: String,
}

impl CredentialStore for KeyringStore {
    fn name(&self) -> String {
        format!("OS keychain (service `{}`)", self.service)
    }

    fn save(&self, account: &str, secret: &SecretString) -> anyhow::Result<()> {
        keyring::Entry::new(&self.service, account)?
            .set_password(secret.expose_secret())
            .context("cannot write to the OS keychain")
    }

    fn load(&self, account: &str) -> anyhow::Result<Option<SecretString>> {
        match keyring::Entry::new(&self.service, account)?.get_password() {
            Ok(password) => Ok(Some(SecretString::from(password))),
            // "no entry" is not an error for us – the user just isn't logged in.
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("cannot read from the OS keychain"),
        }
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        match keyring::Entry::new(&self.service, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error).context("cannot delete from the OS keychain"),
        }
    }
}

/// File encrypted with a passphrase using `age` (modern replacement for GPG-encrypted files;
/// scrypt key derivation + ChaCha20-Poly1305). One file per account.
pub struct EncryptedFileStore {
    pub dir: PathBuf,
    /// Passphrase protecting the files – from a prompt or `PLATCTL_PASSPHRASE` (CI).
    pub passphrase: age::secrecy::SecretString,
}

impl EncryptedFileStore {
    fn path(&self, account: &str) -> PathBuf {
        self.dir.join(format!("{account}.age"))
    }
}

impl CredentialStore for EncryptedFileStore {
    fn name(&self) -> String {
        format!("encrypted files in {}", self.dir.display())
    }

    fn save(&self, account: &str, secret: &SecretString) -> anyhow::Result<()> {
        let recipient = age::scrypt::Recipient::new(self.passphrase.clone());
        let ciphertext = age::encrypt(&recipient, secret.expose_secret().as_bytes())
            .context("encryption failed")?;
        fs::create_dir_all(&self.dir)?;
        write_private(&self.path(account), &ciphertext)
    }

    fn load(&self, account: &str) -> anyhow::Result<Option<SecretString>> {
        let path = self.path(account);
        let ciphertext = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let identity = age::scrypt::Identity::new(self.passphrase.clone());
        // Wrong passphrase or tampered file -> authenticated decryption fails.
        let plaintext = age::decrypt(&identity, &ciphertext)
            .context("cannot decrypt credentials (wrong passphrase?)")?;
        let text = String::from_utf8(plaintext).context("stored credential is not UTF-8")?;
        Ok(Some(SecretString::from(text)))
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        match fs::remove_file(self.path(account)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// Creates the file readable/writable only by the owner (0600 on Unix) – BEFORE writing data,
/// so the secret never exists in a world-readable file.
fn write_private(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    file.write_all(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path, passphrase: &str) -> EncryptedFileStore {
        EncryptedFileStore {
            dir: dir.to_path_buf(),
            passphrase: age::secrecy::SecretString::from(passphrase.to_string()),
        }
    }

    #[test]
    fn encrypted_file_round_trip_and_wrong_passphrase() {
        let dir = std::env::temp_dir().join(format!("platctl-auth-test-{}", std::process::id()));
        let good = store(&dir, "correct horse battery staple");

        good.save("alice", &SecretString::from("plt_secret"))
            .unwrap();
        let raw = fs::read(dir.join("alice.age")).unwrap();
        assert!(
            !raw.windows(10).any(|w| w == b"plt_secret"),
            "stored encrypted"
        );
        assert_eq!(
            good.load("alice").unwrap().unwrap().expose_secret(),
            "plt_secret"
        );
        assert!(store(&dir, "wrong").load("alice").is_err());

        good.delete("alice").unwrap();
        assert!(good.load("alice").unwrap().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn credential_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("platctl-auth-perm-{}", std::process::id()));
        store(&dir, "pass")
            .save("bob", &SecretString::from("x"))
            .unwrap();

        let mode = fs::metadata(dir.join("bob.age"))
            .unwrap()
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, 0o600);
        let _ = fs::remove_dir_all(dir);
    }
}
