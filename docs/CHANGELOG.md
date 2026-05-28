# CHANGELOG — advisory-inbox

> Soft cap < 1000 dòng. Rotate batch cũ → `docs/Archive/CHANGELOG_ARCHIVE.md` khi vượt.

---

## P009 — scan-and-append composite subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded

### Added

- `cli/scan_and_append.rs` real impl: composes `sentinel::extract_block` → `row::parse_row` → `state::read` → dedup partition → `inbox::insert_rows` → `inbox::write_atomic` (inbox FIRST) → `state::write_atomic` (state SECOND) in one invocation. Emits `{ "appended": N, "skipped_dedup": M, "total_open": K }` to stdout. Exit 0/1/2 per ARCHITECTURE §1.
- Sub-mech C: `seen_advisories` monotonic non-shrink (BTreeSet union of pre.seen_advisories ∪ observed_ids). `last_scan_at` UPDATED to `Utc::now()` (scan event). `agent_version` + `schema_version` PRESERVED.
- `tests/scan_and_append_cli.rs` — 3 integration tests: happy (1 kept/1 skipped, Sub-mech C), all-skipped (0 appended, last_scan_at still bumped), empty-block (0/0, last_scan_at bumped). Test B uses `parse-report --input` at runtime to extract IDs (robust to fixture changes).

### Changed

- `main.rs` `Commands::ScanAndAppend` dispatch arm: replaced stub passthrough with 5-family error→exit-code map (`SentinelError || StateReadError` → 1; `RowParseError || StateWriteError || InboxError::Io || fallback` → 2; `InboxError::MissingRowsHeading || InboxError::ParseRow` → 1). Tail `Ok(())` per match-arm uniformity (P004 precedent).

### Atomicity caveat (documented — IMPORTANT)

`scan-and-append` is NOT cross-file atomic. It writes TWO files (`inbox` markdown + `state` JSON) via two separate INV-LOCAL-002 atomic writes. Write order is **inbox FIRST, state SECOND** (safer failure mode: worst case = duplicate rows visible, recoverable via `state-backfill`). If state write fails after inbox write succeeded: run `advisory-inbox state-backfill` to reconcile. `state::write_atomic` is now the FOURTH caller of INV-LOCAL-002 (P006 inbox, P007 migrate-state, P008 state-backfill, P009 scan-and-append).

### Test counts

Baseline 59 (post-P008) → post-P009 62 (39 unit + 23 integration).

---

## P008 — state-backfill subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded

### Added

- `inbox::parse_rows(content: &str) -> Result<Vec<AdvisoryRow>, InboxError>` — reads rows under `## Rows` heading, skips HTML comment blocks, header/separator rows, blank lines; stops at next `## ` heading. 6 unit tests (happy path, empty section, HTML comment skip, bad row error, no heading tolerate, stop-at-next-heading).
- `InboxError::ParseRow { path, line_number, source: RowParseError }` — third variant of `InboxError` enum (after `MissingRowsHeading` + `Io`). Placeholder `PathBuf::new()` for `path` in `parse_rows`; caller (`cli/state_backfill`) re-wraps with real path.
- `cli/state_backfill.rs` real impl: reads state + inbox → filters `processed`/`dismissed` rows → BTreeSet union → `state::write_atomic` (third caller of INV-LOCAL-002). Sub-mech C: `seen_advisories` monotonic non-shrink. `last_scan_at` + `agent_version` PRESERVED (backfill is recovery, not scan event). Output: `{ "backfilled_count": N, "total_seen_after": M }`. Exit 0/1/2 per ARCHITECTURE §1.
- `--dry-run` flag: emits same JSON summary without touching state file (byte-identity contract verified by Test C).
- `tests/fixtures/inbox-5rows-3processed.md` — 5-row test fixture (3 processed/dismissed, 2 open).
- `tests/fixtures/state-1id.json` — state fixture with 1 pre-existing seen ID `CVE-2026-7777`.
- `tests/state_backfill_cli.rs` — 4 integration tests: acceptance (5→4 IDs, Sub-mech C), already-backfilled (idempotent), dry-run byte-identity (Sub-mech F), open-rows excluded.

