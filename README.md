# advisory-inbox

Rust binary for advisory inbox state machine — parse agent report, dedup, append, migrate state. Replaces 142-line Bash heredoc slash command. CLI + MCP dual mode.

## Quick Start

```bash
# Build
cargo build --release

# Show all 8 subcommands
./target/release/advisory-inbox --help
```

### Parse an agent report

Pipe an advisory-watch agent report to the binary; get JSON on stdout:

```bash
advisory-inbox parse-report < path/to/agent-report.md
# → { "advisories_found": N, "rows": [...], "stack_scanned": {} }

# Or read from a file:
advisory-inbox parse-report --input path/to/agent-report.md
```

Exit codes:

| Code | Meaning |
|------|---------|
| `0`  | Parsed N rows successfully (N may be 0 if block is empty) |
| `1`  | Sentinel markers `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` missing |
| `2`  | Row format invalid (bad date, unknown severity, wrong cell count) or I/O error |

```bash
# Other subcommands (logic wired in P005-P011)
./target/release/advisory-inbox dedup --state /tmp/s.json --rows-json /tmp/r.json
```

See `docs/ARCHITECTURE.md` §1 for full CLI surface and `docs/BACKLOG.md` for phiếu pipeline.
