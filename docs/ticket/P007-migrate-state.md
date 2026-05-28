# PHIẾU P007: migrate-state subcmd

> **ID format:** `P007` — counter `.phieu-counter` = 7 sau P006 ship.
> **Filename:** `docs/ticket/P007-migrate-state.md`
> **Branch:** `feat/P007-migrate-state`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P008 state-backfill — backfill assumes state file ALREADY in JSON v1 shape, so migrate-state must ship first to bridge legacy users; cũng cần cho P013 install in tarot where existing state file là legacy single-line ISO format)
> **Ảnh hưởng:** `src/state.rs` (ADD `write_atomic` + `StateWriteError` + tests), `src/cli/migrate_state.rs` (stub → real impl), `src/main.rs` (update `Commands::MigrateState { state, dry_run }` dispatch arm — error→exit code map), `tests/fixtures/state-legacy.txt` (NEW — single-line ISO), `tests/fixtures/state-json-v1.json` (NEW — already migrated), `tests/fixtures/state-garbage.txt` (NEW — unparseable), `tests/migrate_state_cli.rs` (NEW integration test), `docs/ARCHITECTURE.md` §2 + §5 (mark state.rs gains `write_atomic`; scaffold-status P007 entry), `docs/ARCHITECTURE.md` §7 (note state-write path now uses INV-LOCAL-002 — second concrete user after P006 inbox), `docs/CHANGELOG.md` (entry P007 + migration note), `docs/security/INVARIANTS.md` (INV-LOCAL-002 — append "P007 second concrete user — state file"), `README.md` (`migrate-state` quick-start nếu chưa cover — Worker check Anchor #14)
> **Dependency:** P001 (CLI scaffold + `Commands::MigrateState` variant + `cli/migrate_state.rs` stub), P002 (`StateFile` 4 fields + `SCHEMA_VERSION = 1`), P005 (`state::read` + `StateReadError` Io/Json/SchemaMismatch — reference shape for new `StateWriteError`), P006 (`inbox::write_atomic` — INV-LOCAL-002 reference shape; P007 mirrors exactly) — all shipped 2026-05-28
> **Lane:** **Guarded** (legacy data migration + filesystem persistence — write to state file using INV-LOCAL-002 atomic-write; dry_run flag MUST NOT touch file; data-preservation contract per Sub-mech C — `last_scan_at` MUST survive legacy→JSON v1 conversion)
> **Sub-mech áp dụng:** **B** (capability — `cargo check` + `cargo test state` + `cargo test --test migrate_state_cli`), **C** (migration completeness — `last_scan_at` from legacy ISO MUST equal `last_scan_at` in JSON v1 output; if Architect spec says "convert legacy" then count-preservation check = legacy file had 1 timestamp → JSON v1 file has 1 timestamp NOT-zero), **D** (persistence — ARCHITECTURE §2 + §7 + INVARIANTS §3 INV-LOCAL-002 + README updated), **F** (runtime state — error wording does NOT echo file content into stderr; `grep -E 'ghp_|...'` clean across new code)

---

## Context

### Vấn đề hiện tại

P005 ship `dedup` + P006 ship `append`. Cả 2 đều ĐỌC/GHI state file giả định schema JSON v1. Nhưng trong tarot precedent (P282), existing state file là **single-line ISO-8601** format (no JSON): file content = `2026-05-23T12:00:00Z\n`. Nếu user upgrade từ tarot (bash heredoc) sang `advisory-inbox` binary mà không migrate, `state::read()` sẽ fail `StateReadError::Json` → confusing exit 1.

P007 wires `migrate-state` subcmd per ARCHITECTURE §1 dòng 55-64:

```
advisory-inbox migrate-state --state <FILE> [--dry-run]
→ behavior: Detect format (JSON / single-line ISO / missing) → write JSON schema
→ output:   { "from": "legacy"|"json-v1"|"missing", "to": "json-v1", "seen_count": N }
→ exit:     0 success, 1 format unknown, 2 write error
```

**Three input shapes:**

1. **Missing file** — file does not exist at path. Action: write fresh JSON v1 với `last_scan_at = Utc::now()`, `seen_advisories = []`, `agent_version = ""`. Output `from: "missing"`.
2. **JSON file with schema_version==1** — already migrated. Action: no-op re-write (preserve content). Output `from: "json-v1"`. (Idempotent — user can run repeatedly safe.)
3. **JSON file with schema_version != 1** — unknown future schema. Action: fail with `FormatUnknown` → exit 1. Manual intervention required (out-of-scope for MVP migrate; future P-NNN will add v1→v2 migrator if/when v2 ships).
4. **Single-line ISO-8601 file (legacy tarot format)** — parse content as RFC-3339 timestamp. If OK: build `StateFile { schema_version: 1, last_scan_at: parsed, seen_advisories: vec![], agent_version: String::new() }` → atomic write. Output `from: "legacy"`. Data preserved: timestamp.
5. **Any other content** — not parseable JSON, not parseable ISO-8601 → `FormatUnknown` → exit 1.

**`--dry-run` semantics (cứng):** print the INTENDED output state JSON to stdout (so user can pipe/inspect/diff), **DO NOT touch the file**. File state on disk is identical before and after a `--dry-run` invocation.

**State write path is NEW surface.** P005 added `state::read()`; P006 added `inbox::write_atomic()`. P007 adds `state::write_atomic()` (mirror of `inbox::write_atomic`, per INV-LOCAL-002 reference shape established by P006). This is the **second concrete user** of INV-LOCAL-002 → INVARIANTS doc note updates.

Reference BACKLOG.md P007:
- Lane: Guarded (legacy data migration).
- Scope: Detect format → write JSON v1 → preserve `last_scan_at`.
- Acceptance: 3 fixtures (json/legacy/missing) → all migrate clean.
- Sub-mech checks: B, C (migration completeness — count preserved).

### Giải pháp

**4 unit công việc chính:**

1. **`src/state.rs` — ADD `StateWriteError` + `write_atomic`:**
   - `StateWriteError` enum (thiserror, 1 variant for MVP):
     - `Io { path: PathBuf, source: std::io::Error }` — exit code 2 contract per ARCHITECTURE §1 migrate-state.
     - Note: no `Serialize` variant — `serde_json::to_string_pretty(&StateFile)` cannot fail at runtime for our shape (no `#[serde(serialize_with)]` custom hooks, all fields are infallible-serialize). If Worker discovers serialize CAN fail → add `Json` variant (Tầng 2 self-decide, log Discovery).
   - `pub fn write_atomic(path: &Path, state: &StateFile) -> Result<(), StateWriteError>`:
     1. Serialize: `serde_json::to_string_pretty(state).map_err(...)` — if it fails (shouldn't), propagate as `Io` for MVP (or add `Json` variant per above).
     2. Atomic write per INV-LOCAL-002 — EXACT mirror of `inbox::write_atomic` from P006:
        - `NamedTempFile::new_in(parent)` (parent = `path.parent()?`)
        - `temp.write_all(serialized.as_bytes())?`
        - `temp.as_file().sync_all()?` (fsync data+metadata)
        - `temp.persist(path)?` (atomic rename; extract `PersistError.error` → `Io`)
     3. Append trailing newline to output (`serde_json::to_string_pretty` does NOT add trailing `\n`; ARCHITECTURE §2 fixture has trailing newline by convention — match).
   - Unit tests (≥3):
     - `write_atomic` round-trip: serialize → write → read back via `state::read` → field equality.
     - `write_atomic` no-parent-dir: build `&Path` like `Path::new("nonexistent-root-only")` → `Io` error (InvalidInput per parent.ok_or pattern from P006 inbox.rs).
     - `write_atomic` trailing newline: output file ends with `\n` (read back as bytes, last byte == `b'\n'`).

2. **`src/cli/migrate_state.rs` — stub → real impl:**
   - `pub fn run(state: PathBuf, dry_run: bool) -> anyhow::Result<()>`:
     1. Detect file existence: `match std::fs::metadata(&state)`.
        - `Err(e) where e.kind() == NotFound` → branch MISSING.
        - `Err(other)` → propagate as `StateWriteError::Io` (permission denied, etc. — exit 2).
        - `Ok(_)` → read content: `let raw = std::fs::read_to_string(&state)?` (Io → exit 2).
     2. Branch MISSING:
        - Build fresh: `StateFile { schema_version: 1, last_scan_at: Utc::now(), seen_advisories: Vec::new(), agent_version: String::new() }`.
        - `from = "missing"`, `seen_count = 0`.
        - Atomic-write (unless dry_run).
     3. Branch CONTENT EXISTS — try JSON parse: `serde_json::from_str::<StateFile>(&raw)`.
        - Ok with `schema_version == 1` → `from = "json-v1"`, `seen_count = state.seen_advisories.len()`. Atomic-write (unless dry_run) — even though idempotent, re-write ensures file ends in canonical pretty-printed shape (post-migration normalization).
        - Ok with `schema_version != 1` → return `FormatUnknown` error (exit 1).
        - Err (not JSON) → fall through to legacy parse.
     4. Branch LEGACY (raw is not JSON):
        - Strip whitespace: `let trimmed = raw.trim()`. (Strips trailing `\n` and any surrounding spaces.)
        - Parse: `chrono::DateTime::parse_from_rfc3339(trimmed)` → returns `Result<DateTime<FixedOffset>, ParseError>`.
        - Convert to `DateTime<Utc>`: `parsed.with_timezone(&Utc)`.
        - Build `StateFile { schema_version: 1, last_scan_at: utc, seen_advisories: vec![], agent_version: String::new() }`.
        - `from = "legacy"`, `seen_count = 0` (legacy format had no seen IDs — this is correct, NOT data loss).
        - If parse_from_rfc3339 fails → return `FormatUnknown`.
        - Atomic-write (unless dry_run).
     5. Output JSON to stdout (regardless of dry_run): `serde_json::json!({ "from": <s>, "to": "json-v1", "seen_count": N })` + trailing newline.
     6. If `dry_run == true` AND any write would have happened: ALSO emit the intended `StateFile` JSON to stdout AFTER the summary line (or use `--dry-run` flag to skip write but still print summary — see "dry_run output shape" below).
   - **`FormatUnknown` error type:**
     - Define INSIDE `cli/migrate_state.rs` as a thiserror enum (or anyhow `bail!` with stable error tag):
       ```rust
       #[derive(thiserror::Error, Debug)]
       pub enum MigrateError {
           #[error("state file `{path}` format unrecognised (not JSON v1, not single-line ISO-8601 timestamp)")]
           FormatUnknown { path: std::path::PathBuf },
           #[error("state file `{path}` has unsupported schema_version {found} (expected 1)")]
           UnsupportedSchema { path: std::path::PathBuf, found: u32 },
       }
       ```
     - Worker self-decides: 1 variant `FormatUnknown` covering both un-parseable AND wrong-schema_version, OR 2 variants. Recommendation: **2 variants** (clearer Sếp-facing error wording). Both map to exit 1.

3. **`src/main.rs` dispatch arm — error → exit code map:**
   - Update `Commands::MigrateState { state, dry_run }` dispatch (currently flat passthrough from P001 scaffold):
     ```rust
     Commands::MigrateState { state, dry_run } => {
         if let Err(e) = cli::migrate_state::run(state, dry_run) {
             let code = if let Some(me) = e.downcast_ref::<cli::migrate_state::MigrateError>() {
                 match me {
                     cli::migrate_state::MigrateError::FormatUnknown { .. } => 1,
                     cli::migrate_state::MigrateError::UnsupportedSchema { .. } => 1,
                 }
             } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
                 2
             } else {
                 // serde_json error (legacy raw fail wouldn't surface here — it falls to MigrateError);
                 // io error from read_to_string before classify → exit 2 (write/IO category).
                 2
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```
   - Tail `Ok(())` REQUIRED (P004/P005/P006 precedent).

4. **Fixtures + integration test:**
   - `tests/fixtures/state-legacy.txt`:
     ```
     2026-05-23T12:00:00Z
     ```
     (single line, trailing newline)
   - `tests/fixtures/state-json-v1.json`:
     ```json
     {
       "schema_version": 1,
       "last_scan_at": "2026-05-28T09:51:35Z",
       "seen_advisories": [
         "CVE-2026-9256",
         "GHSA-xxxx-yyyy"
       ],
       "agent_version": "advisory-watch@0.1.0"
     }
     ```
   - `tests/fixtures/state-garbage.txt`:
     ```
     this is not json and not a timestamp
     ```
   - `tests/migrate_state_cli.rs` — ≥5 integration tests:
     - **Test A — Missing file:** point `--state` to non-existent path in tempdir → exit 0, stdout JSON `from: "missing"`, `to: "json-v1"`, `seen_count: 0`. File now exists with valid JSON v1 (read back via `serde_json::from_str::<StateFile>`).
     - **Test B — Legacy:** copy `state-legacy.txt` to tempdir → run migrate-state → exit 0, stdout `from: "legacy"`, `seen_count: 0`. File now contains JSON v1 with `last_scan_at == "2026-05-23T12:00:00Z"` (verify via parse back to DateTime + equality). **Sub-mech C count check: timestamp preserved.**
     - **Test C — JSON v1 already:** copy `state-json-v1.json` to tempdir → run migrate-state → exit 0, stdout `from: "json-v1"`, `seen_count: 2`. File still parses as JSON v1 with seen_count=2 (idempotent re-write OK).
     - **Test D — Garbage:** copy `state-garbage.txt` to tempdir → run migrate-state → exit 1, stderr contains "format" or "unrecognised" or "ISO". File unchanged (no temp file leftover).
     - **Test E — `--dry-run` legacy:** copy `state-legacy.txt` to tempdir → run migrate-state `--dry-run` → exit 0, stdout summary `from: "legacy"`. File content UNCHANGED (still the legacy single-line). No `.tmp` artifact in parent dir.

#### Why add `state::write_atomic` instead of inlining in `cli/migrate_state.rs`?

Same reasoning as P006 `inbox::write_atomic` extraction:
- Future P008 state-backfill writes state too → needs same `write_atomic`.
- Future P009 scan-and-append writes state → same.
- Future P011 MCP `state_backfill` tool → same.
- INV-LOCAL-002 reference shape SHOULD live next to `StateFile` (sibling to `state::read`).
- Centralization prevents per-callsite drift in fsync/persist protocol.

Architecturally consistent with P006: inbox module owns inbox read+write atomic; state module owns state read+write atomic.

#### Why no `--force` flag?

User concern: "what if migrate-state corrupts my file?" — answer: atomic write protocol per INV-LOCAL-002 makes corruption impossible (either old content fully present, OR new content fully present, never partial). No `--force` needed because `--dry-run` provides pre-flight confidence + atomic write provides crash-safety.

Out-of-scope for P007:
- `--backup` flag (write `state.bak` before rename) — premature; atomic write covers crash safety. Add only if Sếp encounters real-world rollback need.
- `--force-overwrite-json-v1` — idempotent re-write covers this; no flag needed.
- v1→v2 schema migration — when v2 ships (post-MVP), add new error variant + new branch. Not P007.

#### `chrono::DateTime::parse_from_rfc3339` — strictness

Per chrono 0.4 API: `DateTime::parse_from_rfc3339(s)` accepts strict RFC 3339 format:
- `2026-05-23T12:00:00Z`  ✅
- `2026-05-23T12:00:00.000Z`  ✅
- `2026-05-23T12:00:00+00:00`  ✅
- `2026-05-23 12:00:00Z` (space separator instead of T) ❌ — rejected
- `2026-05-23T12:00:00` (no zone) ❌ — rejected
- `2026-05-23` (date only) ❌ — rejected (this is NaiveDate territory)

Tarot precedent legacy format = `2026-05-23T12:00:00Z\n` (strict RFC 3339 UTC with `Z`). After `trim()`, parses clean.

**Anchor #X marks** `chrono::DateTime::parse_from_rfc3339` API exists `[needs Worker verify via cargo doc chrono]` because P002 verified `serde` rfc3339 path but not direct `parse_from_rfc3339` call.

#### `dry_run` output shape decision

Architect picks: on `--dry-run`, stdout emits THE SAME summary JSON as non-dry-run (`{ from, to, seen_count }`). User who wants to see the full intended state can omit `--dry-run` and re-run after confidence built. Rationale: keep output shape STABLE (consistency for MCP wrapping, scripting).

If Worker discovers Sếp would prefer dry-run also dumps full StateFile JSON: that's Tầng 2 polish, log Discovery, do NOT change MVP behavior in P007.

#### Why `agent_version = String::new()` for fresh + legacy?

`agent_version` is a free-form string (ARCHITECTURE §2). On migrate from legacy or missing, we don't know what agent populated previously. Empty string `""` is the safe default — caller (advisory-watch agent) will set it on next scan. Alternative defaults considered:
- `"unknown"` — adds noise to JSON.
- `"advisory-inbox/migrate-state@0.1.0"` — claims provenance falsely.
- `String::new()` — neutral, downstream code must handle empty safely (which it should, per defensive read).

Locked decision: empty string.

#### Sub-mech C migration completeness — semantic

For P007 specifically: "count preserved" means timestamp survives legacy→JSON conversion. Apply check in Test B (integration):
```rust
let migrated = std::fs::read_to_string(&state_path).unwrap();
let parsed: StateFile = serde_json::from_str(&migrated).unwrap();
assert_eq!(
    parsed.last_scan_at,
    DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z").unwrap().with_timezone(&Utc)
);
```
This IS the migration-completeness check for the legacy→JSON path. For missing→JSON and JSON-v1→JSON-v1, count check = number of preserved fields = 4 (trivially true).

### Scope

- CHỈ sửa: `src/state.rs` (ADD `StateWriteError` + `pub fn write_atomic` + ≥3 unit tests), `src/cli/migrate_state.rs` (stub → real impl + `MigrateError` enum), `src/main.rs` (update `Commands::MigrateState` dispatch arm).
- CHỈ tạo fixtures: `tests/fixtures/state-legacy.txt`, `tests/fixtures/state-json-v1.json`, `tests/fixtures/state-garbage.txt`.
- CHỈ tạo integration: `tests/migrate_state_cli.rs`.
- CHỈ update docs: `docs/ARCHITECTURE.md` §2 (note state.rs gains `write_atomic`), §5 (P007 scaffold-status entry), §7 (state-write second user of atomic protocol); `docs/CHANGELOG.md` (P007 entry + migration note); `docs/security/INVARIANTS.md` (INV-LOCAL-002 — append "P007 second concrete user — state file"); `README.md` (`migrate-state` quick-start if not covered — Anchor #15 conditional).
- KHÔNG sửa: `src/inbox.rs` (P006 lock), `src/sentinel.rs` (P003 lock), `src/row.rs` (P006 lock — already shipped Display), `src/cli/parse_report.rs` (P004 lock), `src/cli/dedup.rs` (P005 lock), `src/cli/append.rs` (P006 lock), `Cargo.toml` (NO new dep — `tempfile`, `thiserror`, `serde_json`, `chrono`, `anyhow`, `serde` all present per P002-P006 verified anchors).
- KHÔNG tạo: `src/error.rs` (ARCHITECTURE §5 pending — not P007 scope).
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 migrate-state: 0/1/2).
- KHÔNG đổi state schema (`schema_version`, fields). P007 ONLY adds write path — schema is locked from P002.
- KHÔNG đổi `StateFile` shape (P002 lock). KHÔNG đổi `StateReadError` (P005 lock) — `StateWriteError` is NEW sibling.
- KHÔNG đổi `state::read` signature.
- KHÔNG bump `SCHEMA_VERSION` constant.
- KHÔNG add `--force`, `--backup`, `--auto-fix-v2` flags.
- KHÔNG implement v1→v2 schema migration (out-of-scope; v2 doesn't exist yet).
- KHÔNG add concurrency lock (ARCHITECTURE §10 deferred).
- KHÔNG modify `inbox::write_atomic` (P006 lock) — `state::write_atomic` is a SEPARATE function in a SEPARATE module, intentional duplication of small atomic-write boilerplate per INV-LOCAL-002 reference shape.
- KHÔNG xoá `#![allow(dead_code)]` từ `sentinel.rs` (P004 follow-up; cross-phiếu housekeeping; NOT P007 scope).
- KHÔNG move `RowsEnvelope` to shared location (P005/P006 deferred to P009).

### Skills consulted

Architect Read `docs/ticket/P006-append-atomic.md` để tham khảo:
- `InboxError` 2-variant enum shape → reused for `StateWriteError` 1-variant pattern (only `Io` needed in P007; `FormatUnknown` lives in `cli/migrate_state.rs` as it's CLI-domain error, not state-module error).
- `write_atomic` INV-LOCAL-002 reference shape from `src/inbox.rs::write_atomic` — copy verbatim into `src/state.rs::write_atomic`, adjusting only the serialization (vs raw String) and error type.
- anyhow downcast → exit code map idiom (main.rs dispatch arm).
- Integration test idiom: `assert_cmd::Command::cargo_bin` + tempdir copy fixture + assert exit code + stdout substring.

Architect Read `docs/ticket/P005-dedup.md` để tham khảo:
- `StateReadError` shape (Io / Json / SchemaMismatch) → P007's `StateWriteError` mirrors `Io` variant; `MigrateError` (in cli/migrate_state.rs) covers FormatUnknown analogous to read-side SchemaMismatch.
- `state::read()` already enforces `schema_version == 1` → migrate-state's JSON-v1 branch can leverage this OR re-implement own check. Architect picks: re-implement check in migrate-state to provide DIFFERENT error wording (read error = "your state is broken, RUN migrate-state"; migrate error = "your state is unknown schema, see docs"). Worker may consider re-using `state::read` and downcasting `SchemaMismatch` → that's also valid Tầng 2 self-decide.

Architect Read `docs/discoveries/P006.md` để học:
- P006 baseline post-ship: 41 tests (30 unit + 11 integration). P007 target: ≥41 + ≥3 new unit (state::write_atomic) + ≥5 new integration = 49+ tests.
- INV-LOCAL-002 doc drift resolved in P006 (ARCHITECTURE §7 now says `sync_all`). P007 inherits clean ARCHITECTURE §7 reference.
- `as_file().sync_all()` chain confirmed in tempfile-3.27.0 source — Worker can reuse without re-verifying.

Architect Read `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 (covered via P006 phiếu). P007 IS second concrete user → add note via Task 5.

Architect did NOT use context7 for `chrono::DateTime::parse_from_rfc3339` (Tool envelope: architect agent only has Read/Write/Glob; context7 NOT in envelope per CLAUDE.md and §4 preflight). Marked Anchor #10 as `[needs Worker verify via cargo doc chrono]` — fallback explicit.

Architect did NOT use context7 for `serde_json::to_string_pretty`, `serde_json::from_str`, `Utc::now` (well-known APIs already exercised in P002-P006).

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/migrate_state.rs` hiện là stub printf TODO (P001 ship). Signature `pub fn run(state: PathBuf, dry_run: bool) -> Result<()>` per P001 scaffold pattern (analogous to P004/P005/P006 stub shape). | P004 Anchor #1 + P005 Anchor #1 + P006 Anchor #1 (all confirmed identical stub signature pattern across 4 prior phiếu). | `[needs Worker verify]` | ⏳ TO VERIFY (Worker Task 0). |
| 2 | `src/main.rs` có `Commands::MigrateState { state: PathBuf, dry_run: bool }` clap variant + dispatch arm `cli::migrate_state::run(state, dry_run)` từ P001 ship. | P006 Anchor #2 confirmed all 8 dispatch arms present at main.rs lines ~25-65; transitively P007's variant should be there. | `[needs Worker verify]` | ⏳ TO VERIFY. |
| 3 | `Commands::MigrateState` clap variant declares `--state <FILE>` REQUIRED + `--dry-run` flag (boolean, no value). | ARCHITECTURE §1 dòng 55-64 + P001 scaffold ship per CLI surface spec. | `[needs Worker verify]` | ⏳ TO VERIFY via `cargo run -- migrate-state --help`. |
| 4 | `src/state.rs` exports `pub struct StateFile` 4 fields: `schema_version: u32`, `last_scan_at: chrono::DateTime<chrono::Utc>`, `seen_advisories: Vec<String>`, `agent_version: String`. + `pub const SCHEMA_VERSION: u32 = 1`. | P002 phiếu + P005 Discovery Anchor #2 ("state.rs:17-34 — fields exact match"). | `[verified]` | ✅ Pre-verified transitively (P002→P005 chain, unchanged). |
| 5 | `src/state.rs` exports `pub fn read(path: &Path) -> Result<StateFile, StateReadError>` (P005 ship). `StateReadError` enum has variants `Io`, `Json`, `SchemaMismatch`. | P005 Discovery Anchor #3 + DISCOVERIES.md index "state::read enforces schema_version==1". | `[verified]` | ✅ Pre-verified. |
| 6 | `src/state.rs` chưa có `pub fn write_atomic` (P002/P005/P006 không scope state-write). | P005 Discovery Anchor #3 ("`state.rs` KHÔNG có `pub fn read` hoặc `pub fn write`") — read shipped in P005; write still missing post-P006 per BACKLOG progression. | `[unverified]` | ⏳ TO VERIFY (`grep -n "pub fn write" src/state.rs` → expect 0 hits). |
| 7 | `src/inbox.rs` exports `pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError>` (P006 ship). Implementation follows INV-LOCAL-002 exactly. | P006 Discovery: "First concrete user of INV-LOCAL-002 atomic write protocol". | `[verified]` | ✅ Pre-verified — Worker references this as `state::write_atomic` reference shape. |
| 8 | `Cargo.toml` `[dependencies]` có `tempfile = "3"` (line 21), `thiserror = "2"` (line 20), `serde_json = "1"` (line 16), `chrono` (line 17), `anyhow = "1"` (line 19), `serde` derive (line 15). KHÔNG cần add dep. | P006 Anchor #7 verified all 6 deps present (Cargo.toml unchanged post-P006). | `[verified]` | ✅ Pre-verified. |
| 9 | `Cargo.toml` `[dev-dependencies]` có `assert_cmd = "2"` + `predicates = "3"`. | P006 Anchor #8 + P005 Anchor #7. | `[verified]` | ✅ Pre-verified. |
| 10 | `chrono::DateTime::parse_from_rfc3339(s: &str)` → `Result<DateTime<FixedOffset>, ParseError>` exists in `chrono = "0.4"`. `DateTime::with_timezone(&Utc)` converts to `DateTime<Utc>`. | P002 ship confirmed chrono serde RFC3339 path (Anchor #2/#3 P002 Discovery); direct `parse_from_rfc3339` call NOT exercised in code yet. Architect does NOT have context7 in envelope (§4 preflight). | `[needs Worker verify via cargo doc chrono]` | ⏳ TO VERIFY (`cargo doc -p chrono --no-deps` + open `DateTime` page, OR test-compile a probe). |
| 11 | `chrono::Utc::now()` → `DateTime<Utc>` available (no feature flag needed beyond default chrono). | P002 ship — `last_scan_at: DateTime<Utc>` serde RFC3339 exercised. `Utc::now()` standard chrono surface. | `[unverified]` | ⏳ TO VERIFY (`grep -n "Utc::now\|Utc.*now" src/state.rs` — likely zero, since P005 tests construct fixed timestamps). |
| 12 | `tempfile::NamedTempFile::new_in(parent) + write_all + as_file().sync_all() + persist(target)` chain compiles + works (per P006 INV-LOCAL-002 establishment). | P006 Discovery Anchor #9 — chain verified, in production at `src/inbox.rs::write_atomic`. | `[verified]` | ✅ Pre-verified (P006 shipped). |
| 13 | `tests/fixtures/` directory tồn tại sau P006 ship (has `inbox-baseline.md`, `rows-2.json`, `state-3ids.json`, `rows-5.json`, `agent-report-1.md`). | P006 Discovery + P005 Discovery Anchor #11. | `[verified]` | ✅ Pre-verified. |
| 14 | `tests/` directory tồn tại với `parse_report_cli.rs` + `dedup_cli.rs` + `append_cli.rs`. | P006 Discovery files-changed table. | `[verified]` | ✅ Pre-verified. |
| 15 | `README.md` chưa có `migrate-state` quick-start section (P004/P005/P006 covered parse-report/dedup/append respectively; migrate-state untouched). | P006 Discovery — README only added `append` quick-start; prior phiếu did NOT touch migrate-state section. | `[unverified]` | ⏳ TO VERIFY (`grep -n "migrate-state" README.md` — expect at most 1 hit in stub list, not in quick-start section). |
| 16 | `docs/ARCHITECTURE.md` §1 migrate-state subcmd block (dòng 55-64) documents I/O contract correctly: input `--state <FILE>` + `--dry-run` flag; output `{ from, to, seen_count }`; exit 0/1/2. | Architect Read ARCHITECTURE.md dòng 55-64 during load context. | `[verified]` | ✅ Dòng 55-64 exact match per phiếu Context spec. |
| 17 | `docs/ARCHITECTURE.md` §2 (dòng 122-152) state schema documents: JSON shape + legacy format = "Single-line ISO-8601 (no JSON): `2026-05-23T12:00:00Z\n`. Migrate-state subcmd detects + converts." | Architect Read ARCHITECTURE.md §2 during load context. | `[verified]` | ✅ Dòng 148-150 confirms legacy format spec. |
| 18 | `docs/ARCHITECTURE.md` §7 (post-P006) Atomic Write Pattern uses `sync_all()` (not `flush`); INV-LOCAL-002 alignment fix already shipped per P006 Discovery SD-3. | P006 Discovery SD-3 — "Cập nhật ARCHITECTURE §7 để align với INV — doc drift resolved trong P006." | `[verified]` | ✅ Pre-verified (P006 fixed). |
| 19 | `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 lists P006 as first concrete user; P007 will be appended as SECOND concrete user. | P006 Discovery files-changed: "`docs/security/INVARIANTS.md` UPDATE: INV-LOCAL-002 'First concrete user' note". | `[unverified]` | ⏳ TO VERIFY (`grep -n "P006\|first concrete" docs/security/INVARIANTS.md` ≥1 hit). |
| 20 | `src/main.rs` `Commands::MigrateState` dispatch arm currently flat passthrough `cli::migrate_state::run(state, dry_run)` (P001 ship; no error mapping yet). | P006 Discovery — only `Commands::Append` arm was updated to error-map; other arms remain stub passthrough. | `[needs Worker verify]` | ⏳ TO VERIFY. |
| 21 | `src/cli/mod.rs` đã có `pub mod migrate_state;` (P001 scaffold ship 8 subcmd modules). | P006 Anchor #19 confirmed `pub mod append;` at cli/mod.rs:7 — all 8 modules registered per P001. | `[unverified]` | ⏳ TO VERIFY (`grep -n "pub mod migrate_state" src/cli/mod.rs`). |
| 22 | `serde_json::to_string_pretty(&StateFile)` outputs canonical 2-space-indent JSON; serde rfc3339 path for `DateTime<Utc>` emits `"YYYY-MM-DDTHH:MM:SSZ"` exact match for tarot precedent format. | P002 Discovery Anchor #2 (serde rfc3339 verified by Architect via context7); P002 tests roundtrip JSON OK. | `[verified]` | ✅ Pre-verified. |

**Note:** Worker EXECUTE Task 0 runs ~10 grep/check verifications (Anchors #1/#2/#3/#6/#10/#11/#15/#19/#20/#21 → 10 anchors `[needs Worker verify]` + `[unverified]`). If ANY anchor diverges from spec → STOP, escalate, write Discovery Report.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. Anchor #10 (`chrono::parse_from_rfc3339`) flagged `[needs Worker verify via cargo doc chrono]` — Worker re-confirms via cargo doc; fallback if API differs: use `chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")` (less strict but covers tarot legacy format) + log Discovery.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND).
> Sếp đọc lúc nghiệm thu. Cap = 3 turns.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no challenges.**

**Anchor verification (recap):**

| # | Assumption | Result | Evidence |
|---|-----------|--------|---------|
| 1 | `src/cli/migrate_state.rs` is stub `pub fn run(state: PathBuf, dry_run: bool) -> Result<()>` | ✅ | `src/cli/migrate_state.rs` lines 1-12 — stub with TODO println |
| 2 | `src/main.rs` has `Commands::MigrateState { state: PathBuf, dry_run: bool }` variant | ✅ | `src/main.rs:50-57` — variant declared; `src/main.rs:140` — dispatch arm present |
| 3 | `--state <FILE>` REQUIRED + `--dry-run` boolean flag | ✅ | `cargo run -- migrate-state --help` shows `--state <STATE>` (required) + `--dry-run` (boolean flag, no value) |
| 4 | `StateFile` 4 fields + `SCHEMA_VERSION = 1` | ✅ | `src/state.rs:17,25-34` — exact match |
| 5 | `state::read` + `StateReadError` Io/Json/SchemaMismatch | ✅ | `src/state.rs:38-90` — all 3 variants present |
| 6 | `pub fn write_atomic` NOT yet in `src/state.rs` | ✅ | `grep -n "pub fn write\|fn write_atomic" src/state.rs` → 0 hits |
| 7 | `inbox::write_atomic` reference shape shipped in P006 | ✅ | Pre-verified (P006 shipped) |
| 8 | All required Cargo deps present (tempfile, thiserror, serde_json, chrono, anyhow, serde) | ✅ | Pre-verified (P002-P006 chain) |
| 9 | `assert_cmd` + `predicates` in dev-dependencies | ✅ | Pre-verified (P006 Anchor #8) |
| 10 | `DateTime::parse_from_rfc3339` API exists in chrono 0.4 | ✅ | `src/state.rs:100-102` already uses `DateTime::parse_from_rfc3339(...).with_timezone(&Utc)` in existing tests — API confirmed. chrono version = 0.4.44. |
| 11 | `Utc::now()` available (no feature flag) | ✅ | chrono 0.4.44 — standard API. Not yet used in src/ but identical to Utc usage at `src/state.rs:9`. |
| 12 | NamedTempFile chain compiles (P006 INV-LOCAL-002 established) | ✅ | Pre-verified (P006 shipped) |
| 13 | `tests/fixtures/` directory exists post-P006 | ✅ | Pre-verified (P006 Discovery) |
| 14 | `tests/` directory with prior cli integration tests | ✅ | Pre-verified (P006 Discovery) |
| 15 | README does NOT yet have `migrate-state` quick-start section | ✅ | `grep -n "migrate-state" README.md` → 1 hit only in exit-code table (line 53), NOT in quick-start section |
| 16 | ARCHITECTURE §1 migrate-state I/O contract correct | ✅ | Pre-verified by Architect |
| 17 | ARCHITECTURE §2 legacy format spec correct | ✅ | Pre-verified by Architect |
| 18 | ARCHITECTURE §7 atomic write uses `sync_all()` — P006 fix | ✅ | Pre-verified (P006 fixed) |
| 19 | INVARIANTS.md §3 INV-LOCAL-002 has P006 first-user note | ✅ | `docs/security/INVARIANTS.md:117` — "First concrete user: src/inbox.rs::write_atomic (P006, shipped 2026-05-28)" |
| 20 | `Commands::MigrateState` dispatch arm is flat passthrough | ✅ | `src/main.rs:140` — `cli::migrate_state::run(state, dry_run)` (no error mapping yet) |
| 21 | `pub mod migrate_state;` registered in `src/cli/mod.rs` | ✅ | `src/cli/mod.rs:10` — confirmed |
| 22 | `serde_json::to_string_pretty` output compatible | ✅ | Pre-verified (P002 roundtrip tests) |

**Baseline test count (pre-P007):** 41 tests (30 unit + 4 dedup_cli + 4 append_cli + 3 parse_report_cli). Target post-P007: ≥49.

**Objections:** None.

**Status:** ✅ WORKER ACCEPTED V1 — no challenges. Ready for Chủ nhà approval gate.

### Turn 1 — Architect Response
*(Architect fill khi invoked RESPOND mode.)*

- [O1.1] → ACCEPT / DEFEND / REFRAME (Tầng 2) / DEFER TO SẾP → action taken
- [O1.2] → …

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V<N>
- Total turns: <count>
- Approved: [date] — code execution may begin

---

## Debug Log (advisory-inbox specific)

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (Sub-mechanism A-F checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | (no hook/cron in this phiếu) | N/A | | N/A |
| B (capability) | `cargo check` | exit 0, 0 warnings | | |
| B (capability) | `cargo test state` | ≥11 tests pass (8 P002/P005 + ≥3 new write_atomic) | | |
| B (capability) | `cargo test --test migrate_state_cli` | ≥5 integration tests pass (missing / legacy / json-v1 / garbage / dry-run) | | |
| B (capability) | `cargo run --quiet -- migrate-state --state <legacy-fixture-in-tempdir>` | exit 0, stdout JSON `from: "legacy"`, file now JSON v1 with last_scan_at preserved | | |
| B (capability) | `cargo run --quiet -- migrate-state --state <garbage-fixture>` | exit 1, stderr "format" | | |
| B (capability) | `cargo run --quiet -- migrate-state --state <legacy> --dry-run` | exit 0, stdout same JSON; file UNCHANGED on disk | | |
| C (migration completeness) | Test B — `last_scan_at` value parsed back equals `2026-05-23T12:00:00Z` (UTC) | exact timestamp match | | |
| C (migration completeness) | Test C — `seen_count` in stdout matches `seen_advisories.len()` from input fixture (= 2) | match | | |
| D (persistence) | `grep -l "migrate-state\|write_atomic" docs/ARCHITECTURE.md` | ≥1 hit each | | |
| D (persistence) | `grep -n "P007\|state.rs" docs/security/INVARIANTS.md` | ≥1 hit (INV-LOCAL-002 second user note) | | |
| D (persistence) | `grep -n "migrate-state" README.md` | ≥1 hit (quick-start section, conditional per Anchor #15) | | |
| D (persistence) | `grep -n "P007" docs/CHANGELOG.md` | ≥1 hit (entry at top) | | |
| E (env drift) | `cargo update --dry-run` | no surprise bump | | |
| E (env drift) | `cargo build --release` from clean target | exit 0, 0 warnings | | |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/state.rs src/cli/migrate_state.rs` | 0 hits | | |
| F (runtime state) | `bash scripts/session-start-banner.sh` | no forbidden key detected | | |
| F (runtime state) | Forbidden write pattern: `grep -E "OpenOptions::append\|std::fs::write.*state\|std::fs::rename" src/state.rs src/cli/migrate_state.rs` | 0 code-hits (doc comment OK) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + D + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code.

**Lệnh chạy (verify Anchors #1, #2, #3, #6, #10, #11, #15, #19, #20, #21):**

```bash
# Anchor #1 — migrate_state stub state
cat src/cli/migrate_state.rs

# Anchor #2 — main.rs MigrateState variant + dispatch
grep -n "MigrateState" src/main.rs

# Anchor #3 — migrate-state clap help
cargo run --quiet -- migrate-state --help 2>&1 | head -20

# Anchor #6 — state.rs no existing write_atomic
grep -n "pub fn write\|fn write_atomic" src/state.rs

# Anchor #10 — chrono parse_from_rfc3339 API
# Option A: cargo doc inspect
# cargo doc --no-deps -p chrono  # then open DateTime page
# Option B: compile-test probe
cat > /tmp/p007-chrono-probe.rs <<'EOF'
use chrono::{DateTime, Utc};
fn _probe() -> Result<(), chrono::ParseError> {
    let s = "2026-05-23T12:00:00Z";
    let dt = DateTime::parse_from_rfc3339(s)?;
    let _utc: DateTime<Utc> = dt.with_timezone(&Utc);
    Ok(())
}
EOF
# Don't compile; just shape-check. If Worker doubts → cargo doc.

# Anchor #11 — Utc::now usage
grep -n "Utc::now\|Utc.*now" src/state.rs src/

# Anchor #15 — README migrate-state coverage
grep -n "migrate-state" README.md

# Anchor #19 — INVARIANTS first-user note from P006
grep -n "P006\|first concrete" docs/security/INVARIANTS.md

# Anchor #20 — main.rs MigrateState dispatch arm shape
grep -n -A 3 "MigrateState" src/main.rs

# Anchor #21 — cli/mod.rs registers migrate_state
grep -n "pub mod migrate_state" src/cli/mod.rs

# Sub-mech F preflight
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml || echo "clean"

# Baseline test count (post-P006)
cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l
# Expect ~41 (30 unit + 11 integration). Phiếu target: ≥49 after P007 (+3 unit + ≥5 integration).
```

**Output:** Worker fill vào Debate Log Turn 1 Anchor table.

**Hard Stop triggers:**
- Anchor #2 — nếu `Commands::MigrateState` không tồn tại HOẶC field naming khác (`state: PathBuf` + `dry_run: bool`) → STOP, escalate P001 drift.
- Anchor #3 — nếu `--state` không REQUIRED HOẶC `--dry-run` không là boolean flag → STOP, escalate ARCHITECTURE §1 drift.
- Anchor #6 — nếu `pub fn write_atomic` ĐÃ tồn tại trong `src/state.rs` → STOP, unexpected (P002/P005/P006 không scope). Investigate before proceeding.
- Anchor #10 — nếu `DateTime::parse_from_rfc3339` API differs (e.g., renamed in chrono 0.5) → Worker fallback to `DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")` + log Discovery. Tầng 2 self-decide if format strictness same.
- Anchor #20 — nếu `Commands::MigrateState` dispatch arm ALREADY error-mapped (e.g., touched by prior PR) → STOP, investigate; phiếu spec assumes flat passthrough start state.
- Anchor #21 — nếu `pub mod migrate_state;` MISSING from `cli/mod.rs` → STOP, escalate P001 drift.

---

### Task 1: `src/state.rs` — ADD `StateWriteError` + `write_atomic` + tests

**File:** `src/state.rs`

**Tìm** (Worker grep verify post-Task 0):
```rust
// Existing P002+P005 surface:
// - pub const SCHEMA_VERSION: u32 = 1;
// - pub struct StateFile { ... 4 fields ... }
// - pub fn read(path: &Path) -> Result<StateFile, StateReadError>
// - #[derive(thiserror::Error, Debug)] pub enum StateReadError { Io, Json, SchemaMismatch }
// - #[cfg(test)] mod tests { ... 8 existing tests ... }
```

**Thêm** (BEFORE `#[cfg(test)] mod tests`):

```rust
use std::io::Write;
use std::path::PathBuf;

use tempfile::NamedTempFile;

/// Errors raised by atomic state-file write.
///
/// Exit-code mapping (caller's responsibility in `main.rs`):
/// - [`StateWriteError::Io`] → exit code 2 (per ARCHITECTURE §1 migrate-state).
#[derive(thiserror::Error, Debug)]
pub enum StateWriteError {
    #[error("state file `{path}` I/O failure: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Atomically write `state` to `path` per INV-LOCAL-002 protocol:
/// temp file in SAME parent directory → fsync data+metadata → atomic rename.
///
/// Output is `serde_json::to_string_pretty` (2-space indent) with trailing newline.
///
/// This is the SECOND concrete user of INV-LOCAL-002 (after `inbox::write_atomic`
/// shipped in P006). Reference shape matches `src/inbox.rs::write_atomic` exactly.
pub fn write_atomic(path: &std::path::Path, state: &StateFile) -> Result<(), StateWriteError> {
    let parent = path.parent().ok_or_else(|| StateWriteError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        ),
    })?;

    // Serialize. `serde_json::to_string_pretty` failures map to Io for MVP
    // (StateFile has no custom #[serde(serialize_with)] hooks; serialize is
    // infallible for our shape). If Worker discovers a real serialize failure,
    // add `Json` variant to StateWriteError as Tầng 2 self-decide.
    let mut serialized = serde_json::to_string_pretty(state).map_err(|source| {
        StateWriteError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::Other, source.to_string()),
        }
    })?;
    serialized.push('\n');

    let mut temp = NamedTempFile::new_in(parent).map_err(|source| StateWriteError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.write_all(serialized.as_bytes()).map_err(|source| StateWriteError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.as_file().sync_all().map_err(|source| StateWriteError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.persist(path).map_err(|e| StateWriteError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}
```

**Thêm tests** (inside existing `#[cfg(test)] mod tests`):

```rust
#[test]
fn write_atomic_round_trip() {
    use chrono::TimeZone;
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("state.json");
    let original = StateFile {
        schema_version: 1,
        last_scan_at: chrono::Utc.with_ymd_and_hms(2026, 5, 28, 9, 51, 35).unwrap(),
        seen_advisories: vec!["CVE-2026-1".to_string(), "CVE-2026-2".to_string()],
        agent_version: "test@1.0".to_string(),
    };
    write_atomic(&target, &original).expect("write atomic");
    let read_back = read(&target).expect("read back");
    assert_eq!(read_back.schema_version, original.schema_version);
    assert_eq!(read_back.last_scan_at, original.last_scan_at);
    assert_eq!(read_back.seen_advisories, original.seen_advisories);
    assert_eq!(read_back.agent_version, original.agent_version);
}

#[test]
fn write_atomic_trailing_newline() {
    use chrono::TimeZone;
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("state.json");
    let state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::Utc.with_ymd_and_hms(2026, 5, 28, 0, 0, 0).unwrap(),
        seen_advisories: vec![],
        agent_version: String::new(),
    };
    write_atomic(&target, &state).expect("write atomic");
    let bytes = std::fs::read(&target).expect("read raw");
    assert_eq!(bytes.last().copied(), Some(b'\n'), "output should end with newline");
}

#[test]
fn write_atomic_no_parent_dir_errors() {
    use chrono::TimeZone;
    let state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::Utc.with_ymd_and_hms(2026, 5, 28, 0, 0, 0).unwrap(),
        seen_advisories: vec![],
        agent_version: String::new(),
    };
    // Root path "/" has no parent in `Path::parent()` semantics on Unix → None.
    // Use a plain "filename" with no parent component.
    let bad = std::path::Path::new("just-a-filename-no-parent");
    let err = write_atomic(bad, &state);
    // We expect either:
    //   (a) parent() returns Some("") (empty path) → NamedTempFile::new_in("") may
    //       succeed by creating in CWD — then test would NOT error (acceptable, Tầng 2 self-decide what to assert here).
    //   (b) parent() returns None → InvalidInput error.
    // Worker probes actual behavior; if (a), this test becomes a no-op assertion or
    // is replaced with a more reliable "permission-denied" scenario. Tầng 2 OK to
    // skip this test if platform-dependent.
    // For deterministic CI: assert "either OK (file created in CWD then cleaned) OR Io error".
    if let Ok(()) = err {
        // Clean up file that landed in CWD
        let _ = std::fs::remove_file(bad);
    }
}
```

**Lưu ý:**
- `StateWriteError` has ONE variant for MVP. If Worker discovers `to_string_pretty` actually fails (e.g., due to NaN in a future field), add `Json` variant — Tầng 2 self-decide.
- `write_atomic` signature mirrors `inbox::write_atomic` exactly: `(path: &Path, content) -> Result<(), Error>`. Content here is `&StateFile` (not `&str`) because state.rs owns serialization.
- Trailing newline append (`serialized.push('\n')`) — matches ARCHITECTURE §2 fixture convention. Worker preserves on round-trip.
- Third test (`write_atomic_no_parent_dir_errors`) flagged Tầng 2 — platform-dependent. Worker decides skip/keep. NOT a Hard Stop.
- NO `unsafe { }` block — INV-LOCAL-001.

---

### Task 2: `src/cli/migrate_state.rs` — stub → real impl

**File:** `src/cli/migrate_state.rs`

**Tìm** (current P001 stub):
```rust
use std::path::PathBuf;

use anyhow::Result;

pub fn run(state: PathBuf, dry_run: bool) -> Result<()> {
    println!("TODO: migrate-state subcmd ...");
    Ok(())
}
```

**Thay bằng:**

```rust
//! `migrate-state` subcommand — detect legacy state file formats and
//! convert to JSON v1 schema. Idempotent for JSON v1 input.
//!
//! See `docs/ARCHITECTURE.md` §1 (CLI surface) and §2 (state schema)
//! for the full I/O contract.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use thiserror::Error;

use crate::state::{self, StateFile, SCHEMA_VERSION};

/// CLI-level errors specific to migrate-state semantics.
///
/// Both variants map to exit code 1 (per ARCHITECTURE §1 migrate-state
/// "format unknown" semantic).
#[derive(Error, Debug)]
pub enum MigrateError {
    #[error("state file `{path}` format unrecognised (not JSON v1, not single-line ISO-8601 timestamp)")]
    FormatUnknown { path: PathBuf },
    #[error("state file `{path}` has unsupported schema_version {found} (expected {expected})")]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
}

pub fn run(state_path: PathBuf, dry_run: bool) -> Result<()> {
    // 1. Detect file existence.
    let raw = match std::fs::read_to_string(&state_path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            // Wrap as StateWriteError::Io so the dispatch arm can downcast → exit 2.
            return Err(state::StateWriteError::Io {
                path: state_path.clone(),
                source: e,
            }
            .into());
        }
    };

    // 2. Branch on existence + content.
    let (from_label, new_state) = match raw {
        None => {
            // MISSING: fresh JSON v1.
            (
                "missing",
                StateFile {
                    schema_version: SCHEMA_VERSION,
                    last_scan_at: Utc::now(),
                    seen_advisories: Vec::new(),
                    agent_version: String::new(),
                },
            )
        }
        Some(content) => {
            // Try JSON parse first.
            match serde_json::from_str::<StateFile>(&content) {
                Ok(parsed) => {
                    if parsed.schema_version != SCHEMA_VERSION {
                        return Err(MigrateError::UnsupportedSchema {
                            path: state_path.clone(),
                            found: parsed.schema_version,
                            expected: SCHEMA_VERSION,
                        }
                        .into());
                    }
                    ("json-v1", parsed)
                }
                Err(_json_err) => {
                    // Fall through to legacy single-line ISO parse.
                    let trimmed = content.trim();
                    match DateTime::parse_from_rfc3339(trimmed) {
                        Ok(parsed_dt) => {
                            let utc = parsed_dt.with_timezone(&Utc);
                            (
                                "legacy",
                                StateFile {
                                    schema_version: SCHEMA_VERSION,
                                    last_scan_at: utc,
                                    seen_advisories: Vec::new(),
                                    agent_version: String::new(),
                                },
                            )
                        }
                        Err(_iso_err) => {
                            return Err(MigrateError::FormatUnknown {
                                path: state_path.clone(),
                            }
                            .into());
                        }
                    }
                }
            }
        }
    };

    // 3. Write (unless dry-run).
    if !dry_run {
        state::write_atomic(&state_path, &new_state)
            .with_context(|| format!("writing migrated state to `{}`", state_path.display()))?;
    }

    // 4. Emit summary JSON to stdout.
    let summary = json!({
        "from": from_label,
        "to": "json-v1",
        "seen_count": new_state.seen_advisories.len(),
    });
    println!("{}", summary);

    Ok(())
}
```

**Lưu ý:**
- Renamed param `state` → `state_path` to avoid shadowing `crate::state` module import (P005 precedent — same fix Worker applied in `cli/dedup.rs`).
- `anyhow::Context::with_context` used to add path context to `state::write_atomic` errors before bubble — main.rs downcast still works (anyhow preserves the inner `StateWriteError` for `downcast_ref`).
- `serde_json::json!` macro emits keys alphabetical (`from` → `seen_count` → `to`) — integration tests MUST use substring match, not exact JSON equality (P004/P005/P006 precedent).
- Order of detection: JSON parse FIRST (more specific failure mode), then legacy ISO parse, then FormatUnknown. Rationale: a corrupted JSON v1 file is more diagnosable than a corrupted legacy timestamp — surfacing JSON parse error first via fall-through avoids "legacy detected" mis-classification of garbled JSON.
  - Subtle case: if file content is `2026-05-23T12:00:00Z` (legacy), `serde_json::from_str::<StateFile>` returns Err (not an object) → fall through to legacy parser → Ok → labeled "legacy". ✅ Correct.
  - If file content is `{...}` valid JSON but schema_version=99 → returns `UnsupportedSchema` (exit 1). ✅ Correct — user must consult docs / wait for v1→v2 migrator.
- Dry-run does NOT call `write_atomic`. File on disk is bit-identical pre/post. Integration Test E verifies via byte comparison.
- For `dry_run == true` AND `from == "missing"`: we don't write the file, but we still emit summary JSON (`from: "missing"`, `seen_count: 0`). User can re-run without `--dry-run` to actually create.
- NO `unsafe { }` block.

---

### Task 3: `src/main.rs` dispatch arm — error → exit code map

**File:** `src/main.rs`

**Tìm** (current P001 scaffold passthrough — Worker confirm post-Task 0 via Anchor #20):

```rust
Commands::MigrateState { state, dry_run } => {
    cli::migrate_state::run(state, dry_run)
}
```

**Thay bằng:**

```rust
Commands::MigrateState { state, dry_run } => {
    if let Err(e) = cli::migrate_state::run(state, dry_run) {
        let code = if e.downcast_ref::<cli::migrate_state::MigrateError>().is_some() {
            1
        } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
            2
        } else {
            // Fallback: unexpected error category → exit 2 (write/IO bucket).
            2
        };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

**Lưu ý:**
- Pattern matches `Commands::Append` arm shipped in P006 + `Commands::Dedup` arm shipped in P005.
- Both `MigrateError` variants (FormatUnknown, UnsupportedSchema) map to exit 1. No need for nested match.
- `StateWriteError::Io` → exit 2.
- Tail `Ok(())` REQUIRED per main.rs match-arm uniformity (P004 Turn 1 O1.1 precedent).
- Use `e.downcast_ref::<T>().is_some()` (boolean check) instead of `if let Some(_)` to avoid unused variable warning.

---

### Task 4: Fixtures + integration test

**File:** `tests/fixtures/state-legacy.txt` (NEW)

**Content** (exactly — single line, trailing newline):
```
2026-05-23T12:00:00Z
```

**File:** `tests/fixtures/state-json-v1.json` (NEW)

**Content:**
```json
{
  "schema_version": 1,
  "last_scan_at": "2026-05-28T09:51:35Z",
  "seen_advisories": [
    "CVE-2026-9256",
    "GHSA-xxxx-yyyy"
  ],
  "agent_version": "advisory-watch@0.1.0"
}
```

**File:** `tests/fixtures/state-garbage.txt` (NEW)

**Content:**
```
this is not json and not a timestamp
```

**File:** `tests/migrate_state_cli.rs` (NEW)

**Skeleton:**

```rust
//! Integration tests for `advisory-inbox migrate-state` subcommand.

use std::path::PathBuf;

use assert_cmd::Command;
use chrono::{DateTime, Utc};
use predicates::str::contains;

/// Helper: copy a fixture file into a tempdir + return the destination path.
fn copy_fixture_to_tempdir(
    fixture_name: &str,
    dir: &tempfile::TempDir,
    target_name: &str,
) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name);
    let dest = dir.path().join(target_name);
    std::fs::copy(&src, &dest).expect("copy fixture");
    dest
}

#[test]
fn migrate_missing_writes_fresh_json_v1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("state.json");
    assert!(!target.exists(), "precondition: target should not exist");

    let assert = Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert();
    assert
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("missing"))
        .stdout(contains("\"to\""))
        .stdout(contains("json-v1"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("0"));

    // File now exists with valid JSON v1.
    assert!(target.exists(), "target should be created");
    let content = std::fs::read_to_string(&target).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
    assert_eq!(parsed["schema_version"], 1);
    assert!(parsed["last_scan_at"].is_string());
    assert!(parsed["seen_advisories"].is_array());
    assert_eq!(parsed["seen_advisories"].as_array().unwrap().len(), 0);
}

#[test]
fn migrate_legacy_preserves_timestamp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-legacy.txt", &dir, "state");

    let assert = Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert();
    assert
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("legacy"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("0"));

    // SUB-MECH C — timestamp preserved.
    let migrated = std::fs::read_to_string(&target).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&migrated).expect("parse JSON");
    let last_scan_at = parsed["last_scan_at"].as_str().expect("last_scan_at string");
    let dt = DateTime::parse_from_rfc3339(last_scan_at).expect("parse rfc3339");
    let expected = DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
        .expect("parse expected")
        .with_timezone(&Utc);
    assert_eq!(dt.with_timezone(&Utc), expected, "timestamp must survive migration");
}

#[test]
fn migrate_json_v1_idempotent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-json-v1.json", &dir, "state.json");

    let assert = Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert();
    assert
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("json-v1"))
        .stdout(contains("\"seen_count\""))
        .stdout(contains("2"));

    // File still parses as JSON v1 with seen_count = 2.
    let migrated = std::fs::read_to_string(&target).expect("read");
    let parsed: serde_json::Value = serde_json::from_str(&migrated).expect("parse JSON");
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["seen_advisories"].as_array().unwrap().len(), 2);
}

#[test]
fn migrate_garbage_errors_exit_1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-garbage.txt", &dir, "state");

    // Capture pre-migration content for unchanged-file assertion.
    let before = std::fs::read_to_string(&target).expect("read before");

    let assert = Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .assert();
    assert.failure().code(1).stderr(contains("format").or(contains("ISO")));

    // File content unchanged (no partial-write, no overwrite).
    let after = std::fs::read_to_string(&target).expect("read after");
    assert_eq!(before, after, "garbage input should leave file untouched");
}

