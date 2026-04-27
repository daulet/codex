# Codex CLI Fork

This is a fork of [OpenAI Codex CLI](https://github.com/openai/codex) with custom features. Fork baseline: [openai/codex `rust-v0.125.0`](https://github.com/openai/codex/releases/tag/rust-v0.125.0). Everything not called out below should be assumed to follow upstream Codex behavior and documentation.

## Install

### macOS

```shell
brew install daulet/tap/codex
```

### Debian/Ubuntu

```shell
curl -fsSL https://github.com/daulet/codex/releases/latest/download/install-deb.sh | sh
```

### From Source

```shell
git clone https://github.com/daulet/codex.git
cd codex/codex-rs
cargo build --release --bin codex
./target/release/codex
```

## Fork Features

- Persistent `/side` conversations: side discussions are saved as real conversation branches instead of temporary scratch sessions.
- Branch-aware `/tree`: the tree view shows side discussions alongside normal conversation branches and can resume them.
- Branch persistence metadata: side branches keep parent thread and parent turn linkage so conversation history is no longer modeled as one flat prompt/response list.
- Fork release automation: GitHub Actions builds macOS and Debian artifacts, publishes GitHub Releases, and updates `daulet/tap/codex`.

## Upstream Codex

For general Codex usage, authentication, configuration, IDE integrations, and development docs, use the upstream resources:

- [OpenAI Codex repository](https://github.com/openai/codex)
- [Codex documentation](https://developers.openai.com/codex)
- [Install and build notes](docs/install.md)