### Changed

- `main.rs` `Commands::Append` dispatch arm: extended exhaustive match on `InboxError` to add `ParseRow { .. } => 1` (compile required — enum gained 3rd variant).
- `main.rs` `Commands::StateBackfill` dispatch arm: replaced stub passthrough with full error→exit-code map (`InboxError` || `StateReadError` → 1, else → 2).

### Test counts

Baseline 49 (post-P007) → post-P008 59 (39 unit + 20 integration).

---

## P007 — migrate-state subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded

### Added

- `state::write_atomic(path, &StateFile)` — second concrete user of INV-LOCAL-002 atomic-write protocol (after P006 inbox). 3 unit tests (round-trip, trailing newline, no-parent edge case).
- `state::StateWriteError` enum (Io variant) — exit code 2 contract per ARCHITECTURE §1.
- `cli/migrate_state.rs` real impl: detects missing / JSON v1 / legacy single-line ISO / garbage; preserves `last_scan_at` across legacy → JSON v1 conversion; `--dry-run` flag NEVER touches file.
- `cli/migrate_state::MigrateError` enum: `FormatUnknown` + `UnsupportedSchema` (both → exit 1).
- `Commands::MigrateState` main.rs dispatch arm: `MigrateError` → exit 1, `StateWriteError::Io` → exit 2.
- 5 new integration tests (`tests/migrate_state_cli.rs`): missing / legacy / json-v1 / garbage / dry-run.
- 3 new fixtures: `tests/fixtures/state-legacy.txt`, `state-json-v1.json`, `state-garbage.txt`.

### Migration note

Existing tarot users with legacy single-line ISO-8601 state files can now run:

```
advisory-inbox migrate-state --state ~/.advisory-scan-state
```

to convert in-place to JSON v1 schema. `last_scan_at` value preserved. `seen_advisories` initialized empty (legacy format had no IDs — this is expected, not data loss).

Run with `--dry-run` first to preview:

```
advisory-inbox migrate-state --state ~/.advisory-scan-state --dry-run
```

### Changed

- `Commands::MigrateState` main.rs dispatch arm now error-maps `MigrateError` → exit 1, `StateWriteError::Io` → exit 2 (previously flat stub passthrough from P001).
- `docs/ARCHITECTURE.md` §2 — added "State write path (post-P007)" subsection.
- `docs/ARCHITECTURE.md` §5 — P007 scaffold-status entry added.
- `docs/security/INVARIANTS.md` — INV-LOCAL-002 P007 second concrete user note appended.
- `README.md` — `migrate-state` quick-start section added.

### Sub-mech checks

- B (cargo check + cargo test state + cargo test --test migrate_state_cli): ✅
- C (migration completeness — last_scan_at preserved in Test B): ✅
- D (persistence — ARCHITECTURE §2/§5/§7 + INVARIANTS + CHANGELOG + README updated): ✅
- E (env drift — cargo update --dry-run clean, release build clean): ✅
- F (no token leak — grep clean across new code): ✅

Test count: baseline 41 → post-P007 49 (33 unit + 16 integration).

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## P006 — append subcmd atomic write (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded (filesystem write)

