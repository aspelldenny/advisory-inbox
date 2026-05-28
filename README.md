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
