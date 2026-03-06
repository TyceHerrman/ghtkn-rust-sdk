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

Tools like cargo-deny, cargo-machete, git-cliff, and jq are installed on demand by the tasks that need them.

### Common Commands

| Command              | Description                                          |
|----------------------|------------------------------------------------------|
| `mise run ci`        | Run all CI checks locally (single OS, not the matrix)|
| `mise run lint`      | Quick lint (clippy + fmt) — fast, offline             |
| `mise run t`         | Run tests                                            |
| `mise run changelog` | Generate CHANGELOG.md via git-cliff                  |
