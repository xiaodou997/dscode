# DS Code

DS Code is the coding product from DouStack. Its desktop app configures the
stock OpenAI Codex application for DouStack while preserving the user's
official Codex conversations, login, plugins, and computer-use capabilities.

The product name is **DouStack Code**, the short display name is **DS Code**, and
the command is `dscode`.

## Desktop MVP

- Detect the installed official Codex desktop app, login state, running state,
  version, and local CLI compatibility.
- Configure the fixed DouStack endpoint `https://miao.313619.xyz` without asking
  the user to enter a URL.
- Merge an `OpenAI`-named DouStack provider into `~/.codex/config.toml` without
  discarding unrelated settings.
- Preview every configuration change, require confirmation, back up the prior
  file under `~/.dscode/backups/official-codex/`, and restore it on demand.
- Save the API key in the operating system credential manager and inject
  `DOUSTACK_API_KEY` when DS Code launches the official application.
- Keep official conversations in `~/.codex`; DS Code never rewrites the login
  file, sessions, plugins, or computer-use data.

Quit the official Codex app before applying or restoring configuration. The
desktop disables those actions while it detects Codex running.

## Run the desktop

```bash
cd apps/dscode-desktop
bun install
bun run tauri dev
```

The browser-only UI preview is available at `http://127.0.0.1:1421/` while the
development process is running. Actual Codex detection, credential storage,
configuration changes, and launching require the Tauri window.

## CLI fallback

The original `dscode` CLI remains an isolated fallback and compatibility tool.
It runs the installed `codex` binary with `CODEX_HOME=~/.dscode/codex`, so it
does not share sessions with the official desktop companion mode.

```bash
cargo test --workspace
cargo run -p dscode-cli -- config
cargo run -p dscode-cli -- doctor
cargo run -p dscode-cli -- init
cargo run -p dscode-cli -- login
cargo run -p dscode-cli -- runtime status
cargo run -p dscode-cli -- runtime probe
cargo run -p dscode-cli -- runtime contract
```

For development, an environment credential can launch Codex directly:

```bash
export DOUSTACK_API_KEY="..."
cargo run -p dscode-cli
```

For normal use, store the credential in macOS Keychain, Windows Credential
Manager, or the Linux Secret Service:

```bash
cargo run -p dscode-cli -- login
cargo run -p dscode-cli
cargo run -p dscode-cli -- logout
```

Persist CLI fallback settings in `~/.dscode/config.toml`:

```bash
cargo run -p dscode-cli -- config set model MODEL
cargo run -p dscode-cli -- config set base-url https://miao.313619.xyz
cargo run -p dscode-cli -- config show
```

Forward normal Codex commands and flags after DS Code options:

```bash
cargo run -p dscode-cli -- --model MODEL exec "explain this repository"
```

Inspect the installed Codex runtime without making a model request:

```bash
cargo run -p dscode-cli -- runtime status
cargo run -p dscode-cli -- runtime probe
cargo run -p dscode-cli -- runtime contract
```

`runtime probe` performs the production stdio `app-server` initialization
handshake and verifies that the server uses DS Code's isolated `CODEX_HOME`.
`runtime contract` starts the same configured DouStack provider and validates
the stable, read-only provider-capability, model-list, and session-list RPCs. It
does not start a model turn or consume model tokens.
The currently tested Codex version is `0.146.0`; other versions are reported as
untested until their protocol schema and behavior pass the compatibility suite.

## Data boundaries

Official Codex continues to own:

```text
~/.codex/
├── config.toml   DS Code may merge the DouStack provider after confirmation
├── auth.json     read-only to DS Code
└── ...           conversations, state, plugins, and tools remain untouched
```

DS Code owns:

```text
~/.dscode/
├── codex/       isolated CLI fallback runtime
├── imports/     staging area for future explicit imports
├── backups/     official-config and migration backups
├── logs/        DS Code logs
└── cache/       Download and compatibility caches
```

The CLI fallback layout can be overridden with `DSCODE_HOME` or `--home`. The
API key is not stored in either directory.

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cd apps/dscode-desktop && bun run check && bun run build
```

## Roadmap

1. Complete interactive acceptance tests for config switching and rollback.
2. Package and sign the macOS desktop application.
3. Add DouStack Chat and Images from the existing website implementation.
4. Expand the standalone coding fallback and app-server integration.

See [ADR 0001](docs/adr/0001-stock-codex-runtime.md) for the runtime architecture,
[ADR 0002](docs/adr/0002-system-credential-store.md) for credential handling,
and [ADR 0003](docs/adr/0003-runtime-compatibility.md) for app-server compatibility.
The boundary between read-only and live provider checks is recorded in
[ADR 0004](docs/adr/0004-read-only-provider-contract.md).
The official desktop companion and shared-session decision is recorded in
[ADR 0005](docs/adr/0005-official-desktop-companion.md).
The staged delivery plan is tracked in [the roadmap](docs/roadmap.md).
