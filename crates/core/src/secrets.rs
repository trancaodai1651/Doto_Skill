//! Kho bí mật: token GitHub, token Cloudflare Tunnel…
//! Production dùng OS vault (macOS Keychain / Windows Credential Manager qua crate `keyring`).
//! Tuyệt đối không lưu bí mật vào SQLite, file cấu hình hay log.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub const SERVICE: &str = "com.doto.DotoSkill";

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> anyhow::Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> anyhow::Result<()>;
    fn delete(&self, key: &str) -> anyhow::Result<()>;
    fn backend_name(&self) -> &'static str;
}

pub type SharedSecrets = Arc<dyn SecretStore>;

/// OS vault thật.
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, key)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("không đọc được OS vault: {e}")),
        }
    }
    fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        keyring::Entry::new(SERVICE, key)?.set_password(value).map_err(|e| anyhow::anyhow!("không ghi được OS vault: {e}"))
    }
    fn delete(&self, key: &str) -> anyhow::Result<()> {
        match keyring::Entry::new(SERVICE, key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("không xoá được khỏi OS vault: {e}")),
        }
    }
    fn backend_name(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macOS Keychain"
        } else if cfg!(windows) {
            "Windows Credential Manager"
        } else {
            "OS keyring"
        }
    }
}

/// Kho trong bộ nhớ — chỉ cho test/CI (mất khi thoát tiến trình).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> anyhow::Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    fn backend_name(&self) -> &'static str {
        "memory (test)"
    }
}

/// Chọn backend: `DOTO_SECRETS=memory` cho CI/E2E, mặc định OS vault.
pub fn default_store() -> SharedSecrets {
    if std::env::var("DOTO_SECRETS").as_deref() == Ok("memory") {
        Arc::new(MemoryStore::default())
    } else {
        Arc::new(KeyringStore)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_crud() {
        let s = MemoryStore::default();
        assert!(s.get("a").unwrap().is_none());
        s.set("a", "1").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("1"));
        s.delete("a").unwrap();
        assert!(s.get("a").unwrap().is_none());
    }
}
