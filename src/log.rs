//! Customizable logging callbacks for the token flow.
//!
//! The [`Logger`] struct holds optional callbacks for each notable event during
//! token acquisition. When a callback is `None`, calling [`Logger::init`] fills
//! it with a default that emits a [`tracing`] log at the appropriate level.
//!
//! This mirrors the Go SDK's `Logger` struct with its `Expire`,
//! `FailedToOpenBrowser`, etc. function fields.

use chrono::{DateTime, Utc};

/// A callback that receives a `DateTime<Utc>` (e.g. a token expiration date).
type DateCallback = Box<dyn Fn(DateTime<Utc>) + Send + Sync>;

/// A callback that receives an error message string.
type ErrorCallback = Box<dyn Fn(&str) + Send + Sync>;

/// A callback that takes no arguments (used for "not found" events).
type NotifyCallback = Box<dyn Fn() + Send + Sync>;

/// Customizable logging callbacks for the token flow.
///
/// Each field is an optional callback. When `None`, a default implementation
/// using [`tracing`] is used. Call [`Logger::init`] to fill in defaults for
/// any `None` fields, or use [`Logger::new`] which calls `init()` automatically.
pub struct Logger {
    /// Called when a cached token's expiration date is known.
    /// Default: `tracing::debug!` with the formatted date.
    pub expire: Option<DateCallback>,

    /// Called when the browser failed to open for the device flow URL.
    /// Default: `tracing::warn!` with the error message.
    pub failed_to_open_browser: Option<ErrorCallback>,

    /// Called when retrieving an access token from the keyring failed.
    /// Default: `tracing::warn!` with the error message.
    pub failed_to_get_access_token_from_keyring: Option<ErrorCallback>,

    /// Called when no access token was found in the keyring (not an error).
    /// Default: `tracing::debug!`.
    pub access_token_is_not_found_in_keyring: Option<NotifyCallback>,

    /// Called when retrieving app configuration from the keyring failed.
    /// Default: `tracing::warn!` with the error message.
    pub failed_to_get_app_from_keyring: Option<ErrorCallback>,

    /// Called when no app configuration was found in the keyring (not an error).
    /// Default: `tracing::debug!`.
    pub app_is_not_found_in_keyring: Option<NotifyCallback>,
}

impl Logger {
    /// Create a new `Logger` with all default callbacks filled in.
    pub fn new() -> Self {
        let mut l = Self {
            expire: None,
            failed_to_open_browser: None,
            failed_to_get_access_token_from_keyring: None,
            access_token_is_not_found_in_keyring: None,
            failed_to_get_app_from_keyring: None,
            app_is_not_found_in_keyring: None,
        };
        l.init();
        l
    }

