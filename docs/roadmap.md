# DS Code roadmap

This roadmap keeps DS Code on the stock Codex runtime while product-specific
configuration, credentials, desktop UX, and distribution stay in this
repository.

## Current status

- Milestone 1 implementation is complete; Keychain login and session resume
  still require interactive user acceptance testing.
- Milestone 2 now detects Codex versions, maintains a long-lived app-server
  stdio client, and checks the stable read-only provider contract. Managed
  installation and live model-turn checks remain open.
- Milestone 3 has a working Tauri dashboard and official Codex configuration
  manager. Interactive configuration switching and packaged builds remain open.
- Milestones 4 and 5 have not started.

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

- [x] Detect and report the installed Codex version and app-server handshake.
- [x] Define an exact tested Codex version and report untested versions clearly.
- [x] Pin the stable app-server schema for the tested Codex version.
- [x] Validate provider capabilities, model discovery, and session discovery
  through stable read-only app-server RPCs.
- Add managed installation and updates without modifying official Codex data.
- Run provider contract checks for streaming, tools, cancellation, compaction,
  and session resume before publishing a release.

## Milestone 3: official desktop companion

- [x] Create a Tauri 2 desktop application and operational status dashboard.
- [x] Detect the official app, login presence, app version, and running state.
- [x] Preview and merge the fixed DouStack provider into official Codex TOML.
- [x] Back up and restore `~/.codex/config.toml` without modifying other data.
- [x] Store the DouStack key outside TOML and inject it when launching Codex.
- Run interactive acceptance tests against official and DouStack accounts.
- Package and sign the macOS application.

## Milestone 4: DouStack product surfaces

- Add Chat and Images using the existing DouStack website API implementation.
- Add a limited coding mode for machines without the official Codex app.
- Build project selection, streaming, approvals, command output, and file diffs
  on the tested Codex app-server contract where appropriate.

## Milestone 5: distribution and migration

- Package signed macOS builds, then add Windows and Linux builds.
- Add update channels, release checksums, and rollback support.
- Add managed official Codex installation/download guidance.
- Preserve official conversations in place; add imports only for explicitly
  selected external DS Code data and never import account configuration.
