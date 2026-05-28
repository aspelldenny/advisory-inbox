# CHANGELOG — advisory-inbox

> Soft cap < 1000 dòng. Rotate batch cũ → `docs/Archive/CHANGELOG_ARCHIVE.md` khi vượt.

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
