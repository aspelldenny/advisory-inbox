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

### Dedup against state

Filter parsed rows against `seen_advisories[]` in a state file:

```bash
advisory-inbox dedup --state .advisory-scan-state --rows-json rows.json
# → { "kept": [...], "observed_ids": [...], "skipped": [...] }
```

- `kept` — rows whose `advisory_id` is NOT yet in state (new advisories).
- `skipped` — rows whose `advisory_id` is already in state (re-observed).
- `observed_ids` — every input row's `advisory_id` (downstream uses this to extend state).

Exit codes:

| Code | Meaning |
|------|---------|
| `0`  | Partition succeeded (any number of kept/skipped, including zero) |
| `1`  | State file missing, malformed JSON, or `schema_version != 1` — run `advisory-inbox migrate-state` to upgrade |
| `2`  | Rows JSON missing or malformed (expected envelope `{ "rows": [...] }`) |

See `docs/ARCHITECTURE.md` §1 for full CLI surface and `docs/BACKLOG.md` for phiếu pipeline.

### `advisory-inbox append`

Insert filtered rows into the inbox markdown at the top of `## Rows`, atomic-write.

```bash
advisory-inbox append --inbox <FILE> --rows-json <FILE>
```

**Input:** `--inbox` markdown path, `--rows-json` JSON file with `{ "rows": [...] }` shape (e.g., output of `dedup`'s `kept` array re-wrapped).

**Output (stdout):** `{ "appended_count": N, "total_open": M }`.

**Exit codes:**

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Inbox missing `## Rows` heading |
| 2    | Write error (rows JSON malformed, file unreadable, disk full, etc.) |

**Atomic write:** uses temp+fsync+rename protocol per INV-LOCAL-002 — partial-write safe across crash/power-loss.

### `migrate-state`

Convert legacy single-line ISO-8601 state file to JSON v1 schema. Idempotent for files already in JSON v1.

```bash
advisory-inbox migrate-state --state <FILE> [--dry-run]
```

**Behaviors:**

- File missing — creates fresh JSON v1 (`last_scan_at = now`, empty `seen_advisories`).
- File is JSON v1 already — idempotent re-write (normalises pretty-print format).
- File is single-line ISO-8601 timestamp (legacy tarot format) — converts to JSON v1, preserves timestamp in `last_scan_at`.
- File is anything else — exit 1 (format unknown).

**Flags:**

- `--dry-run` — print intended `{from, to, seen_count}` summary, but do NOT modify file on disk.

**Output (stdout JSON):**

```json
{"from": "legacy", "to": "json-v1", "seen_count": 0}
```

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0`  | Success |
| `1`  | Format unknown (file content not parseable as JSON v1 or single-line ISO-8601) |
| `2`  | Write error (permission denied, disk full, etc.) |

**Atomic write:** state file write uses temp+fsync+rename per INV-LOCAL-002 — crash-safe.

### `state-backfill`

Recovery path: extracts advisory IDs from `processed`/`dismissed` rows in the inbox markdown and unions them into `state.seen_advisories[]`. Use this when the state file was lost or corrupted but the inbox retains Sếp's review decisions.

```bash
advisory-inbox state-backfill --state <FILE> --inbox <FILE> [--dry-run]
```

**Behavior:**

- Reads `processed` and `dismissed` rows from inbox markdown under the `## Rows` heading.
- Unions extracted IDs with pre-existing `seen_advisories[]` (monotonic — never shrinks).
- `open` rows are NOT backfilled (still pending review).
- Preserves `last_scan_at` and `agent_version` — backfill is recovery, not a scan event.
- Idempotent: re-running produces the same result (BTreeSet union, always re-writes for canonical sort).

**Flags:**

- `--dry-run` — print intended summary JSON, but do NOT modify state file on disk.

**Output (stdout JSON):**

```json
{"backfilled_count": 3, "total_seen_after": 4}
```

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0`  | Success |
| `1`  | Input file invalid (inbox unparseable or state unreadable/wrong schema) |
| `2`  | Write error (permission denied, disk full, etc.) |

**Atomic write:** state file write uses temp+fsync+rename per INV-LOCAL-002 — crash-safe.

### `scan-and-append`

Composite: parse agent report → dedup → append to inbox → update state — all in one command. Replaces the 142-line Bash heredoc pipeline.

```bash
advisory-inbox scan-and-append \
  --report path/to/agent-report.md \
  --inbox path/to/advisory-inbox.md \
  --state path/to/.advisory-scan-state

# Or read report from stdin (omit --report):
cat path/to/agent-report.md | advisory-inbox scan-and-append \
  --inbox path/to/advisory-inbox.md \
  --state path/to/.advisory-scan-state
```

**Output (stdout JSON):**

```json
{"appended": 2, "skipped_dedup": 1, "total_open": 5}
```

- `appended` — new rows inserted into inbox (advisory IDs not yet in state).
- `skipped_dedup` — rows whose `advisory_id` was already in `seen_advisories[]` (skipped).
- `total_open` — count of `open` rows in the inbox after insertion.

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0`  | Success (including empty sentinel block — `appended: 0` is valid) |
| `1`  | Input error: sentinel markers missing, state file unreadable/wrong schema, or inbox missing `## Rows` heading |
| `2`  | Write error: row parse failure, I/O error on inbox or state write, disk full, etc. |

**Atomicity caveat:** This composite writes TWO files (inbox markdown + state JSON) via separate atomic writes (INV-LOCAL-002 per file). The PAIR is NOT cross-file transactional. Write order is **inbox first, state second** — if state write fails after inbox write succeeded, run `advisory-inbox state-backfill` to reconcile.

**State updates:**
- `seen_advisories[]` — extended with all observed IDs (kept ∪ skipped). Monotonic, never shrinks.
- `last_scan_at` — updated to current UTC time (scan event).
- `agent_version` + `schema_version` — preserved unchanged.

## MCP server mode

`advisory-inbox` can also run as an MCP (Model Context Protocol) server, exposing its
functionality to Claude Code and other MCP-capable AI assistants via JSON-RPC 2.0 over
stdin/stdout.

```sh
# Direct invocation (handshake-only as of P010; tools come in P011):
advisory-inbox serve
```

Or wire into your project's `.mcp.json`:

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

The server responds to MCP `initialize` requests with server info (`name: "advisory-inbox"`,
`version: <Cargo.toml>`). As of P010, no tools are registered — P011 will expose 6 tools
(`parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`).

Exit code 5 indicates MCP transport / runtime error. All other exit codes apply only to direct
CLI subcommands (see Exit codes table in each subcmd section above).
