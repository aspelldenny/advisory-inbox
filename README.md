# advisory-inbox

Rust CLI + MCP dual-mode binary for advisory inbox state machine. Parses agent scan reports,
deduplicates against a JSON state file, appends new advisories to an inbox markdown, and updates
state — all atomically. Replaces the 142-line Bash heredoc slash command. Runs as a standalone
CLI or as an MCP (Model Context Protocol) server for Claude Code integration.

## Install

```bash
cargo install advisory-inbox
```

Dev build from source:

```bash
cargo build --release
./target/release/advisory-inbox --help
```

## CLI subcommands

Exit codes for all subcommands: `0` success, `1` input error, `2` processing/write error.
See [docs/ARCHITECTURE.md §1](docs/ARCHITECTURE.md) for full exit-code table and flag reference.

All state/inbox writes use temp+fsync+rename per INV-LOCAL-002 (atomic, crash-safe).

### parse-report

Parse the `<!-- INBOX_APPEND_START/END -->` sentinel block from an agent scan report.

```bash
advisory-inbox parse-report < agent-report.md
# → {"advisories_found":2,"rows":[...],"stack_scanned":{}}
advisory-inbox parse-report --input agent-report.md
```

### dedup

Filter parsed rows against `seen_advisories[]` in a state file.

```bash
advisory-inbox dedup --state .advisory-scan-state --rows-json rows.json
# → {"kept":[...],"skipped":[...],"observed_ids":[...]}
```

### append

Insert filtered rows into the inbox markdown after `## Rows` heading.

```bash
advisory-inbox append --inbox advisory-inbox.md --rows-json kept.json
# → {"appended_count":2,"total_open":5}
```

### migrate-state

Convert a legacy single-line ISO-8601 state file to JSON v1 schema. Idempotent.

```bash
advisory-inbox migrate-state --state .advisory-scan-state [--dry-run]
# → {"from":"legacy","to":"json-v1","seen_count":0}
```

### state-backfill

Recovery: extract advisory IDs from `processed`/`dismissed` inbox rows into state.

```bash
advisory-inbox state-backfill --state .advisory-scan-state --inbox advisory-inbox.md [--dry-run]
# → {"backfilled_count":3,"total_seen_after":4}
```

### scan-and-append

Composite: parse report → dedup → append to inbox → update state. One command.

```bash
advisory-inbox scan-and-append \
  --report agent-report.md \
  --inbox advisory-inbox.md \
  --state .advisory-scan-state
# → {"appended":2,"skipped_dedup":1,"total_open":5}
```

Also accepts report from stdin (omit `--report`).

### init

Generate template inbox markdown + empty state file at given paths.

```bash
advisory-inbox init --inbox-path advisory-inbox.md --state-path .advisory-scan-state
# → {"inbox_created":"advisory-inbox.md","state_created":".advisory-scan-state"}
```

### serve

Start MCP server on stdin/stdout (JSON-RPC 2.0). See [MCP server mode](#mcp-server-mode) below.

```bash
advisory-inbox serve
```

## MCP server mode

`advisory-inbox serve` speaks JSON-RPC 2.0 over stdin/stdout, exposing 6 tools to Claude Code
and other MCP-capable clients.

Wire into your project's `.mcp.json` for Claude Code:

```json
{
  "mcpServers": {
    "advisory-inbox": {
      "command": "/path/to/advisory-inbox",
      "args": ["serve"]
    }
  }
}
```

Example `tools/call` request:

```bash
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cli","version":"0.0.0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"parse_report","arguments":{"report_text":"<!-- INBOX_APPEND_START -->\n| 2026-05-28 | CVE-2026-0001 | https://example.com | pkg@1 | f:1 | High | open | - |\n<!-- INBOX_APPEND_END -->"}}}' \
  | advisory-inbox serve
```

| Tool | Description |
|------|-------------|
| `parse_report` | Parse sentinel block from agent report |
| `dedup` | Filter rows against seen_advisories |
| `append` | Insert rows into inbox markdown |
| `migrate_state` | Migrate legacy state to JSON v1 |
| `state_backfill` | Extract IDs from inbox into state |
| `scan_and_append` | Composite: parse → dedup → append → state update |

See [docs/ARCHITECTURE.md §6](docs/ARCHITECTURE.md) for full tool input/output schemas.

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for CLI surface, state schema, inbox format,
sentinel marker spec, module layout, MCP surface, and atomic write pattern.

## Development

```bash
cargo build --release
cargo test --all
cargo clippy --all-targets -- -D warnings
```

## License

MIT — see [LICENSE](LICENSE).