#[test]
fn migrate_dry_run_legacy_no_file_change() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = copy_fixture_to_tempdir("state-legacy.txt", &dir, "state");

    let before = std::fs::read_to_string(&target).expect("read before");

    let assert = Command::cargo_bin("advisory-inbox")
        .expect("cargo bin")
        .arg("migrate-state")
        .arg("--state")
        .arg(&target)
        .arg("--dry-run")
        .assert();
    assert
        .success()
        .stdout(contains("\"from\""))
        .stdout(contains("legacy"));

    // File content UNCHANGED on disk.
    let after = std::fs::read_to_string(&target).expect("read after");
    assert_eq!(before, after, "dry-run must not touch file");

    // No .tmp leftover in tempdir.
    let leftover = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp")
        })
        .count();
    assert_eq!(leftover, 0, "no temp artifact should remain");
}
```

**Lưu ý:**
- 5 integration tests covering: missing / legacy / json-v1 / garbage / dry-run.
- Test B (legacy) is the **Sub-mech C migration completeness check** — timestamp round-trip via `parse_from_rfc3339` comparison.
- `predicates::str::contains(...).or(...)` for stderr — Worker verifies actual stderr wording matches one of the two patterns (`format` from `FormatUnknown` Display, or `ISO` from same Display). If exact wording shifts due to Worker self-decide on error message, adjust test substring.
- `predicates::BooleanPredicate` (the `.or()` combinator) is provided by predicates 3 — Worker verifies via cargo doc if uncertain (Tầng 2 self-decide test wiring).
- `tempfile::TempDir` from `tempfile = "3"` already in deps.
- No fixture I/O in unit tests; only integration test `copy_fixture_to_tempdir` helper reads `CARGO_MANIFEST_DIR/tests/fixtures/`.

---

### Task 5: Docs updates (Tầng 1 — security boundary touched)

**File:** `docs/ARCHITECTURE.md`

**§2 — State Schema (after the "Legacy format" subsection, ADD):**

```markdown
### State write path (post-P007)

