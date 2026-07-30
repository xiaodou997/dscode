# Runtime compatibility

| DS Code | Codex CLI | CLI provider | app-server initialize | Read-only RPCs | Desktop schema |
| --- | --- | --- | --- | --- | --- |
| 0.1.0 | 0.146.0 | tested | tested on macOS | tested on macOS | stable schema pinned |

The table records verified combinations, not a claim that other versions are
broken. Run these commands to inspect a local installation:

```bash
dscode runtime status
dscode runtime probe
dscode runtime contract
```

An untested version can still be used by the CLI, but the probe exits non-zero
so packaging and desktop startup can enforce the compatibility policy. The
contract command checks `modelProvider/capabilities/read`, `model/list`, and
`thread/list` without starting a model turn.