    /// Fill in default callbacks for any fields that are `None`.
    ///
    /// This matches the Go SDK's `InitLogger()` behavior: fields that already
    /// have a value are left untouched, while `None` fields get a sensible
    /// default that logs via [`tracing`].
    pub fn init(&mut self) {
        if self.expire.is_none() {
            self.expire = Some(Box::new(|ex_date| {
                tracing::debug!(expiration_date = %ex_date, "token expires");
            }));
        }
        if self.failed_to_open_browser.is_none() {
            self.failed_to_open_browser = Some(Box::new(|err| {
                tracing::warn!(error = err, "failed to open browser");
            }));
        }
        if self.failed_to_get_access_token_from_keyring.is_none() {
            self.failed_to_get_access_token_from_keyring = Some(Box::new(|err| {
                tracing::warn!(error = err, "failed to get access token from keyring");
            }));
        }
        if self.access_token_is_not_found_in_keyring.is_none() {
            self.access_token_is_not_found_in_keyring = Some(Box::new(|| {
                tracing::debug!("access token is not found in keyring");
            }));
        }
        if self.failed_to_get_app_from_keyring.is_none() {
            self.failed_to_get_app_from_keyring = Some(Box::new(|err| {
                tracing::warn!(error = err, "failed to get app from keyring");
            }));
        }
        if self.app_is_not_found_in_keyring.is_none() {
            self.app_is_not_found_in_keyring = Some(Box::new(|| {
                tracing::debug!("app is not found in keyring");
            }));
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use chrono::TimeZone;

    use super::*;

    // ---------------------------------------------------------------
    // new() and Default
    // ---------------------------------------------------------------

    #[test]
    fn new_logger_has_all_defaults() {
        let logger = Logger::new();
        assert!(logger.expire.is_some());
        assert!(logger.failed_to_open_browser.is_some());
        assert!(logger.failed_to_get_access_token_from_keyring.is_some());
        assert!(logger.access_token_is_not_found_in_keyring.is_some());
        assert!(logger.failed_to_get_app_from_keyring.is_some());
        assert!(logger.app_is_not_found_in_keyring.is_some());
    }

    #[test]
    fn default_logger_has_all_defaults() {
        let logger = Logger::default();
        assert!(logger.expire.is_some());
        assert!(logger.failed_to_open_browser.is_some());
        assert!(logger.failed_to_get_access_token_from_keyring.is_some());
        assert!(logger.access_token_is_not_found_in_keyring.is_some());
        assert!(logger.failed_to_get_app_from_keyring.is_some());
        assert!(logger.app_is_not_found_in_keyring.is_some());
    }

    // ---------------------------------------------------------------
    // init() fills None fields
    // ---------------------------------------------------------------

    #[test]
    fn init_fills_none_fields() {
        let mut logger = Logger {
            expire: None,
            failed_to_open_browser: None,
            failed_to_get_access_token_from_keyring: None,
            access_token_is_not_found_in_keyring: None,
            failed_to_get_app_from_keyring: None,
            app_is_not_found_in_keyring: None,
        };

        // All fields are None before init.
        assert!(logger.expire.is_none());
        assert!(logger.failed_to_open_browser.is_none());

        logger.init();

        // All fields are Some after init.
        assert!(logger.expire.is_some());
        assert!(logger.failed_to_open_browser.is_some());
        assert!(logger.failed_to_get_access_token_from_keyring.is_some());
        assert!(logger.access_token_is_not_found_in_keyring.is_some());
        assert!(logger.failed_to_get_app_from_keyring.is_some());
        assert!(logger.app_is_not_found_in_keyring.is_some());
    }

    // ---------------------------------------------------------------
    // Custom callback is called
    // ---------------------------------------------------------------

    #[test]
    fn custom_callback_is_called() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let mut logger = Logger::new();
        logger.expire = Some(Box::new(move |_dt| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        let dt = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        (logger.expire.as_ref().unwrap())(dt);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        (logger.expire.as_ref().unwrap())(dt);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn custom_error_callback_receives_message() {
        let captured = Arc::new(std::sync::Mutex::new(String::new()));
        let captured_clone = Arc::clone(&captured);

        let mut logger = Logger::new();
        logger.failed_to_open_browser = Some(Box::new(move |err| {
            *captured_clone.lock().unwrap() = err.to_string();
        }));

        (logger.failed_to_open_browser.as_ref().unwrap())("connection refused");
        assert_eq!(*captured.lock().unwrap(), "connection refused");
    }

    // ---------------------------------------------------------------
    // init() does not overwrite custom callbacks
    // ---------------------------------------------------------------

    #[test]
    fn init_does_not_overwrite_custom_callbacks() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let mut logger = Logger {
            expire: Some(Box::new(move |_dt| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })),
            failed_to_open_browser: None,
            failed_to_get_access_token_from_keyring: None,
            access_token_is_not_found_in_keyring: None,
            failed_to_get_app_from_keyring: None,
            app_is_not_found_in_keyring: None,
        };

        // Fill in defaults for the None fields.
        logger.init();

        // All fields are now Some.
        assert!(logger.failed_to_open_browser.is_some());

        // The custom expire callback should still be our counter.
        let dt = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        (logger.expire.as_ref().unwrap())(dt);
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "custom callback should have been preserved by init()"
        );
    }

    // ---------------------------------------------------------------
    // Default callbacks do not panic
    // ---------------------------------------------------------------

    #[test]
    fn default_callbacks_do_not_panic() {
        let logger = Logger::new();

        let dt = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        (logger.expire.as_ref().unwrap())(dt);
        (logger.failed_to_open_browser.as_ref().unwrap())("test error");
        (logger
            .failed_to_get_access_token_from_keyring
            .as_ref()
            .unwrap())("keyring error");
        (logger
            .access_token_is_not_found_in_keyring
            .as_ref()
            .unwrap())();
        (logger.failed_to_get_app_from_keyring.as_ref().unwrap())("app error");
        (logger.app_is_not_found_in_keyring.as_ref().unwrap())();
    }

    // ---------------------------------------------------------------
    // Calling pattern matches expected usage
    // ---------------------------------------------------------------

    #[test]
    fn calling_pattern_with_if_let() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let logger = Logger {
            expire: None,
            failed_to_open_browser: None,
            failed_to_get_access_token_from_keyring: None,
            access_token_is_not_found_in_keyring: Some(Box::new(move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            })),
            failed_to_get_app_from_keyring: None,
            app_is_not_found_in_keyring: None,
        };

        // This is the calling pattern used by the token manager.
        if let Some(cb) = &logger.access_token_is_not_found_in_keyring {
            cb();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
