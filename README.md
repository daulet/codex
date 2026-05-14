# Codex CLI Fork

This is a fork of [OpenAI Codex CLI](https://github.com/openai/codex) with custom features. Fork baseline: [openai/codex `rust-v0.130.0`](https://github.com/openai/codex/releases/tag/rust-v0.130.0). Everything not called out below should be assumed to follow upstream Codex behavior and documentation.

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

## Unique Fork Features

### Conversation Branching

- Persistent `/side` conversations: side discussions are saved as real conversation branches instead of temporary scratch sessions.
- Branch-aware `/tree`: the tree view shows side discussions alongside normal conversation branches and can resume them.
- Branch persistence metadata: side branches keep parent thread and parent turn linkage so conversation history is no longer modeled as one flat prompt/response list.

### Copy and UI

- Transcript message copy: copy selected user prompts or assistant responses from the transcript without terminal line-wrapping artifacts. This is separate from upstream `/copy`, which only copies the latest assistant response.
- Fork status indicator: the status line shows `[FORK daulet/codex]` alongside the model and reasoning effort so fork builds are easy to distinguish from upstream builds.

### Distribution and Operations

- Fork release automation: GitHub Actions releases each new `main` commit, builds macOS and Debian artifacts, publishes GitHub Releases, and updates `daulet/tap/codex`.
- Fork package channels: macOS installs through `brew install daulet/tap/codex`; Debian and Ubuntu install through the release-hosted one-liner above.
- Additional tool usage telemetry: configured OpenTelemetry deployments receive structured tool-result events with tool metadata, duration, output size, and file-edit churn.

## Upstream Codex

For general Codex usage, authentication, configuration, IDE integrations, and development docs, use the upstream resources:

- [OpenAI Codex repository](https://github.com/openai/codex)
- [Codex documentation](https://developers.openai.com/codex)
- [Install and build notes](docs/install.md)
