# DS Code engineering guide

## Product boundary

DS Code is a DouStack companion for the stock OpenAI Codex desktop application
and CLI. Keep DouStack authentication, provider configuration, diagnostics,
and product UX in this repository. Do not copy code from CodexPlusPlus or patch
the Codex desktop application's packaged resources.

## Security

- Never commit or log API keys.
- Keep credentials out of generated TOML and process arguments.
- Store the DouStack key in the OS credential store and inject it only into the
  official Codex process launched by DS Code.
- Desktop configuration changes may update only `~/.codex/config.toml`, after
  an explicit preview/confirmation and a backup under `~/.dscode/backups/`.
- Preserve unrelated TOML settings and provide a one-click restore path.
- Never modify `~/.codex/auth.json`, session databases, conversation history,
  plugins, or computer-use data.
- The official companion mode uses `~/.codex` so existing conversations remain
  resumable. The CLI fallback may keep an isolated runtime under
  `~/.dscode/codex`.

## Architecture

- `crates/dscode-core` owns Codex provider configuration and launch planning.
- `crates/dscode-credentials` owns operating-system credential integration.
- `crates/dscode-runtime` owns Codex version policy, app-server lifecycle, and
  protocol contract checks.
- `apps/dscode-cli` owns terminal input/output and process lifecycle.
- `apps/dscode-desktop` owns the Tauri desktop shell, official-app detection,
  confirmation flows, and visual status dashboard.
- Keep the core interface small and test behavior through that interface.
- Treat Codex as a versioned external runtime. Avoid source forks unless a
  required capability cannot be implemented through configuration, SDK, or
  app-server.

## Verification

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
