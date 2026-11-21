use zeroize::{Zeroize, Zeroizing};

pub const KEYRING_SERVICE: &str = "io.krono.Krono";
pub const KEYRING_USER: &str = "github_token";
pub const KEYRING_TMDB_USER: &str = "tmdb_api_key";

pub trait SecretBackend {
    fn get(&self) -> Result<Option<Zeroizing<String>>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

pub struct SystemKeyring;

impl SystemKeyring {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| format!("keyring init error: {e}"))
    }
}

impl SecretBackend for SystemKeyring {
    fn get(&self) -> Result<Option<Zeroizing<String>>, String> {
        let entry = Self::entry()?;
        match entry.get_password() {
            Ok(secret) => {
                if secret.trim().is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(Zeroizing::new(secret)))
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keyring read error: {e}")),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        let trimmed = secret.trim();
        if trimmed.is_empty() {
            return Err("token cannot be empty".to_string());
        }
        let entry = Self::entry()?;
        entry
            .set_password(trimmed)
            .map_err(|e| format!("keyring write error: {e}"))
    }

    fn delete(&self) -> Result<(), String> {
        let entry = Self::entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keyring delete error: {e}")),
        }
    }
}

pub fn load_github_token() -> Result<Option<Zeroizing<String>>, String> {
    SystemKeyring.get()
}

pub fn save_github_token(token: &str) -> Result<(), String> {
    SystemKeyring.set(token)
}

pub fn delete_github_token() -> Result<(), String> {
    SystemKeyring.delete()
}

pub fn has_github_token() -> bool {
    matches!(load_github_token(), Ok(Some(_)))
}

pub struct TmdbKeyring;

impl TmdbKeyring {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_TMDB_USER)
            .map_err(|e| format!("keyring init error: {e}"))
    }
}

impl SecretBackend for TmdbKeyring {
    fn get(&self) -> Result<Option<Zeroizing<String>>, String> {
        let entry = Self::entry()?;
        match entry.get_password() {
            Ok(secret) => {
                if secret.trim().is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(Zeroizing::new(secret)))
                }
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("keyring read error: {e}")),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        let trimmed = secret.trim();
        if trimmed.is_empty() {
            return Err("token cannot be empty".to_string());
        }
        let entry = Self::entry()?;
        entry
            .set_password(trimmed)
            .map_err(|e| format!("keyring write error: {e}"))
    }

    fn delete(&self) -> Result<(), String> {
        let entry = Self::entry()?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("keyring delete error: {e}")),
        }
    }
}

pub fn load_tmdb_token() -> Result<Option<Zeroizing<String>>, String> {
    TmdbKeyring.get()
}

pub fn save_tmdb_token(token: &str) -> Result<(), String> {
    TmdbKeyring.set(token)
}

pub fn delete_tmdb_token() -> Result<(), String> {
    TmdbKeyring.delete()
}

pub fn has_tmdb_token() -> bool {
    matches!(load_tmdb_token(), Ok(Some(_)))
}

/// Moves a non-empty legacy plaintext token into `backend` and wipes `legacy_token`
/// ONLY after `backend.set` returns `Ok(())`.
pub fn migrate_legacy_token_with<B: SecretBackend>(
    legacy_token: &mut String,
    backend: &B,
) -> Result<bool, String> {
    let trimmed = legacy_token.trim().to_string();
    if trimmed.is_empty() {
        legacy_token.zeroize();
        legacy_token.clear();
        return Ok(false);
    }
    let wrapped = Zeroizing::new(trimmed);
    backend.set(wrapped.as_str())?;
    legacy_token.zeroize();
    legacy_token.clear();
    Ok(true)
}

pub fn migrate_legacy_token(legacy_token: &mut String) -> Result<bool, String> {
    migrate_legacy_token_with(legacy_token, &SystemKeyring)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockBackend {
        stored: RefCell<Option<String>>,
        fail_set: bool,
    }

    impl SecretBackend for MockBackend {
        fn get(&self) -> Result<Option<Zeroizing<String>>, String> {
            Ok(self.stored.borrow().clone().map(Zeroizing::new))
        }

        fn set(&self, secret: &str) -> Result<(), String> {
            if self.fail_set {
                return Err("mock keyring unavailable".to_string());
            }
            *self.stored.borrow_mut() = Some(secret.to_string());
            Ok(())
        }

        fn delete(&self) -> Result<(), String> {
            *self.stored.borrow_mut() = None;
            Ok(())
        }
    }

    #[test]
    fn migration_wipes_legacy_token_only_on_success() {
        let backend = MockBackend {
            stored: RefCell::new(None),
            fail_set: false,
        };
        let mut legacy = "  ghp_secret_123  ".to_string();
        let migrated = migrate_legacy_token_with(&mut legacy, &backend).unwrap();
        assert!(migrated);
        assert!(legacy.is_empty());
        assert_eq!(backend.get().unwrap().as_deref().map(String::as_str), Some("ghp_secret_123"));
    }

    #[test]
    fn migration_preserves_legacy_token_when_keyring_fails() {
        let backend = MockBackend {
            stored: RefCell::new(None),
            fail_set: true,
        };
        let mut legacy = "ghp_secret_123".to_string();
        let res = migrate_legacy_token_with(&mut legacy, &backend);
        assert!(res.is_err());
        assert_eq!(legacy, "ghp_secret_123");
        assert!(backend.get().unwrap().is_none());
    }

    #[test]
    fn migration_skips_empty_token_without_writing() {
        let backend = MockBackend {
            stored: RefCell::new(None),
            fail_set: true,
        };
        let mut legacy = "   ".to_string();
        let migrated = migrate_legacy_token_with(&mut legacy, &backend).unwrap();
        assert!(!migrated);
        assert!(legacy.is_empty());
    }

    #[test]
    fn tmdb_token_save_load_delete_via_backend() {
        let backend = MockBackend {
            stored: RefCell::new(None),
            fail_set: false,
        };
        backend.set("tmdb_v4_key").unwrap();
        assert_eq!(
            backend.get().unwrap().as_deref().map(String::as_str),
            Some("tmdb_v4_key")
        );
        backend.delete().unwrap();
        assert!(backend.get().unwrap().is_none());
    }
}
