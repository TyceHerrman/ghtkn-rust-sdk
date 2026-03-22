# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-03-22

### Bug Fixes

- Use AsRef<Path> for read() and handle empty path ([18aab95](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/18aab951be46b29b510bf69da1cb5028890890e9))
- Address code review parity gaps with Go SDK ([1893231](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/1893231d85fefa3b7d72de6bd4e9bae1b640b702))
- Address PR review feedback ([6f7e812](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/6f7e812f9a33beb2f9778e7fd80e2ff4cc8a675d))
- Lower token_or_none() log level from warn to debug ([9fce0c9](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/9fce0c96ce42f40a525d472e47da4b9c2bf2d705))
- Address PR review feedback for StoreToken handling ([3b3ac9c](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/3b3ac9c6bec37c22eb0d83a45c7fa391f3416a83))
- Address PR review feedback for log level, tests, and perf ([f7f3e67](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/f7f3e67618565aecd4fdc3d824aea1eef62863ae))
- Allow ISC license for reqwest 0.13 dependency chain ([24710fd](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/24710fdf1982222986b70dafdd7d861292ba31cb))
- Allow CDLA-Permissive-2.0 and OpenSSL licenses for reqwest 0.13 deps ([1b65e68](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/1b65e68d62b98a6fb7fbed0e184c8ecb3c0249cb))
- *(deps)* Update rust crate reqwest to 0.13 ([459a8f3](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/459a8f393331744beded7745377bda1cb7f360a7))
- Add explicit tool inputs for taiki-e/install-action ([51e01a9](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/51e01a9598620cf038d6b5754800387c3d90b2d0))
- Switch keyring from sync to async secret-service backend ([fc40370](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/fc4037000ee10c7ccd75d1a8a68e74d101db27e6))
- *(deps)* Update rustls-webpki to 0.103.10 (RUSTSEC-2026-0049) ([9f207b2](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/9f207b2810061df1f4f7bc5b3b220462202f33ed))

### CI

- Fix CI failures by installing libdbus-1-dev and using stable Rust ([a0279c2](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/a0279c280fde8ddbf597223b12d67ef977f1df6a))
- Add cargo-deny, edition 2024, MSRV 1.88, and CI lint job ([1065b4f](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/1065b4f6b1666eb5e60f77166e42e3a1636c6d11))
- Fix formatting and replace broken cargo-msrv action ([f9a5d50](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/f9a5d50db9c76367847b332864876332feac7db8))
- Pin action refs and deny wildcard versions ([14ffe24](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/14ffe2451a1f372fcbbd25438037c9b1ae9c0577))
- Replace cmdx with mise task runner ([708efe0](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/708efe0816c3f3d7186d63c72d153ee936af67d3))
- Add mise-managed Rust toolchain and enable Claude Code plugins ([5efab43](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/5efab438d7ba35bb6e121dafdc24202e38ef3e6c))
- Add fnox config and ignore local overrides ([32cd505](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/32cd5057c1edb1408626e7acb80cb9e061c967db))
- Improve project setup inspired by mise patterns ([e04a6da](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/e04a6dab6838fb97048043705b2f939c574530e8))
- Add crates.io publish workflow and cargo-release config ([84b446e](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/84b446ea55e5814a95ac4ab3e7ce4414e36e28bf))

### Documentation

- Add vet/fmt aliases to README and fix setup description ([162b81c](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/162b81c1b9fdeaeee03ad2faadd271f7350cd235))

### Features

- Add project foundation and config module ([3690ff6](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/3690ff6a2069cffcd2c0ed4b8fb63b8496eaf81c))
- Add keyring module for token caching ([53a4c6b](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/53a4c6b78e5fc06b6ac7cccfc6d8e108ea2b7d10))
- Add keyring module for token caching ([ce092d3](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/ce092d339c8d762b9154de5adba5a06934d8759f))
- Add browser and log modules ([d3473eb](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/d3473ebbec10ded99d1c6218ec96a70ec5e37f98))
- Add device flow and GitHub API modules ([476ac0d](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/476ac0d92e5da63d8b65ddcec2466e27eb6d221e))
- Add token manager, public API, and examples ([ae6bef6](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/ae6bef66a4689335fa4b3b123f3296cd2157eb3f))
- Add CI workflow, AGENTS.md, cmdx config, and integration tests ([13861f6](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/13861f660a6383107059fe1c33f2949475c35632))
- Add Client::token_source() convenience method ([62548e5](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/62548e555bba4aef9633ca5adb351c63ac518bcd))
- Add base URL testability, StoreToken recovery, and token_or_none API ([d01a37e](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/d01a37ed1ac464941514af52d4c0db47e959777c))

### Miscellaneous

- *(deps)* Update actions/checkout action to v6 ([4d8e6cc](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/4d8e6ccaf8269654e14397c3137171a1655efcdc))
- *(deps)* Update taiki-e/install-action digest to 854cac6 ([fe57901](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/fe579016793c7cb06fe6d4124f453997cea19565))
- Add greptile API key secret ([7ba9563](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/7ba956328e45efc7bd60de9f5a53766cfd82f10c))
- Release v0.1.0 ([22280fc](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/22280fc992243e67c7e5f31cd5aecc2e29781340))

### Testing

- Use separate MockServers for github and api base URLs ([76f231d](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/76f231d34bb0b6a4c5182bfee61493db9b4eac9e))
- Use make_test_client in token_or_none error test ([22327bd](https://github.com/TyceHerrman/ghtkn-rust-sdk/commit/22327bd1c0136d373ed6ad69cbf08a446f9d2adb))