- New module `src/inbox.rs` — `read_inbox`, `insert_rows`, `write_atomic` + `InboxError` enum (2 variants: `MissingRowsHeading` → exit 1, `Io` → exit 2). First concrete user of INV-LOCAL-002 atomic-write protocol; establishes reference shape for P007/P008/P009/P011.
- `cli/append.rs` wired (stub → real impl): reads `{ "rows": [...] }` JSON envelope, reads inbox markdown, inserts rows after `## Rows` heading (rows[0] topmost), atomic-writes result, emits `{ "appended_count": N, "total_open": M }` to stdout.
- `impl Display for AdvisoryRow / Status / Severity` added to `src/row.rs` — pipe-delim 8-col render per ARCHITECTURE §3. Status = lowercase, Severity = PascalCase (matches serde rename_all convention).
- `src/main.rs`: added `mod inbox;`, updated `Commands::Append` dispatch arm with `InboxError` downcast + variant-aware exit code map (MissingRowsHeading → 1, Io → 2, other → 2).
- New fixtures: `tests/fixtures/inbox-baseline.md` (2 rows: 1 open + 1 processed + HTML comment placeholder), `tests/fixtures/rows-2.json` (2 open rows).
- New integration test `tests/append_cli.rs` (4 cases: happy path 2-new-rows / missing heading exit 1 / rows malformed exit 2 / atomic write no-leftover-tmp).
- `docs/ARCHITECTURE.md` §5: mark P006 shipped, remove `inbox.rs` from Pending list.
- `docs/security/INVARIANTS.md` §3 INV-LOCAL-002: added "First concrete user" + user-supplied path note.
- `README.md`: added `append` quick-start section.
- Test count: baseline 31 → post-P006 41 (30 unit + 11 integration).
- Sub-mech B ✅, D ✅, E ✅, F ✅.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## P005 — dedup subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Normal

- Wire `src/cli/dedup.rs` — `run(state: PathBuf, rows_json: PathBuf) -> anyhow::Result<()>` reads state via `state::read`, deserializes rows envelope `{ "rows": [...] }`, partitions rows into `kept`/`skipped` against `state.seen_advisories`, emits JSON `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` to stdout. `observed_ids` carries every input row's `advisory_id` regardless of kept/skipped.
- Extend `src/state.rs`: add `pub fn read(&Path) -> Result<StateFile, StateReadError>` (read + parse + schema_version validate). Add `pub enum StateReadError` (Io/Json/SchemaMismatch, all via `thiserror`). SchemaMismatch Display hints `advisory-inbox migrate-state` (P007 wire-up).
- Remove `#![allow(dead_code)]` from `src/state.rs` (consumer wire-in complete: `read()` + `cli::dedup`).
- `src/main.rs`: dispatch `Commands::Dedup { state, rows_json }` maps `StateReadError` → exit 1, other → exit 2; anyhow cause chain printed to stderr.
- New fixtures `tests/fixtures/state-3ids.json` + `tests/fixtures/rows-5.json` (5 rows, 2 match state IDs).
- New integration test `tests/dedup_cli.rs` (4 cases: happy 3-kept/2-skipped, state missing → 1, schema mismatch → 1, rows malformed → 2).
- New unit tests in `src/state.rs`: `read_happy_path`, `read_missing_file_errors`, `read_schema_mismatch_errors`, `read_malformed_json_errors`.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## P004 — parse-report subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Normal

- Wire `src/cli/parse_report.rs` — `run(Option<PathBuf>) -> anyhow::Result<()>` reads stdin or `--input <FILE>`, calls `sentinel::extract_block` then `row::parse_row` per line, emits JSON `{ "rows": [...], "stack_scanned": {}, "advisories_found": N }` to stdout.
- Extend `src/row.rs`: add `pub fn parse_row(&str) -> Result<AdvisoryRow, RowParseError>` (pipe-split + per-cell decode), `pub enum RowParseError` (5 variants via `thiserror`), `impl FromStr for Status` + `impl FromStr for Severity`.
- Remove `#![allow(dead_code)]` from `src/row.rs` (consumer wire-in complete per P002 Discovery follow-up). `src/state.rs` keeps the attribute until P005 dedup wire-in.
- `src/main.rs`: dispatch `Commands::ParseReport { input }` maps `SentinelError` → exit 1, all other errors → exit 2; `anyhow` cause chain printed to stderr.
- New fixture `tests/fixtures/agent-report-1.md` (2 rows).
- New integration test `tests/parse_report_cli.rs` (3 cases: happy path, missing sentinel → exit 1, bad severity → exit 2).
- `stack_scanned` placeholder `{}` per BACKLOG MVP scope — future phiếu may parse `**Stack scanned:**` section.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## P003 — Sentinel parser (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Normal

