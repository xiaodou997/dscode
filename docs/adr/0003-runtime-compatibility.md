# ADR 0003: Gate desktop use on a tested Codex runtime

Status: accepted for phase two

## Context

DS Code delegates agent behavior to the stock Codex binary. The CLI provider
configuration is small, but the desktop application will depend on the much
larger app-server protocol. Codex generates app-server schemas per binary
version, so assuming compatibility across untested releases would make session,
approval, and file-operation behavior unreliable.

## Decision

DS Code records one exact Codex version as its tested runtime baseline. The
initial baseline is `0.146.0`.

`dscode runtime status` parses and classifies the installed version.
`dscode runtime probe` starts `codex app-server` with the stable stdio
transport, sends `initialize`, validates its response and isolated `codexHome`,
sends `initialized`, and shuts the child process down. It does not opt into the
experimental API.

`dscode runtime contract` uses the same initialization path but keeps the
app-server process alive while it checks stable read-only RPCs. Live model-turn
behavior remains a separate release check because it can contact the configured
provider and consume quota.

The ordinary CLI launcher may continue with an untested Codex version because
its integration uses the smaller provider configuration surface. Runtime probes
return a non-zero status for an untested version even when initialization works.
The desktop client will require both the tested version and a successful probe.

WebSocket transport will not be used for production. The combined stable schema
is stored under `schemas/codex/<version>/` with the exact Codex version and
checksum that passed the runtime probe. Experimental schema fields are excluded.

## Upgrade procedure

1. Select a candidate stock Codex release.
2. Generate its stable app-server schema without experimental fields.
3. Run initialization, streaming, tools, approvals, cancellation, compaction,
   and session-resume tests against DouStack.
4. Update the tested version and bundled schema in the same change.
5. Publish the DS Code update before offering that Codex runtime to users.

## Consequences

- Upstream Codex updates remain replaceable without maintaining a source fork.
- Desktop protocol breakage is caught before release instead of at user startup.
- Newer Codex versions are visible immediately but are not silently treated as
  desktop-compatible.
- Supporting multiple runtime versions later requires a schema and test result
  for every supported version.
