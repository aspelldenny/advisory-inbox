# ARCHITECTURE — advisory-inbox

> Single source of truth cho code structure, state schema, CLI surface, MCP surface.
> Update mandatory khi phiếu Tầng 1 ship (RULES.md §11).

---

## §1. CLI Surface

```
advisory-inbox <SUBCOMMAND> [OPTIONS]

Subcommands:
  parse-report       Parse sentinel block from agent report (stdin or --input <path>)
  dedup              Filter rows against state seen_advisories[]
  append             Insert rows after `## Rows` heading in inbox markdown
  migrate-state      Convert legacy single-line ISO state file to JSON schema
  state-backfill     Extract advisory IDs from inbox rows into state seen_advisories[]
  scan-and-append    Composite: parse → dedup → append + state update
  serve              Start MCP server on stdin/stdout (JSON-RPC 2.0)
  init               Generate default config templates
```

### Subcmd: parse-report

```
advisory-inbox parse-report [--input <FILE>]
```

- **Input:** stdin (default) or `--input <FILE>` agent report markdown
- **Output:** stdout JSON `{ "rows": [ ... ], "stack_scanned": {...}, "advisories_found": N }`
- **Exit:** 0 success, 1 sentinel block missing, 2 parse error

### Subcmd: dedup

```
advisory-inbox dedup --state <FILE> --rows-json <FILE>
```

- **Input:** `--state` JSON state file path, `--rows-json` rows JSON from parse-report
- **Output:** stdout JSON `{ "kept": [...], "skipped": [...], "observed_ids": [...] }`
- **Exit:** 0 success, 1 state file unreadable, 2 rows malformed

### Subcmd: append

```
advisory-inbox append --inbox <FILE> --rows-json <FILE>
```

- **Input:** `--inbox` markdown path, `--rows-json` filtered rows
- **Behavior:** Insert rows after line matching `^## Rows$`, newest at top, atomic write temp+rename
- **Output:** stdout JSON `{ "appended_count": N, "total_open": M }`
- **Exit:** 0 success, 1 inbox missing `## Rows` heading, 2 write error

### Subcmd: migrate-state

```
advisory-inbox migrate-state --state <FILE> [--dry-run]
```

- **Input:** `--state` existing state file (any format)
- **Behavior:** Detect format (JSON / single-line ISO / missing) → write JSON schema
- **Output:** stdout JSON `{ "from": "legacy", "to": "json-v1", "seen_count": N }`
- **Exit:** 0 success, 1 format unknown, 2 write error

### Subcmd: state-backfill

```
advisory-inbox state-backfill --state <FILE> --inbox <FILE> [--dry-run]
```

- **Input:** `--state` JSON path, `--inbox` markdown path
- **Behavior:** Extract advisory IDs from inbox rows status `processed`/`dismissed` → union with existing `seen_advisories[]`
- **Output:** stdout JSON `{ "backfilled_count": N, "total_seen_after": M }`
- **Exit:** 0 success, 1 inbox unparseable, 2 write error

### Subcmd: scan-and-append (composite)

```
advisory-inbox scan-and-append \
  --report <STDIN_OR_FILE> \
  --inbox <FILE> \
  --state <FILE>
```

- **Behavior:** parse-report → dedup → append → state update (atomic), 1 lệnh
- **Output:** stdout JSON `{ "appended": N, "skipped_dedup": M, "total_open": K }`
- **Exit:** 0 success, 1..3 per subcmd error mapping

### Subcmd: serve (MCP)

```
advisory-inbox serve
```

- **Transport:** stdin/stdout JSON-RPC 2.0 (rmcp stdio)
- **Tools exposed:** 6 (parse_report, dedup, append, migrate_state, state_backfill, scan_and_append)
- **Behavior:** Long-running, no exit until stdin closed

### Subcmd: init

```
advisory-inbox init [--inbox-path <PATH>] [--state-path <PATH>]
```