`src/state.rs` exports `pub fn write_atomic(path, &StateFile) -> Result<(), StateWriteError>`
per INV-LOCAL-002 atomic-write protocol. Output format: `serde_json::to_string_pretty`
(2-space indent) with trailing newline. Second concrete user of INV-LOCAL-002 (first:
`src/inbox.rs::write_atomic` from P006).
```

**§5 — Module Layout — scaffold status block, append:**

```markdown
- P007: `state.rs` gains `pub fn write_atomic` + `StateWriteError` (Io variant). `cli/migrate_state.rs` wired (file existence detect → JSON parse / legacy ISO parse / FormatUnknown). `MigrateError` enum (FormatUnknown + UnsupportedSchema) in `cli/migrate_state.rs`. Second concrete user of INV-LOCAL-002 (state-write path).
```

**File:** `docs/CHANGELOG.md`

**Add entry at TOP** (per CHANGELOG conventions from P002-P006):

```markdown
## P007 — migrate-state subcmd (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded

### Added

- `state::write_atomic(path, &StateFile)` — second concrete user of INV-LOCAL-002 atomic-write protocol (after P006 inbox). 3 unit tests.
- `state::StateWriteError` enum (Io variant) — exit code 2 contract per ARCHITECTURE §1.
- `cli/migrate_state.rs` real impl: detects missing / JSON v1 / legacy single-line ISO / garbage; preserves `last_scan_at` across legacy → JSON v1 conversion; `--dry-run` flag does NOT touch file.
- `cli/migrate_state::MigrateError` enum: `FormatUnknown` + `UnsupportedSchema` (both → exit 1).
- 5 new integration tests (`tests/migrate_state_cli.rs`).
- 3 new fixtures (`tests/fixtures/state-legacy.txt`, `state-json-v1.json`, `state-garbage.txt`).

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

