# advisory-inbox

Rust binary for advisory inbox state machine — parse agent report, dedup, append, migrate state. Replaces 142-line Bash heredoc slash command. CLI + MCP dual mode.

## Quick Start (P001 scaffold)

```bash
# Build
cargo build --release

# Show all 8 subcommands
./target/release/advisory-inbox --help

# Each subcommand currently prints a TODO message (logic wired in P004-P011)
./target/release/advisory-inbox parse-report
./target/release/advisory-inbox dedup --state /tmp/s.json --rows-json /tmp/r.json
```

See `docs/ARCHITECTURE.md` §1 for full CLI surface and `docs/BACKLOG.md` for phiếu pipeline.