- **Behavior:** Generate template `<inbox-path>` with `## Rows` heading + `.advisory-scan-state` empty JSON schema
- **Output:** stdout JSON `{ "inbox_created": <path>, "state_created": <path> }`
- **Exit:** 0 success, 1 file exists (no overwrite)

### Exit code conventions

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Input error (file missing, format invalid) |
| 2    | Processing error (parse fail, write fail) |
| 3    | Concurrency/lock error (state file held by another process — future) |
| 5    | MCP transport error (rmcp serve mode only) |
| 64+  | Reserved for future |

---

## §2. State Schema

### File: `.advisory-scan-state`

**Format:** JSON, atomic write via temp+rename.

```json
{
  "schema_version": 1,
  "last_scan_at": "2026-05-28T09:51:35Z",
  "seen_advisories": [
    "CVE-2026-9256",
    "GHSA-xxxx-yyyy",
    "CVE-2026-27205"
  ],
  "agent_version": "advisory-watch@0.1.0"
}
```

**Field constraints:**
- `schema_version`: u32, current = 1. Bump khi breaking change.
- `last_scan_at`: ISO-8601 UTC string. `chrono::DateTime<chrono::Utc>` parse.
- `seen_advisories`: array of advisory ID strings. Dedup via `BTreeSet` internal.
- `agent_version`: string, free-form (e.g., `"advisory-watch@0.1.0"`).

### Legacy format (pre-migration)

Single-line ISO-8601 (no JSON): `2026-05-23T12:00:00Z\n`. Migrate-state subcmd detects + converts.

---

## §3. Inbox Markdown Format

### File: `docs/security/advisory-inbox.md` (target project)

```markdown
# Advisory Inbox

> Sếp gạt row "open" → "processed" hoặc "dismissed" + ghi note.

## Rows

| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-9999 | https://... | next@<15.5.17 | src/middleware.ts:42 | High | open | - |

<!-- Placeholder example (in HTML comment — append skips this) -->
<!--
| 2026-05-23 | GHSA-xxxx-yyyy | ... | example@<1.0 | indirect | Medium | open | - |
-->
```

**Rules:**
- Heading `## Rows` mandatory. Append inserts AFTER this line, newest at top.
- HTML comment block `<!-- ... -->` skipped by parser (placeholder examples).
- Pipe-delimited row, 8 columns: Date / Advisory ID / Source / Package / File:Line / Severity / Status / Note.
- Status enum: `open` / `processed` / `dismissed`.
- Severity enum: `Critical` / `High` / `Medium` / `Low` (upstream official only per RULES.md §X).

---

## §4. Sentinel Marker Format (agent report parsing)

Agent emits report with structured block:

```markdown
## Advisory Scan Report — 2026-05-28

**Stack scanned:**
- pnpm-lock.yaml resolved: 42 packages
- requirements.txt exact pin: 8 deps

**Advisories found:** 3

<!-- INBOX_APPEND_START -->
| 2026-05-28 | CVE-2026-9999 | https://... | next@<15.5.17 | src/middleware.ts:42 | High | open | - |
| 2026-05-28 | GHSA-aaaa-bbbb | https://... | flask@<2.3.5 | astro-service/app.py:8 | Medium | open | - |
<!-- INBOX_APPEND_END -->
```

**Parser rules:**
- Match first occurrence of `<!-- INBOX_APPEND_START -->` and `<!-- INBOX_APPEND_END -->`.
- Extract pipe-delimited rows between markers.
- Skip blank lines + comment lines inside block.
- Empty block (0 rows) = valid, return empty rows array.
- Missing markers = error exit code 1.

---

## §5. Module Layout

