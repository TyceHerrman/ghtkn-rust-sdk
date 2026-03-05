//! OAuth device flow protocol for GitHub access tokens.
//!
//! Implements the full device authorization grant flow as specified in
//! [RFC 8628](https://datatracker.ietf.org/doc/html/rfc8628):
//!
//! 1. Request a device code from GitHub
//! 2. Display the user code and verification URI to the user
//! 3. Open the verification URI in the user's browser
//! 4. Poll for the access token until the user authorizes or the code expires
//!
//! Polling uses interval-based timing (not exponential backoff):
//! - Initial interval = max(server_interval, 5s)
//! - On `slow_down`: current_interval += 5s
//! - On `authorization_pending`: keep current interval

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::browser::{Browser, BrowserError};
use crate::log::Logger;

/// Additional interval added on `slow_down` responses (5 seconds).
const ADDITIONAL_INTERVAL: Duration = Duration::from_secs(5);

/// Minimum polling interval (5 seconds).
const MIN_INTERVAL_SECS: i64 = 5;

/// Response from GitHub's device code endpoint (`POST /login/device/code`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: i64,
    pub interval: i64,
}

/// Response from GitHub's access token endpoint during polling.
#[derive(Debug, Clone, Deserialize)]
struct AccessTokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_in: i64,
    #[serde(default)]
    error: String,
}

/// Result of the device flow -- a fresh access token with its expiration date.
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub access_token: String,
    pub expiration_date: DateTime<Utc>,
}

/// Trait for displaying the device code to the user.
///
/// Implementations control how the user code and verification URI are
/// presented (terminal, GUI, etc.).
pub trait DeviceCodeUI: Send + Sync {
    /// Display the device code information to the user.
    fn show(
        &self,
        device_code: &DeviceCodeResponse,
        expiration_date: DateTime<Utc>,
    ) -> Result<(), crate::Error>;
}

/// Simple terminal-based device code UI.
///
/// Behavior matches the Go SDK's `SimpleDeviceCodeUI`:
/// - If stdin **is** a terminal: prints the user code and waits for Enter
/// - If stdin is **not** a terminal (pipe/redirect): prints and waits 5 seconds
pub struct SimpleDeviceCodeUI;

impl DeviceCodeUI for SimpleDeviceCodeUI {
    fn show(
        &self,
        device_code: &DeviceCodeResponse,
        expiration_date: DateTime<Utc>,
    ) -> Result<(), crate::Error> {
        use std::io::IsTerminal;

        let ex = expiration_date.to_rfc3339();

        if std::io::stdin().is_terminal() {
            eprintln!(
                "The application uses the device flow to generate your GitHub User Access Token.\n\
                 Copy your one-time code: {}\n\
                 This code is valid until {}\n\
                 Press Enter to open {} in your browser...",
                device_code.user_code, ex, device_code.verification_uri,
            );
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
        } else {
            eprintln!(
                "The application uses the device flow to generate your GitHub User Access Token.\n\
                 Copy your one-time code: {}\n\
                 This code is valid until {}\n\
                 {} will open automatically after a few seconds...",
                device_code.user_code, ex, device_code.verification_uri,
            );
            std::thread::sleep(Duration::from_secs(5));
        }
        Ok(())
    }
}

/// Device flow client for obtaining GitHub access tokens.
///
/// Encapsulates the HTTP client, browser, logger, and UI needed to run the
/// full device authorization grant flow. All dependencies are borrowed so that
/// the caller retains ownership and can reuse them across multiple calls.
pub struct DeviceFlowClient<'a> {
    http_client: reqwest::Client,
    browser: &'a dyn Browser,
    logger: &'a Logger,
    device_code_ui: &'a dyn DeviceCodeUI,
    /// Base URL for GitHub endpoints. Defaults to `https://github.com`.
    /// Override for testing with a mock server.
    base_url: String,
}

impl<'a> DeviceFlowClient<'a> {
    /// Create a new device flow client with the default GitHub base URL.
    pub fn new(
        http_client: reqwest::Client,
        browser: &'a dyn Browser,
        logger: &'a Logger,
        device_code_ui: &'a dyn DeviceCodeUI,
    ) -> Self {
        Self {
            http_client,
            browser,
            logger,
            device_code_ui,
            base_url: "https://github.com".to_string(),
        }
    }

