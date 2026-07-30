# DS Code

DS Code is the coding product from DouStack. It launches the stock OpenAI Codex
runtime with a DouStack model provider, so users do not need to edit Codex TOML
or sign in with an official OpenAI account.

The product name is **DouStack Code**, the short display name is **DS Code**, and
the command is `dscode`.

## Phase-one scope

- Use the installed stock `codex` binary as the agent runtime.
- Inject the DouStack provider through Codex command-line configuration.
- Read the API key from `DOUSTACK_API_KEY`; never place it in arguments or TOML.
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
```

To launch Codex after supplying a DouStack key:

```bash
export DOUSTACK_API_KEY="..."
cargo run -p dscode-cli
```

Forward normal Codex commands and flags after DS Code options:

```bash
cargo run -p dscode-cli -- --model MODEL exec "explain this repository"
```

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

Override the location with `DSCODE_HOME` or `--home`. DS Code does not read or
write `~/.codex` during normal operation.

## Roadmap

1. Validate the DouStack Responses API contract and model catalog.
2. Add OS credential storage and a guided login flow.
3. Add managed Codex installation and compatibility tests.
4. Build the DS Code desktop client on Codex app-server.

See [ADR 0001](docs/adr/0001-stock-codex-runtime.md) for the initial architecture.
