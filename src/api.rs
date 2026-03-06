//! Token manager and public client API.
//!
//! Orchestrates the full token flow: config -> keyring -> device flow -> GitHub
//! API -> keyring store. Provides the [`Client`] as the main entry point and
//! [`TokenSource`] for cached, on-demand token retrieval.

use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::browser::{Browser, DefaultBrowser};
use crate::config::{self, App};
use crate::deviceflow::{DeviceCodeUI, DeviceFlowClient, SimpleDeviceCodeUI};
use crate::error::Error;
use crate::github::GitHubClient;
use crate::keyring::{AccessToken, DEFAULT_SERVICE_KEY, Keyring};
use crate::log::Logger;

/// Input parameters for token retrieval.
///
/// All fields have sensible defaults. Use [`Default::default`] for the common
/// case and override individual fields as needed.
pub struct InputGet {
    /// Custom keyring service name. When empty, [`DEFAULT_SERVICE_KEY`] is used.
    pub keyring_service: String,
    /// App selection by name. When empty, falls back to the `GHTKN_APP`
    /// environment variable.
    pub app_name: String,
    /// Custom config file path. When empty, the platform-specific default is
    /// auto-detected via [`config::get_path`].
    pub config_file_path: String,
    /// App selection by git owner (used for git-credential integration).
    pub app_owner: String,
    /// Minimum remaining token lifetime before renewal. A token whose
    /// expiration is within this duration of "now" is treated as expired.
    pub min_expiration: Duration,
}

impl Default for InputGet {
    fn default() -> Self {
        Self {
            keyring_service: String::new(),
            app_name: String::new(),
            config_file_path: String::new(),
            app_owner: String::new(),
            min_expiration: Duration::ZERO,
        }
    }
}

/// The main client for GitHub token management.
///
/// Owns all dependencies (logger, browser, device-code UI, keyring) and lends
/// them to the underlying [`DeviceFlowClient`] when a new token must be created.
///
/// # Example
///
/// ```no_run
/// use ghtkn::{Client, InputGet};
///
/// # async fn run() -> ghtkn::Result<()> {
/// let client = Client::new();
/// let (token, app) = client.get(&InputGet::default()).await?;
/// println!("token for {}: {}...", app.name, &token.access_token[..8]);
/// # Ok(())
/// # }
/// ```
pub struct Client {
    logger: Logger,
    device_code_ui: Box<dyn DeviceCodeUI>,
    browser: Box<dyn Browser>,
    keyring: Keyring,
    github_base_url: String,
    api_base_url: String,
}

