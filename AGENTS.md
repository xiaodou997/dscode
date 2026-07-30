# DS Code engineering guide

## Product boundary

DS Code is a DouStack launcher and client for the stock OpenAI Codex runtime.
Keep DouStack authentication, provider configuration, diagnostics, and product
UX in this repository. Do not copy code from CodexPlusPlus or patch the Codex
desktop application's packaged resources.

## Security

- Never commit or log API keys.
- Keep credentials out of generated TOML and process arguments.
- Prefer OS credential storage; environment-based credentials are the phase-one
  fallback.
- Do not overwrite a user's existing Codex configuration.
- Normal operation must keep Codex runtime data under `~/.dscode/codex`.
- Treat `~/.codex` as read-only if an explicit import feature is added later.

## Architecture

- `crates/dscode-core` owns Codex provider configuration and launch planning.
- `crates/dscode-credentials` owns operating-system credential integration.
- `crates/dscode-runtime` owns Codex version policy and app-server probing.
- `apps/dscode-cli` owns terminal input/output and process lifecycle.
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