```
src/
├── main.rs              # CLI entry — clap parse, dispatch to subcmd
├── cli/
│   ├── mod.rs           # Subcmd registry
│   ├── parse_report.rs
│   ├── dedup.rs
│   ├── append.rs
│   ├── migrate_state.rs
│   ├── state_backfill.rs
│   ├── scan_and_append.rs
│   ├── init.rs
│   └── serve.rs         # MCP server entry
├── state.rs             # State file JSON read/write atomic
├── inbox.rs             # Inbox markdown parser + writer atomic
├── row.rs               # AdvisoryRow struct + (de)serialize
├── sentinel.rs          # Sentinel marker regex + block extract
├── mcp/
│   ├── mod.rs           # MCP service definition
│   ├── tools.rs         # 6 tools schema + handler
│   └── transport.rs     # rmcp stdio wiring
└── error.rs             # thiserror Error enum
```

**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `sentinel.rs`, `mcp/`, `error.rs`.

---

## §6. MCP Surface

### Server info

- **Name:** `advisory-inbox`
- **Version:** Cargo.toml version
- **Transport:** stdio JSON-RPC 2.0 (rmcp)

### Tools exposed (6)

| Tool name | Description | Input schema | Output schema |
|-----------|-------------|--------------|---------------|
| `parse_report` | Parse sentinel block | `{ "report_text": "string" }` | `{ "rows": [...], "stack_scanned": {...}, "advisories_found": N }` |
| `dedup` | Filter against seen IDs | `{ "state_path": "string", "rows": [...] }` | `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` |
| `append` | Insert into inbox | `{ "inbox_path": "string", "rows": [...] }` | `{ "appended_count": N, "total_open": M }` |
| `migrate_state` | Legacy → JSON | `{ "state_path": "string", "dry_run": bool }` | `{ "from": "string", "to": "string", "seen_count": N }` |
| `state_backfill` | Extract IDs from inbox | `{ "state_path": "string", "inbox_path": "string" }` | `{ "backfilled_count": N }` |
| `scan_and_append` | Composite | `{ "report_text": "string", "inbox_path": "string", "state_path": "string" }` | `{ "appended": N, "skipped_dedup": M, "total_open": K }` |

### Error format

All tools return MCP error on failure:
```json
{
  "code": -32000,
  "message": "Inbox missing `## Rows` heading",
  "data": { "subcmd": "append", "exit_code": 1 }
}
```

---

## §7. Atomic Write Pattern

Every write to `inbox.md` or `.advisory-scan-state`:

```rust
use std::fs;
use tempfile::NamedTempFile;

fn atomic_write(target: &Path, content: &[u8]) -> Result<()> {
    let parent = target.parent().context("no parent dir")?;
    let mut temp = NamedTempFile::new_in(parent)?;  // same filesystem → rename atomic
    temp.write_all(content)?;
    temp.flush()?;
    temp.persist(target)?;  // atomic rename
    Ok(())
}
```

**Rule:** temp file MUST be in same parent dir as target (same filesystem for atomic rename).

---

## §8. Test Strategy

- **Unit tests** (per module): `#[cfg(test)]` in same file. Pure logic (sentinel parse, dedup filter, row serialize).
- **Integration tests** (`tests/`): `assert_cmd` invoke binary, `predicates` assert stdout/stderr.
- **Fixtures** (`tests/fixtures/`): real agent report samples (export từ tarot history), state JSON exemplars, inbox markdown samples.
- **MCP tests:** mock stdio transport, send JSON-RPC, assert response.

Target: `cargo test --all` ≥ 30 tests after Phase 3.

---

## §9. Performance Budget

- **Cold start:** < 50ms (CLI parse → first action)
- **parse-report:** < 100ms for typical 10-row report
- **append (atomic write):** < 50ms for 100-row inbox
- **MCP serve:** < 10ms per tool dispatch (excluding actual work)

Profile via `cargo flamegraph` if regression.

---

## §10. Future Surface (NOT in MVP)

- Concurrency lock (multi-process state write — Sub-mech C related)
- Inbox archive (rows > 90 days status=dismissed → archive file)
- Watch mode (`advisory-inbox watch --state <path>` tail-follow)
- Severity threshold filter (`--min-severity High`)

Track in `docs/BACKLOG.md` Defer section.
