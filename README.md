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

### Releasing

Releases are managed by [cargo-release](https://github.com/crate-ci/cargo-release) and published to crates.io via GitHub Actions.

**One-time setup:** Add a `CARGO_REGISTRY_TOKEN` secret to the GitHub repository ([crates.io tokens](https://crates.io/settings/tokens) → scope: `publish-new` for first publish, `publish-update` thereafter).

```sh
# Dry run (default, shows what would happen)
cargo release <level>

# Actually release (generates changelog, bumps version, tags, pushes)
cargo release <level> --execute
```

Where `<level>` is `patch`, `minor`, or `major`. To release the current version as-is (e.g. for the first publish), omit the level:

```sh
cargo release --execute
```

The `cargo release` command will:
1. Bump the version in `Cargo.toml` (if a level is given)
2. Generate/update `CHANGELOG.md` via git-cliff
3. Create a release commit
4. Create a `v{version}` git tag
5. Push the commit and tag to origin

CI then picks up the tag and runs tests across all platforms before publishing to crates.io.
