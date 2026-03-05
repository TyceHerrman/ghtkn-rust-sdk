//! Keyring integration for token caching.
//!
//! Provides secure storage and retrieval of GitHub access tokens using the
//! system keyring. Uses the same service key, user key, and JSON schema as
//! the Go SDK for cross-tool compatibility.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// The service key used to store tokens in the system keyring.
/// Must match the Go SDK exactly for cross-tool compatibility.
pub const DEFAULT_SERVICE_KEY: &str = "github.com/suzuki-shunsuke/ghtkn";

/// A cached GitHub access token with metadata.
///
/// Serialized as JSON and stored in the system keyring.
/// The JSON schema matches the Go SDK exactly:
/// ```json
/// {
///   "access_token": "ghu_...",
///   "expiration_date": "2024-01-01T00:00:00Z",
///   "login": "username"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessToken {
    pub access_token: String,
    pub expiration_date: DateTime<Utc>,
    pub login: String,
}

/// Abstraction over the system keyring for testability.
///
/// The real implementation uses the `keyring` crate. Tests can provide
/// a mock backend (e.g. backed by a `HashMap`).
pub trait KeyringBackend: Send + Sync {
    /// Retrieve a password from the keyring.
    ///
    /// Returns `Ok(Some(password))` if found, `Ok(None)` if the entry does
    /// not exist, or `Err` on a real failure.
    fn get(&self, service: &str, user: &str) -> crate::Result<Option<String>>;

    /// Store a password in the keyring.
    fn set(&self, service: &str, user: &str, password: &str) -> crate::Result<()>;
}

/// System keyring backend using the `keyring` crate.
pub struct SystemBackend;

impl KeyringBackend for SystemBackend {
    fn get(&self, service: &str, user: &str) -> crate::Result<Option<String>> {
        let entry = keyring::Entry::new(service, user)
            .map_err(|e| Error::Keyring(format!("create keyring entry: {e}")))?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Keyring(format!("get password from keyring: {e}"))),
        }
    }

    fn set(&self, service: &str, user: &str, password: &str) -> crate::Result<()> {
        let entry = keyring::Entry::new(service, user)
            .map_err(|e| Error::Keyring(format!("create keyring entry: {e}")))?;
        entry
            .set_password(password)
            .map_err(|e| Error::Keyring(format!("set password in keyring: {e}")))
    }
}

/// High-level keyring wrapper that handles JSON serialization and validation.
pub struct Keyring {
    backend: Box<dyn KeyringBackend>,
}

impl Keyring {
    /// Create a new `Keyring` using the system keyring backend.
    pub fn new() -> Self {
        Self {
            backend: Box::new(SystemBackend),
        }
    }

    /// Create a new `Keyring` with a custom backend (for testing).
    pub fn with_backend(backend: Box<dyn KeyringBackend>) -> Self {
        Self { backend }
    }

    /// Retrieve and validate a cached access token.
    ///
    /// Returns `Ok(None)` if no entry exists for the given service and key.
    /// Returns an error if the stored JSON is invalid or any required field
    /// is empty.
    pub fn get(&self, service: &str, key: &str) -> crate::Result<Option<AccessToken>> {
        let json = match self.backend.get(service, key)? {
            Some(s) => s,
            None => return Ok(None),
        };

        let token: AccessToken = serde_json::from_str(&json)
            .map_err(|e| Error::Keyring(format!("decode keyring value as JSON: {e}")))?;

        if token.access_token.is_empty() {
            return Err(Error::Keyring("access_token is required".into()));
        }
        if token.login.is_empty() {
            return Err(Error::Keyring("login is required".into()));
        }
        if token.expiration_date == DateTime::<Utc>::default() {
            return Err(Error::Keyring("expiration_date is required".into()));
        }

        Ok(Some(token))
    }

    /// Store an access token in the keyring as JSON.
    pub fn set(&self, service: &str, key: &str, token: &AccessToken) -> crate::Result<()> {
        let json = serde_json::to_string(token)
            .map_err(|e| Error::Keyring(format!("encode token as JSON: {e}")))?;
        self.backend.set(service, key, &json)
    }
}

