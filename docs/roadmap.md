# DS Code roadmap

This roadmap keeps DS Code on the stock Codex runtime while product-specific
configuration, credentials, desktop UX, and distribution stay in this
repository.

## Milestone 1: local product foundation

- Persist the DouStack endpoint and preferred model in `~/.dscode/config.toml`.
- Store the API key in the operating system credential manager.
- Add `dscode login`, `dscode logout`, and configuration commands.
- Resolve credentials without putting secrets in config files or arguments.
- Verify session creation and `dscode resume --last` under the isolated home.

Acceptance criteria:

- A new user can run `dscode login` followed by `dscode` without exporting an
  environment variable.
- `DOUSTACK_API_KEY` remains an explicit override for development and CI.
- No credential appears in `~/.dscode/config.toml`, logs, diagnostics, or
  process arguments.
- Unit tests and a local smoke test cover configuration and credential-source
  precedence.

## Milestone 2: managed Codex runtime

- Detect and report the installed Codex version and supported app-server API.
- Define a tested Codex version range and reject incompatible versions clearly.
- Add managed installation and updates without modifying official Codex data.
- Run provider contract checks for streaming, tools, cancellation, compaction,
  and session resume before publishing a release.

## Milestone 3: desktop MVP

- Create a Tauri 2 desktop application backed by `codex app-server` over stdio.
- Reuse the core configuration, data-layout, and credential modules.
- Implement project selection, chat streaming, session history, approvals,
  command output, and file diffs.
- Pin generated app-server schemas to the bundled and tested Codex version.

## Milestone 4: distribution and migration

- Package signed macOS builds, then add Windows and Linux builds.
- Add update channels, release checksums, and rollback support.
- Import official Codex conversations through an explicit read-only workflow.
- Back up imported data and never import official account configuration.