- `Commands::MigrateState` main.rs dispatch arm now error-maps `MigrateError` → exit 1, `StateWriteError::Io` → exit 2.
- `docs/ARCHITECTURE.md` §2 — added "State write path (post-P007)" subsection.
- `docs/ARCHITECTURE.md` §5 — P007 scaffold-status entry.
- `docs/security/INVARIANTS.md` — INV-LOCAL-002 note: P007 second concrete user.

### Sub-mech checks
- B (cargo check + cargo test state + cargo test --test migrate_state_cli): ✅
- C (migration completeness — last_scan_at preserved): ✅ verified in Test B.
- D (persistence — ARCHITECTURE / INVARIANTS / CHANGELOG / README updated): ✅
- F (no token leak — grep clean across new code): ✅
```

**File:** `docs/security/INVARIANTS.md`

**INV-LOCAL-002 section — UPDATE** the "concrete users" note (Worker grep-locates the existing P006 note added per P006 Discovery, then APPENDs):

```markdown
**Concrete users:**
- P006 — `src/inbox.rs::write_atomic` (inbox markdown write path).
- P007 — `src/state.rs::write_atomic` (state JSON write path). Reference shape mirrored exactly from P006.
```

(Exact wording depends on what P006 shipped — Worker reads INVARIANTS.md, locates the existing P006 "First concrete user" note, and appends P007 as second user in the same list/paragraph. If P006 used different wording — Worker harmonizes; this is Tầng 2 self-decide cosmetic.)

**File:** `README.md`

**Add section** (after the `append` quick-start added by P006, before any "Other subcommands" stub list):

```markdown
### `migrate-state`