impl Default for Keyring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use chrono::TimeZone;

    use super::*;

    // ---------------------------------------------------------------
    // Mock backend
    // ---------------------------------------------------------------

    struct MockBackend {
        store: Mutex<HashMap<(String, String), String>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
    }

    impl KeyringBackend for MockBackend {
        fn get(&self, service: &str, user: &str) -> crate::Result<Option<String>> {
            let store = self.store.lock().unwrap();
            Ok(store.get(&(service.to_string(), user.to_string())).cloned())
        }

        fn set(&self, service: &str, user: &str, password: &str) -> crate::Result<()> {
            let mut store = self.store.lock().unwrap();
            store.insert(
                (service.to_string(), user.to_string()),
                password.to_string(),
            );
            Ok(())
        }
    }

    fn make_keyring() -> Keyring {
        Keyring::with_backend(Box::new(MockBackend::new()))
    }

    fn sample_token() -> AccessToken {
        AccessToken {
            access_token: "ghu_test123".into(),
            expiration_date: Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap(),
            login: "testuser".into(),
        }
    }

    // ---------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------

    #[test]
    fn default_service_key_value() {
        assert_eq!(DEFAULT_SERVICE_KEY, "github.com/suzuki-shunsuke/ghtkn");
    }

    #[test]
    fn get_returns_none_when_not_found() {
        let kr = make_keyring();
        let result = kr.get(DEFAULT_SERVICE_KEY, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_returns_token_when_valid() {
        let kr = make_keyring();
        let token = sample_token();
        kr.set(DEFAULT_SERVICE_KEY, "client1", &token).unwrap();

        let got = kr.get(DEFAULT_SERVICE_KEY, "client1").unwrap().unwrap();
        assert_eq!(got, token);
    }

    #[test]
    fn get_returns_error_for_invalid_json() {
        let kr = make_keyring();
        // Manually insert invalid JSON via the backend.
        kr.backend
            .set(DEFAULT_SERVICE_KEY, "bad", "not-json")
            .unwrap();

        let result = kr.get(DEFAULT_SERVICE_KEY, "bad");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("decode keyring value as JSON"),
            "unexpected error: {err_msg}"
        );
    }

    #[test]
    fn get_returns_error_for_empty_access_token() {
        let kr = make_keyring();
        let json = r#"{"access_token":"","expiration_date":"2025-06-15T12:00:00Z","login":"user"}"#;
        kr.backend
            .set(DEFAULT_SERVICE_KEY, "empty_at", json)
            .unwrap();

        let result = kr.get(DEFAULT_SERVICE_KEY, "empty_at");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("access_token is required"),
            "unexpected error: {err_msg}"
        );
    }

    #[test]
    fn get_returns_error_for_empty_login() {
        let kr = make_keyring();
        let json =
            r#"{"access_token":"ghu_abc","expiration_date":"2025-06-15T12:00:00Z","login":""}"#;
        kr.backend
            .set(DEFAULT_SERVICE_KEY, "empty_login", json)
            .unwrap();

        let result = kr.get(DEFAULT_SERVICE_KEY, "empty_login");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("login is required"),
            "unexpected error: {err_msg}"
        );
    }

    #[test]
    fn set_stores_valid_json() {
        let kr = make_keyring();
        let token = sample_token();
        kr.set(DEFAULT_SERVICE_KEY, "client1", &token).unwrap();

        // Read back the raw JSON from the backend.
        let raw = kr
            .backend
            .get(DEFAULT_SERVICE_KEY, "client1")
            .unwrap()
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["access_token"], "ghu_test123");
        assert_eq!(parsed["login"], "testuser");
        // Verify RFC3339 format.
        assert!(parsed["expiration_date"].as_str().unwrap().contains('T'));
    }

    #[test]
    fn roundtrip_set_then_get() {
        let kr = make_keyring();
        let token = sample_token();

        kr.set(DEFAULT_SERVICE_KEY, "rt_client", &token).unwrap();
        let got = kr.get(DEFAULT_SERVICE_KEY, "rt_client").unwrap().unwrap();

        assert_eq!(got.access_token, token.access_token);
        assert_eq!(got.expiration_date, token.expiration_date);
        assert_eq!(got.login, token.login);
    }

    #[test]
    fn get_returns_error_for_missing_fields() {
        let kr = make_keyring();
        // JSON missing the login field entirely.
        let json = r#"{"access_token":"ghu_abc","expiration_date":"2025-06-15T12:00:00Z"}"#;
        kr.backend
            .set(DEFAULT_SERVICE_KEY, "missing", json)
            .unwrap();

        let result = kr.get(DEFAULT_SERVICE_KEY, "missing");
        assert!(result.is_err());
    }

    #[test]
    fn get_returns_error_for_invalid_date() {
        let kr = make_keyring();
        let json = r#"{"access_token":"ghu_abc","expiration_date":"not-a-date","login":"user"}"#;
        kr.backend
            .set(DEFAULT_SERVICE_KEY, "bad_date", json)
            .unwrap();

        let result = kr.get(DEFAULT_SERVICE_KEY, "bad_date");
        assert!(result.is_err());
    }

    #[test]
    fn different_keys_are_independent() {
        let kr = make_keyring();
        let token1 = AccessToken {
            access_token: "token_a".into(),
            expiration_date: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
            login: "user_a".into(),
        };
        let token2 = AccessToken {
            access_token: "token_b".into(),
            expiration_date: Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap(),
            login: "user_b".into(),
        };

        kr.set(DEFAULT_SERVICE_KEY, "key1", &token1).unwrap();
        kr.set(DEFAULT_SERVICE_KEY, "key2", &token2).unwrap();

        let got1 = kr.get(DEFAULT_SERVICE_KEY, "key1").unwrap().unwrap();
        let got2 = kr.get(DEFAULT_SERVICE_KEY, "key2").unwrap().unwrap();

        assert_eq!(got1.login, "user_a");
        assert_eq!(got2.login, "user_b");
    }
}
