//! Integration tests for the ghtkn SDK.
//!
//! These tests verify cross-module behavior with mocked dependencies.
//! No network access or real system keyring is required.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use pretty_assertions::assert_eq;

use ghtkn::config::{App, Config};
use ghtkn::keyring::{AccessToken, KeyringBackend, DEFAULT_SERVICE_KEY};

// ---------------------------------------------------------------------------
// Mock keyring backend (same pattern as unit tests, but accessible here)
// ---------------------------------------------------------------------------

struct MockBackend {
    store: Mutex<HashMap<(String, String), String>>,
}

impl MockBackend {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }

    /// Pre-populate the mock keyring with a raw JSON string.
    fn insert(&self, service: &str, user: &str, json: &str) {
        let mut store = self.store.lock().unwrap();
        store.insert((service.to_string(), user.to_string()), json.to_string());
    }
}

impl KeyringBackend for MockBackend {
    fn get(&self, service: &str, user: &str) -> ghtkn::Result<Option<String>> {
        let store = self.store.lock().unwrap();
        Ok(store.get(&(service.to_string(), user.to_string())).cloned())
    }

    fn set(&self, service: &str, user: &str, password: &str) -> ghtkn::Result<()> {
        let mut store = self.store.lock().unwrap();
        store.insert(
            (service.to_string(), user.to_string()),
            password.to_string(),
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Test 1: Config loading -> app selection -> keyring roundtrip
// ---------------------------------------------------------------------------

#[test]
fn config_load_select_keyring_roundtrip() {
    // 1. Create a temp config file.
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("ghtkn.yaml");
    std::fs::write(
        &config_path,
        "apps:\n  - name: my-app\n    client_id: Iv1.abc123\n    git_owner: myorg\n",
    )
    .unwrap();

    // 2. Read and validate the config.
    let cfg = ghtkn::config::read(&config_path).unwrap().unwrap();
    cfg.validate().unwrap();
    assert_eq!(cfg.apps.len(), 1);

    // 3. Select the app.
    let app = ghtkn::config::select_app(&cfg, "", "myorg").unwrap();
    assert_eq!(app.name, "my-app");
    assert_eq!(app.client_id, "Iv1.abc123");

    // 4. Store a token in a mock keyring using the app's client_id as key.
    let backend = MockBackend::new();
    let keyring = ghtkn::keyring::Keyring::with_backend(Box::new(backend));

    let token = AccessToken {
        access_token: "ghu_roundtrip_token".into(),
        expiration_date: Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap(),
        login: "testuser".into(),
    };
    keyring
        .set(DEFAULT_SERVICE_KEY, &app.client_id, &token)
        .unwrap();

    // 5. Read the token back from the keyring.
    let got = keyring
        .get(DEFAULT_SERVICE_KEY, &app.client_id)
        .unwrap()
        .unwrap();
    assert_eq!(got.access_token, "ghu_roundtrip_token");
    assert_eq!(got.login, "testuser");
    assert_eq!(
        got.expiration_date,
        Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Test 2: Token expiration threshold logic
// ---------------------------------------------------------------------------

#[test]
fn token_expiration_threshold_logic() {
    let backend = MockBackend::new();
    let keyring = ghtkn::keyring::Keyring::with_backend(Box::new(backend));

    // Store a token that expires in 10 minutes from now.
    let expiration = Utc::now() + chrono::Duration::minutes(10);
    let token = AccessToken {
        access_token: "ghu_threshold_test".into(),
        expiration_date: expiration,
        login: "testuser".into(),
    };
    keyring
        .set(DEFAULT_SERVICE_KEY, "threshold-app", &token)
        .unwrap();

    // Read the token back.
    let got = keyring
        .get(DEFAULT_SERVICE_KEY, "threshold-app")
        .unwrap()
        .unwrap();

    // With a 5-minute threshold, the token should still be valid
    // (10 min remaining > 5 min threshold).
    let five_min = Duration::from_secs(5 * 60);
    let min_exp_5 = chrono::Duration::from_std(five_min).unwrap_or(chrono::Duration::zero());
    let expired_5 = Utc::now() + min_exp_5 > got.expiration_date;
    assert!(
        !expired_5,
        "token with 10 min remaining should be valid with 5 min threshold"
    );

    // With a 15-minute threshold, the token should be considered expired
    // (10 min remaining < 15 min threshold).
    let fifteen_min = Duration::from_secs(15 * 60);
    let min_exp_15 = chrono::Duration::from_std(fifteen_min).unwrap_or(chrono::Duration::zero());
    let expired_15 = Utc::now() + min_exp_15 > got.expiration_date;
    assert!(
        expired_15,
        "token with 10 min remaining should be expired with 15 min threshold"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Config validation catches all error cases
// ---------------------------------------------------------------------------

#[test]
fn config_validation_catches_empty_apps() {
    let cfg = Config { apps: vec![] };
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("apps is required"),
        "unexpected error: {err}"
    );
}

#[test]
fn config_validation_catches_empty_name() {
    let cfg = Config {
        apps: vec![App {
            name: String::new(),
            client_id: "xxx".into(),
            git_owner: String::new(),
        }],
    };
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("name is required"),
        "unexpected error: {err}"
    );
}

#[test]
fn config_validation_catches_empty_client_id() {
    let cfg = Config {
        apps: vec![App {
            name: "app".into(),
            client_id: String::new(),
            git_owner: String::new(),
        }],
    };
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("client_id is required"),
        "unexpected error: {err}"
    );
}

#[test]
fn config_validation_catches_duplicate_names() {
    let cfg = Config {
        apps: vec![
            App {
                name: "dup".into(),
                client_id: "xxx".into(),
                git_owner: String::new(),
            },
            App {
                name: "dup".into(),
                client_id: "yyy".into(),
                git_owner: String::new(),
            },
        ],
    };
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("app name must be unique"),
        "unexpected error: {err}"
    );
}

#[test]
fn config_validation_catches_duplicate_git_owners() {
    let cfg = Config {
        apps: vec![
            App {
                name: "app1".into(),
                client_id: "xxx".into(),
                git_owner: "same-owner".into(),
            },
            App {
                name: "app2".into(),
                client_id: "yyy".into(),
                git_owner: "same-owner".into(),
            },
        ],
    };
    let err = cfg.validate().unwrap_err();
    assert!(
        err.to_string().contains("app git_owner must be unique"),
        "unexpected error: {err}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Keyring JSON compatibility with Go SDK
// ---------------------------------------------------------------------------

#[test]
fn keyring_json_compatible_with_go_sdk() {
    // This is the exact JSON format the Go SDK stores in the keyring.
    let go_sdk_json = r#"{"access_token":"ghu_abc123","expiration_date":"2025-06-15T12:00:00Z","login":"testuser"}"#;

    // Set up a mock keyring pre-populated with the Go SDK JSON.
    let backend = MockBackend::new();
    backend.insert(DEFAULT_SERVICE_KEY, "Iv1.go_client", go_sdk_json);
    let keyring = ghtkn::keyring::Keyring::with_backend(Box::new(backend));

    // The Rust SDK should be able to parse it correctly.
    let token = keyring
        .get(DEFAULT_SERVICE_KEY, "Iv1.go_client")
        .unwrap()
        .unwrap();

    assert_eq!(token.access_token, "ghu_abc123");
    assert_eq!(token.login, "testuser");
    assert_eq!(
        token.expiration_date,
        Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
    );
}

#[test]
fn keyring_json_roundtrip_produces_go_compatible_format() {
    // Store a token via the Rust SDK and verify the JSON matches Go SDK format.
    let backend = MockBackend::new();
    let keyring = ghtkn::keyring::Keyring::with_backend(Box::new(backend));

    let token = AccessToken {
        access_token: "ghu_rust_token".into(),
        expiration_date: Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap(),
        login: "rustuser".into(),
    };
    keyring
        .set(DEFAULT_SERVICE_KEY, "Iv1.rust", &token)
        .unwrap();

    // Read back the raw JSON and verify field names match the Go SDK.
    let got = keyring
        .get(DEFAULT_SERVICE_KEY, "Iv1.rust")
        .unwrap()
        .unwrap();
    assert_eq!(got.access_token, "ghu_rust_token");
    assert_eq!(got.login, "rustuser");
    assert_eq!(
        got.expiration_date,
        Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
    );

    // Verify the underlying JSON uses the exact Go SDK field names.
    // We re-serialize and check.
    let json = serde_json::to_string(&got).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("access_token").is_some(),
        "missing access_token field"
    );
    assert!(
        parsed.get("expiration_date").is_some(),
        "missing expiration_date field"
    );
    assert!(parsed.get("login").is_some(), "missing login field");
    // Verify no extra fields.
    assert_eq!(
        parsed.as_object().unwrap().len(),
        3,
        "expected exactly 3 fields in serialized JSON"
    );
}

#[test]
fn keyring_go_sdk_json_with_subsecond_precision() {
    // The Go SDK may store timestamps with or without fractional seconds.
    // Verify we handle both.
    let json_with_nanos = r#"{"access_token":"ghu_nano","expiration_date":"2025-06-15T12:00:00.123456789Z","login":"nanouser"}"#;

    let backend = MockBackend::new();
    backend.insert(DEFAULT_SERVICE_KEY, "Iv1.nano", json_with_nanos);
    let keyring = ghtkn::keyring::Keyring::with_backend(Box::new(backend));

    let token = keyring
        .get(DEFAULT_SERVICE_KEY, "Iv1.nano")
        .unwrap()
        .unwrap();
    assert_eq!(token.access_token, "ghu_nano");
    assert_eq!(token.login, "nanouser");
}

// ---------------------------------------------------------------------------
// Test 5: App selection priority matches Go SDK
// ---------------------------------------------------------------------------

#[test]
fn app_selection_priority_owner_first() {
    let cfg = Config {
        apps: vec![
            App {
                name: "default-app".into(),
                client_id: "cid_default".into(),
                git_owner: "default-org".into(),
            },
            App {
                name: "owner-app".into(),
                client_id: "cid_owner".into(),
                git_owner: "target-org".into(),
            },
            App {
                name: "named-app".into(),
                client_id: "cid_named".into(),
                git_owner: String::new(),
            },
        ],
    };
    cfg.validate().unwrap();

    // Priority 1: owner match takes precedence over everything.
    let app = ghtkn::config::select_app(&cfg, "named-app", "target-org").unwrap();
    assert_eq!(app.name, "owner-app", "owner match should take priority");
}

#[test]
fn app_selection_priority_default_when_no_key() {
    let cfg = Config {
        apps: vec![
            App {
                name: "first-app".into(),
                client_id: "cid_first".into(),
                git_owner: String::new(),
            },
            App {
                name: "second-app".into(),
                client_id: "cid_second".into(),
                git_owner: String::new(),
            },
        ],
    };
    cfg.validate().unwrap();

    // Priority 2: empty key and empty owner returns first app.
    let app = ghtkn::config::select_app(&cfg, "", "").unwrap();
    assert_eq!(
        app.name, "first-app",
        "empty key/owner should return first app"
    );
}

#[test]
fn app_selection_priority_name_match() {
    let cfg = Config {
        apps: vec![
            App {
                name: "first-app".into(),
                client_id: "cid_first".into(),
                git_owner: String::new(),
            },
            App {
                name: "target-app".into(),
                client_id: "cid_target".into(),
                git_owner: String::new(),
            },
        ],
    };
    cfg.validate().unwrap();

    // Priority 3: name match.
    let app = ghtkn::config::select_app(&cfg, "target-app", "").unwrap();
    assert_eq!(app.name, "target-app", "name match should work");
}

#[test]
fn app_selection_owner_miss_falls_through() {
    let cfg = Config {
        apps: vec![
            App {
                name: "first-app".into(),
                client_id: "cid_first".into(),
                git_owner: "org1".into(),
            },
            App {
                name: "second-app".into(),
                client_id: "cid_second".into(),
                git_owner: "org2".into(),
            },
        ],
    };
    cfg.validate().unwrap();

    // Owner miss with empty key falls through to default (first app).
    let app = ghtkn::config::select_app(&cfg, "", "nonexistent-org").unwrap();
    assert_eq!(
        app.name, "first-app",
        "owner miss with empty key should return first app"
    );

    // Owner miss with name key falls through to name match.
    let app = ghtkn::config::select_app(&cfg, "second-app", "nonexistent-org").unwrap();
    assert_eq!(
        app.name, "second-app",
        "owner miss should fall through to name match"
    );

    // Owner miss + name miss returns None.
    let app = ghtkn::config::select_app(&cfg, "nonexistent", "nonexistent-org");
    assert!(app.is_none(), "owner miss + name miss should return None");
}

// ---------------------------------------------------------------------------
// Test 6: Config file read -> validate -> select roundtrip with multiple apps
// ---------------------------------------------------------------------------

#[test]
fn full_config_roundtrip_multiple_apps() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("ghtkn.yaml");
    std::fs::write(
        &config_path,
        r#"apps:
  - name: personal
    client_id: Iv1.personal123
  - name: work
    client_id: Iv1.work456
    git_owner: my-company
  - name: oss
    client_id: Iv1.oss789
    git_owner: open-source-org
"#,
    )
    .unwrap();

    let cfg = ghtkn::config::read(&config_path).unwrap().unwrap();
    cfg.validate().unwrap();

    assert_eq!(cfg.apps.len(), 3);

    // Default (no key, no owner) returns first app.
    let app = ghtkn::config::select_app(&cfg, "", "").unwrap();
    assert_eq!(app.name, "personal");

    // Select by owner.
    let app = ghtkn::config::select_app(&cfg, "", "my-company").unwrap();
    assert_eq!(app.name, "work");

    // Select by name.
    let app = ghtkn::config::select_app(&cfg, "oss", "").unwrap();
    assert_eq!(app.name, "oss");
}
