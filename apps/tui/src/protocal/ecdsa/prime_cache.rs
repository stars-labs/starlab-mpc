//! Encrypted unused primes. Publishing never overwrites another process's set;
//! consumption atomically removes the published lease before primes can be used.
use crate::keystore::encryption::{
    KeyDerivation, decrypt_data_with_method, encrypt_data_with_method,
};
use sha2::{Digest, Sha256};
use starlab_core::ecdsa::Primes;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Cache {
    pub(super) path: PathBuf,
    // A domain-derived cache-only password; never the wallet password.
    secret: String,
    #[cfg(test)]
    pub(super) fail_directory_sync: bool,
}

#[derive(Clone)]
pub(super) struct Lease([u8; 32]);

impl Cache {
    pub(super) fn new(base: &Path, device: &str, password: &str) -> Result<Self, String> {
        // This lease protocol needs Unix directory fsync to make removal
        // durable before use. Unsupported platforms retain fresh-memory mode.
        if !cfg!(unix) {
            return Err("durable prime-cache claims require Unix directory sync".into());
        }
        if device.is_empty() || device.contains(['/', '\\']) || device == "." || device == ".." {
            return Err("invalid prime-cache device id".into());
        }
        let salt = format!("starlab/unused-paillier-primes/v1/{device}");
        let secret =
            pbkdf2::pbkdf2_hmac_array::<Sha256, 32>(password.as_bytes(), salt.as_bytes(), 100_000);
        Ok(Self {
            path: base.join(device).join("unused-ecdsa-primes.cache"),
            secret: hex::encode(secret),
            #[cfg(test)]
            fail_directory_sync: false,
        })
    }

    pub(super) fn load(&self) -> Option<(Primes, Lease)> {
        let bytes = fs::read(&self.path).ok()?;
        let plain = decrypt_data_with_method(&bytes, &self.secret, KeyDerivation::Pbkdf2).ok()?;
        let primes = serde_json::from_slice(&plain).ok()?;
        Some((primes, Lease(Sha256::digest(&bytes).into())))
    }

    /// None means these newly generated primes were never published and remain
    /// private. A hard link is an atomic create-if-absent; never rename over a lease.
    pub(super) fn publish(&self, primes: &Primes) -> Result<Option<Lease>, String> {
        let plain = serde_json::to_vec(primes).map_err(|e| e.to_string())?;
        let bytes = encrypt_data_with_method(&plain, &self.secret, KeyDerivation::Pbkdf2)
            .map_err(|e| e.to_string())?;
        let parent = self.path.parent().ok_or("cache has no parent")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = parent.join(format!(".prime-cache-{}.tmp", uuid::Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            match fs::hard_link(&temp, &self.path) {
                Ok(()) => Ok(Some(Lease(Sha256::digest(&bytes).into()))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
                Err(e) => Err(e),
            }
        })();
        let _ = fs::remove_file(temp);
        // Once published, always retain the lease even if directory sync fails:
        // treating it as private would allow another process to consume it too.
        if matches!(result, Ok(Some(_)))
            && let Err(e) = self.sync_directory(parent)
        {
            tracing::warn!("prime-cache publication directory sync failed: {e}");
        }
        result.map_err(|e| e.to_string())
    }

    fn sync_directory(&self, parent: &Path) -> std::io::Result<()> {
        #[cfg(test)]
        if self.fail_directory_sync {
            return Err(std::io::Error::other("injected directory sync failure"));
        }
        File::open(parent)?.sync_all()
    }

    pub(super) fn claim(&self, lease: &Lease) -> bool {
        let Some(parent) = self.path.parent() else {
            return false;
        };
        let claimed = parent.join(format!(".prime-cache-{}.claimed", uuid::Uuid::new_v4()));
        if fs::rename(&self.path, &claimed).is_err() {
            return false;
        }
        let matches = fs::read(&claimed)
            .is_ok_and(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)) == lease.0);
        // A mismatching replacement is discarded, never used with the old memory
        // copy. Failure to durably remove a lease also means discard the primes.
        let removed = fs::remove_file(&claimed).is_ok();
        let durable = self.sync_directory(parent).is_ok();
        matches && removed && durable
    }
}
