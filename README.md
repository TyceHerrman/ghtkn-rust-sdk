# ghtkn-rust-sdk

Rust port of ghtkn-go-sdk — GitHub token management via OAuth device flow.

## Development

### Prerequisites

- **Rust** via [rustup](https://rustup.rs/) with clippy and rustfmt components:
  ```sh
  rustup component add clippy rustfmt
  ```
- **[mise](https://mise.jdx.dev/)** for task running and cargo tool provisioning

### Setup

```sh
mise install
```

This provisions cargo-deny, cargo-machete, cargo-release, git-cliff, jq, and other helpers.

### Common Commands

| Command              | Description                                            |
|----------------------|--------------------------------------------------------|
| `mise run ci`        | Run all CI checks locally (single OS, not the matrix)  |
| `mise run lint`      | Quick lint (clippy + fmt)                              |
| `mise run t`         | Run tests                                              |
| `mise run v`         | Run clippy with `-D warnings` (warnings are errors)    |
| `mise run f`         | Check formatting via `cargo fmt --check`               |
| `mise run changelog` | Generate CHANGELOG.md via git-cliff                    |