    /// Create a new device flow client with a custom base URL (for testing).
    pub fn with_base_url(
        http_client: reqwest::Client,
        browser: &'a dyn Browser,
        logger: &'a Logger,
        device_code_ui: &'a dyn DeviceCodeUI,
        base_url: String,
    ) -> Self {
        Self {
            http_client,
            browser,
            logger,
            device_code_ui,
            base_url,
        }
    }

    /// Run the full device flow to obtain an access token.
    ///
    /// 1. Requests a device code from GitHub
    /// 2. Displays the device code to the user via [`DeviceCodeUI`]
    /// 3. Opens the verification URI in the browser (non-fatal on failure)
    /// 4. Polls for the access token until authorized or expired
    pub async fn create(&self, client_id: &str) -> crate::Result<AccessToken> {
        if client_id.is_empty() {
            return Err(crate::Error::DeviceFlow("client id is required".into()));
        }
        let device_code = self.get_device_code(client_id).await?;

        let now = Utc::now();
        let expiration_date = now + chrono::Duration::seconds(device_code.expires_in);

        // Show device code UI (errors propagated).
        self.device_code_ui.show(&device_code, expiration_date)?;

        // Try to open browser (always non-fatal).
        self.open_browser(&device_code.verification_uri);

        // Poll for access token.
        let token_resp = self
            .poll_for_access_token(
                client_id,
                &device_code.device_code,
                device_code.interval,
                expiration_date,
            )
            .await?;

        let token_expiration = Utc::now() + chrono::Duration::seconds(token_resp.expires_in);

        Ok(AccessToken {
            access_token: token_resp.access_token,
            expiration_date: token_expiration,
        })
    }

    /// Request a device code from GitHub.
    async fn get_device_code(&self, client_id: &str) -> crate::Result<DeviceCodeResponse> {
        let url = format!("{}/login/device/code", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .header("Accept", "application/json")
            .json(&serde_json::json!({"client_id": client_id}))
            .send()
            .await
            .map_err(|e| crate::Error::DeviceFlow(format!("request device code: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_else(|_| "<unreadable>".into());
            return Err(crate::Error::DeviceFlow(format!(
                "error from GitHub: status={status}, body={body}"
            )));
        }

        resp.json::<DeviceCodeResponse>()
            .await
            .map_err(|e| crate::Error::DeviceFlow(format!("decode device code response: {e}")))
    }

    /// Check for an access token (single poll request).
    async fn check_access_token(
        &self,
        client_id: &str,
        device_code: &str,
    ) -> crate::Result<AccessTokenResponse> {
        let url = format!("{}/login/oauth/access_token", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": client_id,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
            }))
            .send()
            .await
            .map_err(|e| crate::Error::DeviceFlow(format!("check access token: {e}")))?;

        resp.json::<AccessTokenResponse>()
            .await
            .map_err(|e| crate::Error::DeviceFlow(format!("decode access token response: {e}")))
    }

    /// Poll for an access token until the user authorizes or the device code expires.
    ///
    /// Uses interval-based polling (not exponential backoff):
    /// - Initial interval = max(server_interval, 5s)
    /// - On `slow_down`: interval += 5s
    /// - On `authorization_pending`: keep current interval
    async fn poll_for_access_token(
        &self,
        client_id: &str,
        device_code: &str,
        server_interval: i64,
        expiration_date: DateTime<Utc>,
    ) -> crate::Result<AccessTokenResponse> {
        let mut interval = Duration::from_secs(server_interval.max(MIN_INTERVAL_SECS) as u64);

        loop {
            tokio::time::sleep(interval).await;

            // Check if device code expired.
            if Utc::now() >= expiration_date {
                return Err(crate::Error::DeviceFlow("device code expired".into()));
            }

            let resp = self.check_access_token(client_id, device_code).await?;

            match resp.error.as_str() {
                "" => return Ok(resp),
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += ADDITIONAL_INTERVAL;
                    continue;
                }
                other => {
                    return Err(crate::Error::DeviceFlow(format!(
                        "device flow error: {other}"
                    )));
                }
            }
        }
    }

