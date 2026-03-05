//! Example: custom logger callbacks.
//!
//! Shows how to override individual logging callbacks on the [`Logger`] to
//! integrate with your own logging or monitoring infrastructure.
//!
//! ```sh
//! cargo run --example custom_log
//! ```

use ghtkn::{Client, InputGet, Logger};

#[tokio::main]
async fn main() {
    // Start with a logger that has no defaults filled in.
    let mut logger = Logger {
        expire: Some(Box::new(|expiration_date| {
            eprintln!("[custom] cached token expires at {expiration_date}");
        })),
        failed_to_open_browser: Some(Box::new(|err| {
            eprintln!("[custom] browser error (non-fatal): {err}");
        })),
        failed_to_get_access_token_from_keyring: Some(Box::new(|err| {
            eprintln!("[custom] keyring read error (non-fatal): {err}");
        })),
        access_token_is_not_found_in_keyring: Some(Box::new(|| {
            eprintln!("[custom] no cached token found, starting device flow");
        })),
        failed_to_get_app_from_keyring: None,
        app_is_not_found_in_keyring: None,
    };

    // Fill in defaults for any fields left as None.
    logger.init();

    let mut client = Client::new();
    client.set_logger(logger);

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
