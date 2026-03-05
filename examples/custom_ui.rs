//! Example: custom device code UI and browser implementation.
//!
//! Shows how to implement the [`DeviceCodeUI`] and [`Browser`] traits for
//! environments where the default terminal output or system browser are
//! unsuitable (e.g. a GUI application or a headless server).
//!
//! ```sh
//! cargo run --example custom_ui
//! ```

use chrono::{DateTime, Utc};

use ghtkn::browser::BrowserError;
use ghtkn::deviceflow::DeviceCodeResponse;
use ghtkn::{Browser, Client, DeviceCodeUI, InputGet};

/// A device code UI that formats output as JSON for machine consumption.
struct JsonDeviceCodeUI;

impl DeviceCodeUI for JsonDeviceCodeUI {
    fn show(
        &self,
        device_code: &DeviceCodeResponse,
        expiration_date: DateTime<Utc>,
    ) -> ghtkn::Result<()> {
        let json = serde_json::json!({
            "user_code": device_code.user_code,
            "verification_uri": device_code.verification_uri,
            "expires_at": expiration_date.to_rfc3339(),
        });
        eprintln!("{}", serde_json::to_string_pretty(&json).unwrap());
        Ok(())
    }
}

/// A browser implementation that only prints the URL instead of opening it.
/// Useful for headless environments or SSH sessions.
struct PrintOnlyBrowser;

impl Browser for PrintOnlyBrowser {
    fn open(&self, url: &str) -> Result<(), BrowserError> {
        eprintln!("Please open this URL in your browser: {url}");
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let mut client = Client::new();
    client.set_device_code_ui(Box::new(JsonDeviceCodeUI));
    client.set_browser(Box::new(PrintOnlyBrowser));

    let input = InputGet::default();

    match client.get(&input).await {
        Ok((token, app)) => {
            println!("App:   {}", app.name);
            println!("User:  {}", token.login);
            println!(
                "Token: {}...",
                &token.access_token[..token.access_token.len().min(8)]
            );
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}
