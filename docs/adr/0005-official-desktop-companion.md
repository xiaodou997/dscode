# ADR 0005: Manage the official Codex desktop configuration

Status: accepted

## Context

The immediate product problem is not a missing coding agent. Users already use
the official Codex application with DouStack's Responses-compatible endpoint,
but manual provider and API-key setup is difficult. Replacing the official app
would also lose closed-source desktop integrations and increase maintenance.

Users want existing official conversations to remain resumable after switching
between an official account and DouStack.

## Decision

DS Code's primary desktop mode is a companion to the installed official Codex
application. It uses the official `~/.codex` data directory in place and may
change only `~/.codex/config.toml`.

Before a change, DS Code renders the merged TOML for review, requires explicit
confirmation, refuses to continue while Codex is running, and snapshots the
previous file under `~/.dscode/backups/official-codex/`. TOML changes are
structured and preserve unrelated settings. A restore command re-applies the
latest snapshot or removes the config if it did not previously exist.

The provider is named `OpenAI`, uses the fixed DouStack endpoint, and references
`DOUSTACK_API_KEY`. The key remains in the operating-system credential store and
is injected only when DS Code launches the official process.

DS Code reads only enough metadata to report whether official authentication is
present. It never writes `auth.json`, session databases, conversation history,
plugins, MCP configuration, computer-use data, or packaged app resources.

The existing isolated CLI mode remains available for diagnostics and limited
operation without the official application.

## Consequences

- Official conversations and desktop capabilities remain available.
- The official application must be launched through DS Code when the DouStack
  provider needs its stored environment credential.
- Configuration switching is reversible and auditable without duplicating the
  complete official data directory.
- Upstream Codex remains an external dependency that must be version-detected
  and compatibility-tested.