    /// Attempt to open the verification URI in the user's browser.
    ///
    /// All errors are non-fatal:
    /// - `NoCommandFound`: silently suppressed (matches Go SDK behavior)
    /// - Other failures: logged via the `failed_to_open_browser` callback
    fn open_browser(&self, url: &str) {
        match self.browser.open(url) {
            Ok(()) => {}
            Err(BrowserError::NoCommandFound) => {
                // Suppress warning -- matches Go SDK's ErrNoCommandFound behavior.
            }
            Err(BrowserError::Failed(msg)) => {
                if let Some(cb) = &self.logger.failed_to_open_browser {
                    cb(&msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::browser::{Browser, BrowserError};

    // ---------------------------------------------------------------
    // Test helpers
    // ---------------------------------------------------------------

    /// A mock browser that always succeeds.
    struct OkBrowser;

    impl Browser for OkBrowser {
        fn open(&self, _url: &str) -> Result<(), BrowserError> {
            Ok(())
        }
    }

    /// A mock browser that returns a failure.
    struct FailingBrowser;

    impl Browser for FailingBrowser {
        fn open(&self, _url: &str) -> Result<(), BrowserError> {
            Err(BrowserError::Failed("browser crashed".into()))
        }
    }

    /// A mock browser that returns NoCommandFound.
    struct NoCommandBrowser;

    impl Browser for NoCommandBrowser {
        fn open(&self, _url: &str) -> Result<(), BrowserError> {
            Err(BrowserError::NoCommandFound)
        }
    }

    /// A no-op device code UI.
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

    /// Create a device flow client pointing at the given mock server.
    fn make_client<'a>(
        server: &MockServer,
        browser: &'a dyn Browser,
        logger: &'a Logger,
        ui: &'a dyn DeviceCodeUI,
    ) -> DeviceFlowClient<'a> {
        DeviceFlowClient::with_base_url(reqwest::Client::new(), browser, logger, ui, server.uri())
    }

    // ---------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------

    #[tokio::test]
    async fn test_get_device_code() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_test123",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 5
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let resp = client.get_device_code("test_client_id").await.unwrap();

        assert_eq!(resp.device_code, "dc_test123");
        assert_eq!(resp.user_code, "ABCD-1234");
        assert_eq!(resp.verification_uri, "https://github.com/login/device");
        assert_eq!(resp.expires_in, 900);
        assert_eq!(resp.interval, 5);
    }

    #[tokio::test]
    async fn test_poll_success_immediate() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_success123",
                "expires_in": 28800
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);

        // Use a very short interval for testing and a far-future expiration.
        let expiration = Utc::now() + chrono::Duration::seconds(900);
        let resp = client
            .poll_for_access_token("test_client_id", "dc_test", 0, expiration)
            .await
            .unwrap();

        assert_eq!(resp.access_token, "ghu_success123");
        assert_eq!(resp.expires_in, 28800);
    }