Convert legacy single-line ISO-8601 state file to JSON v1 schema. Idempotent for files already in JSON v1.

```
advisory-inbox migrate-state --state <FILE> [--dry-run]
```

**Behaviors:**
- File missing → creates fresh JSON v1 (`last_scan_at = now`, empty seen_advisories).
- File is JSON v1 already → no-op re-write (idempotent).
- File is single-line ISO-8601 timestamp (legacy tarot format) → converts to JSON v1, preserves timestamp.
- File is anything else → exit 1 (format unknown).

**Flags:**
- `--dry-run` — print intended `{from, to, seen_count}` summary, but do NOT modify file.

**Output (stdout JSON):**
```json
{"from": "legacy", "to": "json-v1", "seen_count": 0}
```

**Exit codes:**
- `0` — success
- `1` — format unknown (file content not parseable as JSON v1 or single-line ISO)
- `2` — write error (permission denied, disk full, etc.)
```

**Lưu ý:**
- Anchor #15 conditional: if Worker discovers README already has a `migrate-state` section (unlikely per phiếu spec), update in-place instead of add.
- Anchor #19 conditional: if INVARIANTS.md does NOT have a "concrete users" subsection yet (only inline mention), Worker creates one — Tầng 2 self-decide format. Goal: grep `P007|state.rs` returns ≥1 hit.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/state.rs` | Task 1: ADD `StateWriteError` enum + `pub fn write_atomic` + 3 unit tests |
| `src/cli/migrate_state.rs` | Task 2: stub → real impl + `MigrateError` enum |
| `src/main.rs` | Task 3: update `Commands::MigrateState` dispatch arm with error → exit code map |
| `tests/fixtures/state-legacy.txt` | Task 4: NEW — single-line ISO timestamp |
| `tests/fixtures/state-json-v1.json` | Task 4: NEW — already migrated state |
| `tests/fixtures/state-garbage.txt` | Task 4: NEW — unparseable content |
| `tests/migrate_state_cli.rs` | Task 4: NEW — 5 integration tests |
| `docs/ARCHITECTURE.md` | Task 5: §2 state write path subsection + §5 P007 scaffold-status entry |
| `docs/CHANGELOG.md` | Task 5: P007 entry at top |
| `docs/security/INVARIANTS.md` | Task 5: INV-LOCAL-002 — P007 second concrete user appended |
| `README.md` | Task 5: `migrate-state` quick-start section |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/inbox.rs` | `write_atomic` shape unchanged (P006 lock); state.rs::write_atomic mirrors but does NOT modify inbox.rs. |
| `src/row.rs` | `AdvisoryRow` + `Status` + `Severity` + Display impls unchanged (P002/P006 lock). |
| `src/sentinel.rs` | Unchanged (P003 lock). |
| `src/cli/parse_report.rs` | Unchanged (P004 lock). |
| `src/cli/dedup.rs` | Unchanged (P005 lock). |
| `src/cli/append.rs` | Unchanged (P006 lock). |
| `src/cli/mod.rs` | `pub mod migrate_state;` already registered (P001 ship — verify Anchor #21). |
| `Cargo.toml` | NO dep changes — `tempfile`, `chrono`, `thiserror`, `serde_json`, `anyhow`, `serde`, `assert_cmd`, `predicates` all already present. |
| `src/state.rs` `StateFile` shape | NOT modified — P007 only ADDs sibling `StateWriteError` + `write_atomic`. |
| `src/state.rs` `StateReadError` | NOT modified (P005 lock). |
| `src/state.rs` `SCHEMA_VERSION` | NOT bumped — still `= 1`. |

---

## Luật chơi (Constraints)

1. **No new Cargo dep.** All required crates (`tempfile`, `chrono`, `thiserror`, `serde_json`, `anyhow`, `serde`, `assert_cmd`, `predicates`) already in Cargo.toml per P002-P006 verified anchors.
2. **INV-LOCAL-002 atomic-write protocol** — `state::write_atomic` MUST follow `NamedTempFile::new_in(parent) → write_all → as_file().sync_all() → persist(target)` exactly per P006 `inbox::write_atomic` reference shape. Forbidden: `std::fs::write`, `OpenOptions::append`, `std::fs::rename` outside `tempfile::persist`.
3. **`--dry-run` flag — file NEVER touched.** Integration Test E verifies byte-identity pre/post invocation.
4. **Migration must preserve `last_scan_at`** (Sub-mech C). Integration Test B asserts via `DateTime::parse_from_rfc3339` round-trip equality. Legacy `seen_advisories` initialized as empty `Vec::new()` is CORRECT (legacy format had no IDs — NOT data loss).
5. **No schema bump.** `SCHEMA_VERSION` constant stays `= 1`. Any need to migrate JSON v1 → v2 is OUT OF SCOPE for P007 — when v2 ships, a separate phiếu adds the v1→v2 path.
6. **Stay within current CLI surface.** No new flags beyond `--state` and `--dry-run`. NO `--force`, `--backup`, `--auto-fix-v2`, `--quiet`.
7. **No `unsafe { }` block.** INV-LOCAL-001.
8. **Docs Gate Tầng 1 (AUTO).** Security boundary touched (state file write path = filesystem persistence). CHANGELOG + ARCHITECTURE + INVARIANTS + README updates ALL mandatory. `docs-gate --all --verbose` must pass.
9. **No `unwrap()` / `expect()`** in production code paths in `cli/migrate_state.rs` or `state::write_atomic`. Tests OK to use `expect("...")`.
10. **Error messages do NOT leak file content.** Path is OK (Sếp typed it knowingly); raw content of file MUST NOT appear in stderr (e.g., do NOT print the malformed JSON content into "format unknown" error). Sub-mech F.
11. **Match-arm uniformity in main.rs.** All match arms either return `Ok(())` directly OR end with `Ok(())` after `process::exit` branch (P004 Turn 1 O1.1 precedent).
12. **Test count target:** Post-P007 ≥ 49 tests (P006 baseline 41 + ≥3 unit state + ≥5 integration migrate_state).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (target ≥ 49 tests)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] `cargo run --quiet -- migrate-state --help` — shows `--state <STATE>` REQUIRED + `--dry-run` flag, exit 0.
- [ ] Copy `tests/fixtures/state-legacy.txt` to `/tmp/test-state` → `cargo run --quiet -- migrate-state --state /tmp/test-state` → exit 0, stdout JSON with `from: "legacy"`, `/tmp/test-state` now contains pretty-printed JSON v1 with `last_scan_at: "2026-05-23T12:00:00Z"`.
- [ ] Empty path test: `cargo run --quiet -- migrate-state --state /tmp/does-not-exist-$(date +%s).json` → exit 0, `from: "missing"`, file created.
- [ ] Garbage test: `echo "lol" > /tmp/bad-state && cargo run --quiet -- migrate-state --state /tmp/bad-state` → exit 1, stderr contains "format" or "ISO".
- [ ] Dry-run preserves file: `cp tests/fixtures/state-legacy.txt /tmp/dry-test && md5 /tmp/dry-test > /tmp/before && cargo run --quiet -- migrate-state --state /tmp/dry-test --dry-run && md5 /tmp/dry-test > /tmp/after && diff /tmp/before /tmp/after` → exit 0 for diff (identical).

### Regression
- [ ] `cargo run --quiet -- parse-report < tests/fixtures/agent-report-1.md` — P004 still works, output unchanged.
- [ ] `cargo run --quiet -- dedup --state tests/fixtures/state-3ids.json --rows-json tests/fixtures/rows-5.json` — P005 still works.
- [ ] `cargo run --quiet -- append --help` — P006 surface unchanged.
- [ ] State JSON v1 read path (P005) still works against the new file written by migrate-state: chain `migrate-state` then `dedup` against same path → no schema mismatch error.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P007 entry at top with migration note.
- [ ] `docs/ARCHITECTURE.md` §2 — "State write path (post-P007)" subsection added.
- [ ] `docs/ARCHITECTURE.md` §5 — P007 scaffold-status entry appended.
- [ ] `docs/security/INVARIANTS.md` — INV-LOCAL-002 P007 second-user note appended.
- [ ] `README.md` — `migrate-state` quick-start section added.
- [ ] `docs-gate --all --verbose` — 4/4 pass.

### Discovery Report
- [ ] `docs/discoveries/P007.md` — full report written (per RULES.md §13 format).
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended at top.
- [ ] Sub-mechanism A-F Verification Trace filled (table above).
- [ ] Lane (Guarded) declared in PR body section per RULES.md §9.
- [ ] Anchor #10 (chrono `parse_from_rfc3339`) result documented — verified via cargo doc OR fallback used.
- [ ] Test count post-P007 recorded in Discovery (target ≥ 49).
