# ghtkn

Rust port of [ghtkn](https://github.com/suzuki-shunsuke/ghtkn) (Go) — GitHub token management with OAuth device flow, keyring caching, and config-driven app selection.

[![Crates.io](https://img.shields.io/crates/v/ghtkn)](https://crates.io/crates/ghtkn)
[![docs.rs](https://img.shields.io/docsrs/ghtkn)](https://docs.rs/ghtkn)
[![License: MIT](https://img.shields.io/crates/l/ghtkn)](https://github.com/TyceHerrman/ghtkn-rust-sdk/blob/main/LICENSE)

## Install

```sh
cargo add ghtkn
cargo add tokio --features macros,rt-multi-thread
```

## Configuration

Create a config file at `~/.config/ghtkn/ghtkn.yaml` (Linux/macOS) or `%APPDATA%\ghtkn\ghtkn.yaml` (Windows):

```yaml
apps:
  - name: my-app
    client_id: Iv1.xxxxxxxxxxxxxxxx
```

Each app entry requires a `name` and `client_id` from a [GitHub App](https://docs.github.com/en/apps/creating-github-apps). Optionally add `git_owner` to scope an app to a specific GitHub organization.

## Usage

```rust
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
```

## Features

- **OAuth device flow** — authenticate via browser using the [device authorization grant](https://datatracker.ietf.org/doc/html/rfc8628) (RFC 8628)
- **Keyring caching** — tokens are stored in the system keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service) and reused across sessions
- **Multi-app config** — define multiple GitHub Apps in `ghtkn.yaml` and select by name or `git_owner`
- **Silent token retrieval** — `token_or_none()` returns a cached token without prompting, useful for CLI tools that want optional authentication

## API Documentation

Full API docs are available on [docs.rs](https://docs.rs/ghtkn).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, commands, and release workflow.

## License

MIT
