# AI Assistant Guidelines for ghtkn-rust-sdk

This document contains common guidelines for AI assistants working on the ghtkn Rust SDK.

## Language

This project uses **English** for all code comments, documentation, and communication.

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

### Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Common Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Changes that do not affect the meaning of the code
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `test`: Adding missing tests or correcting existing tests
- `chore`: Changes to the build process or auxiliary tools
- `ci`: Changes to CI configuration files and scripts

### Examples

```
feat: add GitHub token management via keyring
fix: handle empty configuration file correctly
docs: add function documentation to keyring module
chore(deps): update dependency aquaproj/aqua-registry to v4.403.0
```

## Code Validation

After making code changes, **always run** the following commands to validate and test:

### Validation (clippy)

```bash
mise run v
```

Runs `cargo clippy -- -D warnings`.

### Testing

```bash
mise run t
```

Runs `cargo test`.

### Formatting

```bash
mise run fmt
```

Runs `cargo fmt -- --check`.

All commands should pass before committing changes.

## Project Structure

```
ghtkn-rust-sdk/
├── src/
│   ├── lib.rs          # Crate root, public re-exports
│   ├── api.rs          # Token manager and public Client API
│   ├── browser.rs      # Cross-platform browser opening
│   ├── config.rs       # YAML configuration management
│   ├── deviceflow.rs   # OAuth device flow (RFC 8628)
│   ├── error.rs        # Error types (thiserror)
│   ├── github.rs       # GitHub REST API client
│   ├── keyring.rs      # System keyring integration
│   └── log.rs          # Customizable logging callbacks
├── tests/
│   └── integration.rs  # Cross-module integration tests
├── examples/
│   ├── simple.rs       # Basic usage example
│   ├── custom_log.rs   # Custom logging callbacks
│   └── custom_ui.rs    # Custom device code UI
├── Cargo.toml
├── mise.toml           # Task runner configuration
└── AGENTS.md           # AI assistant guidelines (this file)
```

## Module Responsibilities

### api

High-level `Client` for GitHub token management. Orchestrates the full flow: config -> keyring -> device flow -> GitHub API -> keyring store. Also provides `TokenSource` for cached, on-demand token retrieval.

### config

Configuration management including reading, parsing, and validating `.ghtkn.yaml` files. Handles app selection by name, git owner, or default (first app).

### keyring

Token persistence and caching using the system keyring. Stores `AccessToken` as JSON with the same schema as the Go SDK for cross-tool compatibility.

### deviceflow

OAuth device authorization grant flow (RFC 8628). Handles the full flow: request device code, display user code, open browser, poll for access token.

### github

GitHub REST API client for authenticated requests. Currently provides `GET /user` for retrieving the authenticated user's login.

### browser

Cross-platform browser opening via the `open` crate. Provides a `Browser` trait for testability.

### log

Customizable logging callbacks for the token flow. Mirrors the Go SDK's `Logger` struct with optional callback fields.

### error

Central error type using `thiserror`. All modules use `Error` and `Result<T>` from this module.

## Testing

### Test Framework Guidelines

- Use standard Rust testing with `#[test]` and `#[tokio::test]`
- Use `pretty_assertions` for readable diffs in assertions
- Use `wiremock` for HTTP mocking in async tests
- Use mock backends (trait-based) for keyring testing
- Do **NOT** use testify-style assertion macros
- Unit tests go in `#[cfg(test)] mod tests` within the source file
- Integration tests go in the `tests/` directory

### Running Tests

- Run all tests: `mise run t` or `cargo test`
- Run a specific test: `cargo test test_name`
- Run tests for a specific module: `cargo test config::tests`

## Dependencies

This project uses:

- **tokio** for async runtime
- **reqwest** for HTTP client
- **serde** / **serde_json** / **serde_yaml** for serialization
- **keyring** for system keyring access
- **chrono** for date/time handling
- **thiserror** for error types
- **tracing** for structured logging
- **open** for cross-platform browser opening
- **wiremock** for HTTP mocking (dev)
- **tempfile** for temp files in tests (dev)
- **pretty_assertions** for readable test diffs (dev)

## Code Style Guidelines

1. Follow standard Rust conventions (`rustfmt`)
2. Use meaningful variable and function names
3. Add doc comments (`///`) for all public functions, types, and constants
4. Keep functions focused and small
5. Handle errors with the `?` operator and `thiserror`
6. Use `tracing` for logging
7. Always end files with a newline character
8. Run clippy before committing

## File Naming Conventions

- Source files: `snake_case` (e.g., `device_flow.rs`)
- Unit tests: `#[cfg(test)] mod tests` within the source file
- Integration tests: `tests/` directory (e.g., `tests/integration.rs`)

## Error Handling

- Use `thiserror` for error type definitions
- Use `?` for error propagation
- Add context to errors with descriptive messages
- Use `tracing::warn!` for non-fatal errors (e.g., keyring store failures)

## Pull Request Process

1. Create a feature branch from `main`
2. Make changes and ensure `mise run v`, `mise run t`, and `mise run fmt` pass
3. Write clear commit messages following Conventional Commits
4. Create PR with descriptive title and body
5. Wait for CI checks to pass

## Important Commands

```bash
# Run clippy linter
mise run v

# Run tests
mise run t

# Check formatting
mise run fmt

# Build the project
mise run b

# Generate documentation
mise run d
```

## Cross-Tool Compatibility

This SDK stores tokens in the system keyring using the same JSON schema and service key as the Go SDK (`github.com/suzuki-shunsuke/ghtkn`). The `AccessToken` struct serializes to:

```json
{
  "access_token": "ghu_...",
  "expiration_date": "2024-01-01T00:00:00Z",
  "login": "username"
}
```

Any changes to the keyring module must preserve this format for interoperability.
