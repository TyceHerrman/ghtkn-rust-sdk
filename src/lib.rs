//! GitHub token management via OAuth device flow.
//!
//! `ghtkn` provides a complete SDK for obtaining GitHub access tokens through
//! the OAuth device authorization grant flow (RFC 8628). Tokens are cached in
//! the system keyring for reuse across sessions.
//!
//! # Quick start
//!
//! ```no_run
//! use ghtkn::{Client, InputGet};
//!
//! # async fn run() -> ghtkn::Result<()> {
//! let client = Client::new();
//! let (token, app) = client.get(&InputGet::default()).await?;
//! println!("Token for {}: {}...", app.name, &token.access_token[..8]);
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod browser;
pub mod config;
pub mod deviceflow;
pub mod error;
pub mod github;
pub mod keyring;
pub mod log;

// -- Primary API --
pub use api::{Client, InputGet, TokenSource};

// -- Browser --
pub use browser::{Browser, BrowserError, DefaultBrowser};

// -- Config --
pub use config::{App, Config};

// -- Device flow --
pub use deviceflow::{DeviceCodeResponse, DeviceCodeUI, SimpleDeviceCodeUI};

// -- Errors --
pub use error::{Error, Result};

// -- GitHub --
pub use github::GitHubClient;

// -- Keyring --
pub use keyring::{AccessToken, DEFAULT_SERVICE_KEY};

// -- Logger --
pub use log::Logger;

/// Return the default config file path for the current platform.
///
/// Delegates to [`config::get_path`] with the real environment and OS.
pub fn get_config_path() -> Result<String> {
    let path = config::get_path(|k| std::env::var(k).ok(), std::env::consts::OS)?;
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Config("config path contains non-UTF-8 characters".into()))
}