- Add `src/sentinel.rs` module: `extract_block(&str) -> Result<Vec<String>, SentinelError>` extracts raw row lines between first `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` pair.
- `SentinelError` enum (`MissingStartMarker` / `MissingEndMarker`) via `thiserror` 2.x derive.
- Multiple START markers: use first pair, emit `eprintln!` warn (no fail) — intentional operational stderr (per Turn 1 O1.2).
- Skip blank lines + HTML-comment lines (`<!-- ... -->`) inside block.
- 6 inline unit tests (happy path / empty block / missing start / missing end / multiple pairs / comment-skip).
- Register `mod sentinel;` in `src/main.rs`.
- Update `docs/ARCHITECTURE.md` §5 scaffold status.
- Doctrine sync (Turn 1 O1.1 ACCEPT): amend `CLAUDE.md` Tech Stack `Regex` entry + File Structure comment for `sentinel.rs` to reflect `str::find` implementation choice; `regex` crate remains declared for `inbox.rs`/pattern matching.

Not yet wired into `cli/parse_report.rs` — that's P004.

---

## 2026-05-28 — P002: row/state types (serde)

### Added
- `src/row.rs`: `AdvisoryRow` struct (8 fields per ARCHITECTURE §3) + `Status` enum (`open`/`processed`/`dismissed`) + `Severity` enum (`Critical`/`High`/`Medium`/`Low`).
- `src/state.rs`: `StateFile` struct (4 fields per ARCHITECTURE §2) + `pub const SCHEMA_VERSION: u32 = 1`.
- Unit tests inline: 4 in `row.rs` (roundtrip, status lowercase, severity PascalCase, known-JSON parse), 4 in `state.rs` (roundtrip, schema_version const, known-JSON parse, seen_advisories order preserved).

### Changed
- `src/main.rs`: added `mod row;` + `mod state;` declarations (alphabetical after `mod cli;`).
- `docs/ARCHITECTURE.md` §5: mark `row.rs` + `state.rs` shipped.

### Notes
- Types declared but not yet consumed — `src/cli/*.rs` stubs unchanged (still printf TODO). P003 sentinel parser is next consumer.
- Schema lock: `SCHEMA_VERSION = 1`. Sub-mech C bump rule armed for P007 migrate-state.
- No new dependencies. `chrono` already carries `features = ["serde"]` (Cargo.toml line 17).
- `#![allow(dead_code)]` added to both modules (scaffold types, no consumer yet in binary code path). Will be removed when P004+ wire-in imports them.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## 2026-05-28 — P001: Scaffold CLI surface

### P001 — Scaffold CLI surface (clap derive, 8 subcommand stubs)

- Added `src/main.rs` with clap 4 derive `Cli` + `Commands` enum (8 variants).
- Added `src/cli/` module skeleton with 8 stub handlers (`parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`, `serve`, `init`).
- Each stub prints `TODO: <subcmd> — wired in P<NNN>` and exits 0 per BACKLOG acceptance.
- No new dependency added. `Cargo.toml` unchanged.
- Lane: Normal. Sub-mech checks: B (cargo check + cargo build), D (ARCHITECTURE §1 + §5 grep preserved).

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)

---

## 2026-05-28 — Bootstrap (P000)

- Initial repo seed via Workflow v2.1 pilot setup
- Cargo crate scaffolded (edition 2024, deps: clap/serde/tokio/chrono/rmcp/tempfile/regex/anyhow/thiserror)
- Workflow v2.1 doctrine ported from `~/sos-kit/docs/WORKFLOW_V2.1.md`
- Skeleton copied from `~/advisory-cron` (agents, scripts, ticket template, INVARIANTS)
- `docs/RULES.md` written với 17 sections covering all 13 v2.1 items
- `docs/PROJECT.md` vision + scope cứng
- `docs/ARCHITECTURE.md` 6 subcmd + state schema + inbox format + MCP surface
- `docs/BACKLOG.md` P001..P013 phiếu queued across 4 phase
- `.tools/runtime-env.allowlist` 3-group schema (required/optional/forbidden)
- `.github/pull_request_template.md` với Lane override section (v2.1 §13)

home: docs/RULES.md (durable doctrine port)