impl Client {
    /// Create a new client with default dependencies.
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
            device_code_ui: Box::new(SimpleDeviceCodeUI),
            browser: Box::new(DefaultBrowser),
            keyring: Keyring::new(),
            github_base_url: "https://github.com".to_string(),
            api_base_url: "https://api.github.com".to_string(),
        }
    }

    /// Set a custom logger.
    pub fn set_logger(&mut self, logger: Logger) {
        self.logger = logger;
    }

    /// Set a custom device code UI.
    pub fn set_device_code_ui(&mut self, ui: Box<dyn DeviceCodeUI>) {
        self.device_code_ui = ui;
    }

    /// Set a custom browser opener.
    pub fn set_browser(&mut self, browser: Box<dyn Browser>) {
        self.browser = browser;
    }

    /// Set a custom keyring (e.g. with a mock backend for testing).
    pub fn set_keyring(&mut self, keyring: Keyring) {
        self.keyring = keyring;
    }

    /// Set a custom GitHub base URL (e.g. for GitHub Enterprise Server).
    pub fn set_github_base_url(&mut self, url: String) {
        self.github_base_url = url.trim_end_matches('/').to_string();
    }

    /// Set a custom GitHub API base URL (e.g. for GitHub Enterprise Server).
    pub fn set_api_base_url(&mut self, url: String) {
        self.api_base_url = url.trim_end_matches('/').to_string();
    }

    /// Create a reusable [`TokenSource`] that caches the access token.
    ///
    /// Consumes the `Client` and returns a `TokenSource` that can be
    /// called repeatedly to get a cached token.
    pub fn token_source(self, input: InputGet) -> TokenSource {
        TokenSource::new(self, input)
    }

    /// Get a GitHub access token.
    ///
    /// Flow:
    /// 1. Determine config file path (from input or auto-detect)
    /// 2. Read and validate YAML config
    /// 3. Select app (by owner, by name, or first)
    /// 4. Check keyring for cached, non-expired token
    /// 5. If expired/missing: device flow -> get user login -> store in keyring
    /// 6. Return token and app config
    pub async fn get(&self, input: &InputGet) -> crate::Result<(AccessToken, App)> {
        // 1. Determine config path.
        let config_path = if input.config_file_path.is_empty() {
            config::get_path(|k| std::env::var(k).ok(), std::env::consts::OS)?
        } else {
            PathBuf::from(&input.config_file_path)
        };

        // 2. Read and validate config.
        let cfg = config::read(&config_path)?
            .ok_or_else(|| Error::Config("configuration file is empty".into()))?;
        cfg.validate()?;

        // 3. Select app.
        let app_name = if input.app_name.is_empty() {
            std::env::var("GHTKN_APP").unwrap_or_default()
        } else {
            input.app_name.clone()
        };

        let app = config::select_app(&cfg, &app_name, &input.app_owner)
            .ok_or_else(|| Error::Config("no matching app found".into()))?
            .clone();

        // 4. Resolve keyring service.
        let service = if input.keyring_service.is_empty() {
            DEFAULT_SERVICE_KEY.to_string()
        } else {
            input.keyring_service.clone()
        };

        // 5. Try keyring, fall back to device flow.
        //
        // StoreToken is non-fatal: the token was obtained but the keyring
        // write failed.  Re-wrap it so the caller's (token, app) are the
        // ones from the error, exactly like Go's
        // `return token, app, ErrStoreToken`.
        match self
            .get_or_create_token(&service, &app, input.min_expiration)
            .await
        {
            Ok(token) => Ok((token, app)),
            Err(Error::StoreToken { message, token, .. }) => Err(Error::StoreToken {
                message,
                token,
                app: Box::new(app),
            }),
            Err(e) => Err(e),
        }
    }

    /// Try to retrieve a valid token from the keyring; create one if
    /// missing or expired.
    async fn get_or_create_token(
        &self,
        service: &str,
        app: &App,
        min_expiration: Duration,
    ) -> crate::Result<AccessToken> {
        match self.keyring.get(service, &app.client_id) {
            Ok(Some(token)) => {
                if check_expired(token.expiration_date, min_expiration) {
                    // Token expired — log and fall through to create a new one.
                    if let Some(cb) = &self.logger.expire {
                        cb(token.expiration_date);
                    }
                } else {
                    return Ok(token);
                }
            }
            Ok(None) => {
                if let Some(cb) = &self.logger.access_token_is_not_found_in_keyring {
                    cb();
                }
            }
            Err(e) => {
                if let Some(cb) = &self.logger.failed_to_get_access_token_from_keyring {
                    cb(&e.to_string());
                }
            }
        }

        self.create_token(service, app).await
    }

    /// Run the device flow to obtain a fresh token, fetch the user login,
    /// and store the result in the keyring.
    ///
    /// If the keyring write fails, returns [`Error::StoreToken`] carrying the
    /// token and app (matching Go SDK's `ErrStoreToken` non-fatal sentinel).
    async fn create_token(&self, service: &str, app: &App) -> crate::Result<AccessToken> {
        let http_client = reqwest::Client::new();

        let df_client = DeviceFlowClient::with_base_url(
            http_client,
            self.browser.as_ref(),
            &self.logger,
            self.device_code_ui.as_ref(),
            self.github_base_url.clone(),
        );

        let df_token = df_client.create(&app.client_id).await?;

        // Get user login via GET /user.
        let gh_client =
            GitHubClient::with_base_url(&df_token.access_token, self.api_base_url.clone());
        let user = gh_client.get_user().await?;

        let kr_token = AccessToken {
            access_token: df_token.access_token,
            expiration_date: df_token.expiration_date,
            login: user.login,
        };

        // Store in keyring — non-fatal. On failure return StoreToken with
        // the token and app so callers can still use them (matches Go SDK's
        // `return token, app, ErrStoreToken`).
        if let Err(e) = self.keyring.set(service, &app.client_id, &kr_token) {
            return Err(Error::StoreToken {
                message: e.to_string(),
                token: Box::new(kr_token),
                app: Box::new(app.clone()),
            });
        }

        Ok(kr_token)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Cached token source for repeated access.
///
/// Wraps a [`Client`] and caches the token after the first successful
/// retrieval. Thread-safe via an internal `tokio::sync::Mutex`.
pub struct TokenSource {
    client: Client,
    input: InputGet,
    cached: tokio::sync::Mutex<Option<String>>,
}

impl TokenSource {
    /// Create a new `TokenSource` that will retrieve tokens using the given
    /// client and input parameters.
    pub fn new(client: Client, input: InputGet) -> Self {
        Self {
            client,
            input,
            cached: tokio::sync::Mutex::new(None),
        }
    }

    /// Get a token, returning a cached value if available.
    ///
    /// Handles [`Error::StoreToken`] transparently: if the token was obtained
    /// but the keyring write failed, the token is still cached and returned.
    pub async fn token(&self) -> crate::Result<String> {
        let mut cached = self.cached.lock().await;
        if let Some(token) = cached.as_ref() {
            return Ok(token.clone());
        }
        let access_token = match self.client.get(&self.input).await {
            Ok((token, _)) => token.access_token,
            Err(Error::StoreToken { token, .. }) => token.access_token,
            Err(e) => return Err(e),
        };
        *cached = Some(access_token.clone());
        Ok(access_token)
    }

    /// Get a token if available, returning `None` on any error.
    ///
    /// This is the recommended entry point for consumers using ghtkn as a
    /// fallback token source (like mise or aqua). All errors are treated
    /// as "no token available" and logged via `tracing::warn!`.
    ///
    /// **Note**: On a cache miss, this triggers the full OAuth device flow
    /// (opens browser, waits for user authorization). Gate calls behind an
    /// opt-in check (e.g. an environment variable) if this is undesirable.
    pub async fn token_or_none(&self) -> Option<String> {
        match self.token().await {
            Ok(token) => Some(token),
            Err(e) => {
                tracing::warn!(error = %e, "ghtkn token unavailable");
                None
            }
        }
    }
}

/// Check whether a token should be considered expired.
///
/// Returns `true` if `now + min_expiration` is after `expiration_date`,
/// meaning the token has less than `min_expiration` remaining.
fn check_expired(expiration_date: DateTime<Utc>, min_expiration: Duration) -> bool {
    let min_exp = chrono::Duration::from_std(min_expiration).unwrap_or(chrono::Duration::zero());
    Utc::now() + min_exp > expiration_date
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{TimeZone, Utc};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::browser::BrowserError;
    use crate::deviceflow::DeviceCodeResponse;
    use crate::keyring::KeyringBackend;

    // ---------------------------------------------------------------
    // check_expired
    // ---------------------------------------------------------------

    #[test]
    fn test_check_expired_not_expired() {
        // Token expires far in the future -- should NOT be expired.
        let expiration = Utc::now() + chrono::Duration::hours(8);
        assert!(!check_expired(expiration, Duration::ZERO));
    }

    #[test]
    fn test_check_expired_is_expired() {
        // Token expired in the past -- should be expired.
        let expiration = Utc::now() - chrono::Duration::hours(1);
        assert!(check_expired(expiration, Duration::ZERO));
    }

    #[test]
    fn test_check_expired_with_min_expiration() {
        // Token expires in 5 minutes, but we require 10 minutes remaining.
        let expiration = Utc::now() + chrono::Duration::minutes(5);
        let min_exp = Duration::from_secs(10 * 60); // 10 minutes
        assert!(check_expired(expiration, min_exp));
    }

    #[test]
    fn test_check_expired_with_min_expiration_sufficient() {
        // Token expires in 20 minutes, we require 10 minutes remaining.
        let expiration = Utc::now() + chrono::Duration::minutes(20);
        let min_exp = Duration::from_secs(10 * 60); // 10 minutes
        assert!(!check_expired(expiration, min_exp));
    }

    #[test]
    fn test_check_expired_exactly_at_boundary() {
        // Token at exact Unix epoch should definitely be expired.
        let expiration = Utc.with_ymd_and_hms(1970, 1, 1, 0, 0, 0).unwrap();
        assert!(check_expired(expiration, Duration::ZERO));
    }

    // ---------------------------------------------------------------
    // InputGet defaults
    // ---------------------------------------------------------------

    #[test]
    fn test_input_get_default() {
        let input = InputGet::default();
        assert!(input.keyring_service.is_empty());
        assert!(input.app_name.is_empty());
        assert!(input.config_file_path.is_empty());
        assert!(input.app_owner.is_empty());
        assert_eq!(input.min_expiration, Duration::ZERO);
    }

    // ---------------------------------------------------------------
    // Client builder pattern
    // ---------------------------------------------------------------

    #[test]
    fn test_client_new() {
        // Should not panic.
        let _client = Client::new();
    }

    #[test]
    fn test_client_default() {
        // Default impl should not panic.
        let _client = Client::default();
    }

    #[test]
    fn test_client_set_logger() {
        let mut client = Client::new();
        let logger = Logger::new();
        client.set_logger(logger);
    }

    #[test]
    fn test_client_set_device_code_ui() {
        let mut client = Client::new();
        client.set_device_code_ui(Box::new(SimpleDeviceCodeUI));
    }

    #[test]
    fn test_client_set_browser() {
        let mut client = Client::new();
        client.set_browser(Box::new(DefaultBrowser));
    }

    #[test]
    fn test_client_set_keyring() {
        let mut client = Client::new();
        client.set_keyring(Keyring::new());
    }

    // ---------------------------------------------------------------
    // TokenSource
    // ---------------------------------------------------------------

    #[test]
    fn test_token_source_new() {
        // Should not panic.
        let _ts = TokenSource::new(Client::new(), InputGet::default());
    }

    // ---------------------------------------------------------------
    // Integration tests: StoreToken recovery, caching, token_or_none
    // ---------------------------------------------------------------

    struct NoopBrowser;

    impl Browser for NoopBrowser {
        fn open(&self, _url: &str) -> Result<(), BrowserError> {
            Ok(())
        }
    }

    struct NoopUI;

    impl DeviceCodeUI for NoopUI {
        fn show(
            &self,
            _device_code: &DeviceCodeResponse,
            _expiration_date: DateTime<Utc>,
        ) -> Result<(), crate::Error> {
            Ok(())
        }
    }

    /// Keyring backend that reads return "no entry" and writes always fail.
    struct FailingWriteBackend;

    impl KeyringBackend for FailingWriteBackend {
        fn get(&self, _service: &str, _user: &str) -> crate::Result<Option<String>> {
            Ok(None)
        }

        fn set(&self, _service: &str, _user: &str, _password: &str) -> crate::Result<()> {
            Err(crate::Error::Keyring(
                "simulated keyring write failure".into(),
            ))
        }
    }

    /// Mount wiremock mocks for the full device flow + /user endpoint.
    async fn mount_device_flow_mocks(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_test",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_test_token_abc",
                "expires_in": 28800
            })))
            .mount(server)
            .await;

        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"login": "testuser"})),
            )
            .mount(server)
            .await;
    }

    /// Build a Client pointing at the wiremock server with a failing keyring.
    fn make_test_client(server_uri: &str) -> Client {
        let mut client = Client::new();
        client.set_browser(Box::new(NoopBrowser));
        client.set_device_code_ui(Box::new(NoopUI));
        client.set_keyring(Keyring::with_backend(Box::new(FailingWriteBackend)));
        client.set_github_base_url(server_uri.to_string());
        client.set_api_base_url(server_uri.to_string());
        client
    }

    /// Test A + B: token() recovers from StoreToken and caches the result.
    ///
    /// The failing keyring triggers StoreToken, but TokenSource::token()
    /// extracts the token and returns Ok. A second call returns the cached
    /// token without hitting the server again.
    #[tokio::test]
    async fn test_token_store_token_recovery_and_caching() {
        let server = MockServer::start().await;
        mount_device_flow_mocks(&server).await;

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("ghtkn.yaml");
        std::fs::write(
            &config_path,
            "apps:\n  - name: test-app\n    client_id: test_client_id\n",
        )
        .unwrap();

        let client = make_test_client(&server.uri());
        let ts = client.token_source(InputGet {
            config_file_path: config_path.to_str().unwrap().to_string(),
            ..Default::default()
        });

        // Test A: first call recovers the token despite keyring write failure.
        let token = ts.token().await.expect("should recover from StoreToken");
        assert_eq!(token, "ghu_test_token_abc");

        // Record how many requests hit the access_token endpoint.
        let requests_after_first = server.received_requests().await.unwrap().len();

        // Test B: second call returns cached token — no new server requests.
        let token2 = ts.token().await.expect("should return cached token");
        assert_eq!(token2, "ghu_test_token_abc");

        let requests_after_second = server.received_requests().await.unwrap().len();
        assert_eq!(
            requests_after_first, requests_after_second,
            "second call should use cached token, not hit server"
        );
    }

    /// Test C: token_or_none() returns None when the config file doesn't exist.
    #[tokio::test]
    async fn test_token_or_none_returns_none_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing_config = dir.path().join("ghtkn.yaml");

        let client = Client::new();
        let ts = client.token_source(InputGet {
            config_file_path: missing_config.to_str().unwrap().to_string(),
            ..Default::default()
        });

        let result = ts.token_or_none().await;
        assert!(
            result.is_none(),
            "should return None when config is missing"
        );
    }

    /// Test D: token_or_none() returns Some on success (via StoreToken recovery).
    #[tokio::test]
    async fn test_token_or_none_returns_some_on_success() {
        let server = MockServer::start().await;
        mount_device_flow_mocks(&server).await;

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("ghtkn.yaml");
        std::fs::write(
            &config_path,
            "apps:\n  - name: test-app\n    client_id: test_client_id\n",
        )
        .unwrap();

        let client = make_test_client(&server.uri());
        let ts = client.token_source(InputGet {
            config_file_path: config_path.to_str().unwrap().to_string(),
            ..Default::default()
        });

        let result = ts.token_or_none().await;
        assert_eq!(
            result,
            Some("ghu_test_token_abc".to_string()),
            "should return Some(token) on success"
        );
    }
}