    #[tokio::test]
    async fn test_poll_authorization_pending_then_success() {
        let server = MockServer::start().await;

        // First two requests return authorization_pending, third returns success.
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending"
            })))
            .up_to_n_times(2)
            .expect(2)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_after_pending",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let expiration = Utc::now() + chrono::Duration::seconds(900);

        // Use interval=0 so polling is fast in tests.
        let resp = client
            .poll_for_access_token("test_client_id", "dc_test", 0, expiration)
            .await
            .unwrap();

        assert_eq!(resp.access_token, "ghu_after_pending");
        assert_eq!(resp.expires_in, 3600);
    }

    #[tokio::test]
    async fn test_poll_slow_down_increases_interval() {
        let server = MockServer::start().await;

        // First request returns slow_down, second returns success.
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "slow_down"
            })))
            .up_to_n_times(1)
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_after_slowdown",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let expiration = Utc::now() + chrono::Duration::seconds(900);

        // Start with interval 0; after slow_down it becomes 0 + 5s = 5s.
        // We can't easily assert the timing in a unit test, but we verify
        // that the flow still completes successfully after slow_down.
        let resp = client
            .poll_for_access_token("test_client_id", "dc_test", 0, expiration)
            .await
            .unwrap();

        assert_eq!(resp.access_token, "ghu_after_slowdown");
    }

    #[tokio::test]
    async fn test_poll_device_code_expired() {
        let server = MockServer::start().await;

        // Always return authorization_pending.
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending"
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);

        // Set expiration in the past so the first poll check sees it expired.
        let expiration = Utc::now() - chrono::Duration::seconds(10);
        let result = client
            .poll_for_access_token("test_client_id", "dc_test", 0, expiration)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("device code expired"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_poll_unknown_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let expiration = Utc::now() + chrono::Duration::seconds(900);

        let result = client
            .poll_for_access_token("test_client_id", "dc_test", 0, expiration)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("access_denied"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn test_browser_failure_is_non_fatal() {
        let server = MockServer::start().await;

        // Mock device code endpoint.
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_test",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;

        // Mock token endpoint -- immediate success.
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_browser_fail",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        // Use a browser that fails -- the flow should still complete.
        let browser = FailingBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let result = client.create("test_client_id").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().access_token, "ghu_browser_fail");
    }

    #[tokio::test]
    async fn test_browser_no_command_found_suppresses_warning() {
        let warning_logged = Arc::new(AtomicBool::new(false));
        let warning_logged_clone = Arc::clone(&warning_logged);

        let mut logger = Logger::new();
        logger.failed_to_open_browser = Some(Box::new(move |_msg| {
            warning_logged_clone.store(true, Ordering::SeqCst);
        }));

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_test",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_no_cmd",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let browser = NoCommandBrowser;
        let ui = NoopUI;
        let client = DeviceFlowClient::with_base_url(
            reqwest::Client::new(),
            &browser,
            &logger,
            &ui,
            server.uri(),
        );

        let result = client.create("test_client_id").await;
        assert!(result.is_ok());

        // The warning callback should NOT have been called for NoCommandFound.
        assert!(
            !warning_logged.load(Ordering::SeqCst),
            "NoCommandFound should not trigger the warning callback"
        );
    }

    #[tokio::test]
    async fn test_browser_failed_logs_warning() {
        let warning_logged = Arc::new(AtomicBool::new(false));
        let warning_logged_clone = Arc::clone(&warning_logged);

        let mut logger = Logger::new();
        logger.failed_to_open_browser = Some(Box::new(move |_msg| {
            warning_logged_clone.store(true, Ordering::SeqCst);
        }));

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_test",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_browser_warn",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let browser = FailingBrowser;
        let ui = NoopUI;
        let client = DeviceFlowClient::with_base_url(
            reqwest::Client::new(),
            &browser,
            &logger,
            &ui,
            server.uri(),
        );

        let result = client.create("test_client_id").await;
        assert!(result.is_ok());

        // The warning callback SHOULD have been called for Failed.
        assert!(
            warning_logged.load(Ordering::SeqCst),
            "Failed browser error should trigger the warning callback"
        );
    }

    #[test]
    fn test_simple_device_code_ui_does_not_panic() {
        let ui = SimpleDeviceCodeUI;
        let device_code = DeviceCodeResponse {
            device_code: "dc_test".into(),
            user_code: "ABCD-1234".into(),
            verification_uri: "https://github.com/login/device".into(),
            expires_in: 900,
            interval: 5,
        };
        let expiration = Utc::now() + chrono::Duration::seconds(900);
        let result = ui.show(&device_code, expiration);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_rejects_empty_client_id() {
        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = DeviceFlowClient::new(reqwest::Client::new(), &browser, &logger, &ui);
        let result = client.create("").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("client id is required"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn test_get_device_code_non_200_status() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(422).set_body_string("bad request"))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let result = client.get_device_code("test_client_id").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("error from GitHub") && err.contains("422"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_device_code_response_serialization() {
        let resp = DeviceCodeResponse {
            device_code: "dc_abc".into(),
            user_code: "WXYZ-9999".into(),
            verification_uri: "https://github.com/login/device".into(),
            expires_in: 600,
            interval: 10,
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DeviceCodeResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.device_code, "dc_abc");
        assert_eq!(parsed.user_code, "WXYZ-9999");
        assert_eq!(parsed.verification_uri, "https://github.com/login/device");
        assert_eq!(parsed.expires_in, 600);
        assert_eq!(parsed.interval, 10);
    }

    #[tokio::test]
    async fn test_full_create_flow() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dc_full",
                "user_code": "FULL-FLOW",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 0
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ghu_full_flow_token",
                "expires_in": 28800
            })))
            .mount(&server)
            .await;

        let browser = OkBrowser;
        let logger = Logger::new();
        let ui = NoopUI;
        let client = make_client(&server, &browser, &logger, &ui);
        let token = client.create("test_client_id").await.unwrap();

        assert_eq!(token.access_token, "ghu_full_flow_token");
        // Expiration should be roughly now + 28800s (8 hours).
        let now = Utc::now();
        assert!(token.expiration_date > now);
        assert!(token.expiration_date < now + chrono::Duration::seconds(28800 + 10));
    }
}
