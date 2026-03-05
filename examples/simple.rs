//! Minimal example: get a GitHub token using all defaults.
//!
//! Reads the config from the platform-specific default location, uses the
//! system keyring, the default browser, and the built-in terminal UI.
//!
//! ```sh
//! cargo run --example simple
//! ```

use ghtkn::{Client, InputGet};

#[tokio::main]
async fn main() {
    let client = Client::new();
    let input = InputGet::default();

    match client.get(&input).await {
        Ok((token, app)) => {
            println!("App:   {}", app.name);
            println!("User:  {}", token.login);
            println!(
                "Token: {}...",
                &token.access_token[..token.access_token.len().min(8)]
            );
            println!("Expires: {}", token.expiration_date);
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}
