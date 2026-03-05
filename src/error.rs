use thiserror::Error;

/// All possible errors in the ghtkn SDK.
#[derive(Debug, Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(String),
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("device flow error: {0}")]
    DeviceFlow(String),
    #[error("github API error: {0}")]
    GitHub(String),
    #[error("browser error: {0}")]
    Browser(String),
    /// Non-fatal sentinel: token was obtained but could not be persisted in
    /// the keyring. Matches Go SDK's `ErrStoreToken` — the caller should
    /// extract the token and app from this variant and continue.
    #[error("store token error: {message}")]
    StoreToken {
        message: String,
        token: Box<crate::keyring::AccessToken>,
        app: Box<crate::config::App>,
    },
    #[error("{0}")]
    Other(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
