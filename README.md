# DS Code

DS Code is the coding product from DouStack. It launches the stock OpenAI Codex
runtime with a DouStack model provider, so users do not need to edit Codex TOML
or sign in with an official OpenAI account.

The product name is **DouStack Code**, the short display name is **DS Code**, and
the command is `dscode`.

## Phase-one scope

- Use the installed stock `codex` binary as the agent runtime.
- Inject the DouStack provider through Codex command-line configuration.
- Store the API key in the operating system credential manager, with
  `DOUSTACK_API_KEY` as an explicit development override.
- Provide configuration preview and local diagnostics.
- Leave the user's existing `~/.codex` configuration unchanged.
- Store DS Code runtime data under `~/.dscode/` and set the child Codex
  `CODEX_HOME` to `~/.dscode/codex`.

The default endpoint is `https://miao.313619.xyz`. Override it with
`DOUSTACK_BASE_URL` or `--base-url` if the deployed Responses API root differs.

## Development

```bash
cargo test --workspace
cargo run -p dscode-cli -- config
cargo run -p dscode-cli -- doctor
cargo run -p dscode-cli -- init
cargo run -p dscode-cli -- login
cargo run -p dscode-cli -- runtime status
cargo run -p dscode-cli -- runtime probe
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

Persist non-sensitive settings in `~/.dscode/config.toml`:

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
```

`runtime probe` performs the production stdio `app-server` initialization
handshake and verifies that the server uses DS Code's isolated `CODEX_HOME`.
The currently tested Codex version is `0.146.0`; other versions are reported as
untested until their protocol schema and behavior pass the compatibility suite.

## Data directory

`dscode init` creates this isolated layout:

```text
~/.dscode/
├── codex/       Codex sessions, SQLite state, logs, and runtime files
├── imports/     Staging area for a future official-data importer
├── backups/     Import and migration backups
├── logs/        DS Code logs
└── cache/       Download and compatibility caches
```

Override the location with `DSCODE_HOME` or `--home`. The API key is not stored
in this directory. DS Code does not read or write `~/.codex` during normal
operation.

## Roadmap

1. Validate the DouStack Responses API contract and model catalog.
2. Add OS credential storage and a guided login flow.
3. Add managed Codex installation and compatibility tests.
4. Build the DS Code desktop client on Codex app-server.

See [ADR 0001](docs/adr/0001-stock-codex-runtime.md) for the runtime architecture,
[ADR 0002](docs/adr/0002-system-credential-store.md) for credential handling,
and [ADR 0003](docs/adr/0003-runtime-compatibility.md) for app-server compatibility.
The staged delivery plan is tracked in [the roadmap](docs/roadmap.md).
