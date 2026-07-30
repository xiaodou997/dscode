# ADR 0002: Store credentials in the operating system

Status: accepted for phase two

## Context

Requiring every user to export `DOUSTACK_API_KEY` makes first-run setup fragile
and does not give the future desktop application a suitable login experience.
Writing the API key to `~/.dscode/config.toml` would expose it to backups,
support bundles, and accidental commits.

## Decision

DS Code stores the DouStack API key in the operating system credential manager
through the Rust `keyring` interface. The stable keyring identity is:

- service: `com.doustack.dscode`
- account: `doustack-api-key`

The non-sensitive endpoint and model remain in `~/.dscode/config.toml`.
`DOUSTACK_API_KEY` takes precedence over the stored credential for development
and CI. At launch, DS Code injects the resolved key only into the Codex child
process environment required by the provider configuration.

The precedence order is:

1. DS Code command-line endpoint and model overrides
2. DouStack environment overrides
3. settings saved in `~/.dscode/config.toml`
4. built-in defaults

For credentials, the order is `DOUSTACK_API_KEY`, then the operating system
credential manager.

## Consequences

- Normal users can log in once without editing shell startup files.
- The CLI and desktop application can share the same credential identity.
- Headless environments can continue to use an explicit environment variable.
- The operating system may ask the user to unlock or authorize its credential
  manager.
- DS Code diagnostics report only the credential source, never its value.
