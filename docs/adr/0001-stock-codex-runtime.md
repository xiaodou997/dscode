# ADR 0001: Use the stock Codex runtime

Status: accepted for the CLI fallback; desktop data-layout decision superseded
by ADR 0005

## Context

DouStack already exposes a model endpoint that users can configure in Codex.
The current user problem is setup complexity: provider TOML, API-key handling,
model selection, and diagnosis require manual work.

Reimplementing the Codex agent would duplicate its sandbox, approvals, tools,
sessions, compaction, MCP, and repository workflow. Maintaining a long-lived
source fork would also make upstream upgrades expensive.

## Decision

DS Code will treat the official Codex binary as a versioned external runtime.
The CLI fallback launches that binary with a user-defined `doustack` model
provider and an environment-backed credential. The child process receives an
isolated `CODEX_HOME` at `~/.dscode/codex`.

The primary desktop product instead acts as a companion to the installed
official application and preserves its sessions in `~/.codex`. That mode and
its strictly limited config mutation policy are defined in ADR 0005.

The desktop phase will use Codex app-server over its supported stdio transport.
Its generated schema must be pinned to the bundled/tested Codex version. The
experimental WebSocket transport will not be used for production.

DS Code will fork Codex only if a required product capability cannot be
implemented through provider configuration, the Codex SDK, or app-server.

## Provider contract

The initial Codex integration assumes a Responses-compatible endpoint. DS Code
must verify streaming events, tool calls, cancellation, error mapping, usage,
and compaction before a release is marked compatible.

If DouStack exposes only Chat Completions for a model, protocol conversion
belongs in the DouStack gateway or a dedicated adapter, not in the UI.

## Consequences

- Users get Codex agent behavior without official-account authentication.
- API keys remain outside Codex configuration and command arguments.
- Upstream updates are deliberate: pin, test, then release.
- The first deliverable is a CLI and diagnostic path, followed by a desktop UI.
