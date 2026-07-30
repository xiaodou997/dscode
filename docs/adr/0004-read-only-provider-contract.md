# ADR 0004: Separate read-only and live provider checks

Status: accepted for phase two

## Context

DS Code needs early warning when an installed Codex app-server no longer
matches the protocol expected by the product. Some useful checks only inspect
local metadata, while starting a model turn can contact DouStack, consume
quota, request approval, or modify session state.

## Decision

`dscode runtime contract` launches the stock Codex app-server with the same
DouStack provider configuration and isolated `CODEX_HOME` used by normal DS
Code sessions. It performs only these stable RPCs:

- `modelProvider/capabilities/read`
- `model/list`
- `thread/list`, filtered to the `doustack` provider

The runtime crate owns the long-lived stdio JSONL client, including initialize,
request matching, notification buffering, timeouts, stderr capture, and child
shutdown. The command never calls `thread/start` or `turn/start`.

Streaming, tool calls, approvals, cancellation, compaction, and resume belong
to an explicit live compatibility suite. That suite must disclose its network
and quota effects before it runs.

## Consequences

- Developers and packaging jobs can verify the desktop protocol foundation
  without making a model request.
- The check still uses the real DS Code provider configuration and credential
  path, so configuration regressions are visible.
- A passing read-only contract is necessary but not sufficient for a release;
  live agent workflows still need acceptance testing.
