//! Cross-platform browser opening.
//!
//! Provides a [`Browser`] trait for opening URLs in a web browser, with a
//! [`DefaultBrowser`] implementation that delegates to the [`open`] crate
//! (which handles platform-specific commands internally).
//!
//! The trait allows callers to inject a custom implementation for testing or
//! environments where a real browser is unavailable.

use std::fmt;

/// Errors from browser operations.
#[derive(Debug)]
pub enum BrowserError {
    /// No browser command was found on the system.
    ///
    /// This sentinel is provided for custom [`Browser`] implementations that
    /// can distinguish "not found" from "execution failed". The
    /// [`DefaultBrowser`] always returns [`BrowserError::Failed`] because the
    /// `open` crate does not expose a separate "not found" error.
    NoCommandFound,

    /// The browser command existed but failed to execute.
    Failed(String),
}

impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommandFound => write!(f, "no command found to open the browser"),
            Self::Failed(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for BrowserError {}

/// Trait for opening URLs in a browser.
///
/// This is intentionally synchronous because browser launching is a quick
/// fire-and-forget operation on all platforms.
pub trait Browser: Send + Sync {
    /// Open the given URL in the user's default browser.
    fn open(&self, url: &str) -> Result<(), BrowserError>;
}

/// Default browser implementation using the [`open`] crate.
///
/// The `open` crate handles platform-specific commands internally:
/// - **macOS**: `open`
/// - **Linux**: `xdg-open` / `x-www-browser` / `www-browser`
/// - **Windows**: `ShellExecuteW`
pub struct DefaultBrowser;

impl Browser for DefaultBrowser {
    fn open(&self, url: &str) -> Result<(), BrowserError> {
        open::that(url).map_err(|e| BrowserError::Failed(format!("open browser: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::*;

    // ---------------------------------------------------------------
    // Object safety
    // ---------------------------------------------------------------

    #[test]
    fn default_browser_trait_is_object_safe() {
        // If this compiles, the trait is object-safe and can be used as
        // `dyn Browser` behind a Box or reference.
        fn _assert_object_safe(_b: &dyn Browser) {}
        let browser = DefaultBrowser;
        _assert_object_safe(&browser);
    }

    // ---------------------------------------------------------------
    // Custom implementation
    // ---------------------------------------------------------------

    struct MockBrowser {
        called: Arc<AtomicBool>,
        should_fail: bool,
    }

    impl Browser for MockBrowser {
        fn open(&self, _url: &str) -> Result<(), BrowserError> {
            self.called.store(true, Ordering::SeqCst);
            if self.should_fail {
                Err(BrowserError::NoCommandFound)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn custom_browser_implementation() {
        let called = Arc::new(AtomicBool::new(false));
        let browser = MockBrowser {
            called: Arc::clone(&called),
            should_fail: false,
        };
        let result = browser.open("https://example.com");
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn custom_browser_can_return_no_command_found() {
        let called = Arc::new(AtomicBool::new(false));
        let browser = MockBrowser {
            called: Arc::clone(&called),
            should_fail: true,
        };
        let result = browser.open("https://example.com");
        assert!(result.is_err());
        assert!(called.load(Ordering::SeqCst));

        let err = result.unwrap_err();
        assert!(
            matches!(err, BrowserError::NoCommandFound),
            "expected NoCommandFound, got: {err}"
        );
    }

    // ---------------------------------------------------------------
    // Error variants
    // ---------------------------------------------------------------

    #[test]
    fn browser_error_variants() {
        let not_found = BrowserError::NoCommandFound;
        assert_eq!(
            not_found.to_string(),
            "no command found to open the browser"
        );

        let failed = BrowserError::Failed("open browser: permission denied".into());
        assert_eq!(failed.to_string(), "open browser: permission denied");
    }

    #[test]
    fn browser_error_is_debug() {
        let err = BrowserError::NoCommandFound;
        let debug = format!("{err:?}");
        assert!(debug.contains("NoCommandFound"));

        let err = BrowserError::Failed("test".into());
        let debug = format!("{err:?}");
        assert!(debug.contains("Failed"));
    }

    #[test]
    fn custom_browser_as_boxed_trait_object() {
        let called = Arc::new(AtomicBool::new(false));
        let browser: Box<dyn Browser> = Box::new(MockBrowser {
            called: Arc::clone(&called),
            should_fail: false,
        });
        let result = browser.open("https://github.com");
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }
}
