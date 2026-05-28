# PHIẾU P008: state-backfill subcmd

> **ID format:** `P008` — counter `.phieu-counter` = 8 sau P007 ship.
> **Filename:** `docs/ticket/P008-state-backfill.md`
> **Branch:** `feat/P008-state-backfill`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P013 install in tarot — backfill là recovery path khi user upgrade từ legacy Bash heredoc + đã có inbox markdown nhưng state file mất/legacy; cũng cần cho P009 scan-and-append composite nếu future composite reuse parse-rows helper)
> **Ảnh hưởng:** `src/inbox.rs` (ADD `pub fn parse_rows` + extend `InboxError` enum thêm variant `ParseRow` — first parse-row consumer; ≥4 unit tests), `src/cli/state_backfill.rs` (stub → real impl), `src/main.rs` (update `Commands::StateBackfill { state, inbox, dry_run }` dispatch arm — error→exit code map), `tests/fixtures/inbox-5rows-3processed.md` (NEW — 5-row inbox với 3 processed/dismissed), `tests/fixtures/state-1id.json` (NEW — JSON v1 với 1 seen ID), `tests/state_backfill_cli.rs` (NEW integration test ≥4 tests), `docs/ARCHITECTURE.md` §5 (P008 scaffold-status entry — `inbox::parse_rows` shipped, `state-backfill` wired), `docs/CHANGELOG.md` (entry P008), `README.md` (`state-backfill` quick-start nếu chưa cover — Worker check Anchor #14)
> **Dependency:** P001 (CLI scaffold + `Commands::StateBackfill` variant + `cli/state_backfill.rs` stub), P002 (`AdvisoryRow` 8 fields + `Status` enum với `Processed`/`Dismissed` variants), P004 (`row::parse_row` + `RowParseError` + `FromStr` for `Status`/`Severity`), P005 (`state::read` + `StateReadError` reference shape), P006 (`inbox::read_inbox` + `InboxError` enum — P008 extends), P007 (`state::write_atomic` + `StateWriteError` — backfill calls write path) — all shipped 2026-05-28
> **Lane:** **Guarded** (filesystem persistence — `state::write_atomic` per INV-LOCAL-002; `--dry-run` MUST NOT touch file; data-preservation contract per Sub-mech C — `seen_advisories[]` must GROW or stay same, NEVER shrink)
> **Sub-mech áp dụng:** **B** (capability — `cargo check` + `cargo test inbox` + `cargo test --test state_backfill_cli`), **C** (migration completeness — pre-existing seen_advisories IDs MUST survive backfill; post-union count ≥ pre-union count; `backfilled_count` = post − pre matches arithmetic), **D** (persistence — ARCHITECTURE §5 + README + CHANGELOG updated), **F** (runtime state — error wording does NOT echo file content; `grep -E 'ghp_|...'` clean across new code)

---

## Context

### Vấn đề hiện tại

P007 ship `migrate-state` — bridges legacy single-line ISO state file → JSON v1. Nhưng có một edge case migrate-state KHÔNG cover: user đã có **inbox markdown với rows status `processed`/`dismissed`** (Sếp đã review qua) nhưng state file mất/garbage/missing → tarot precedent P286 "recovery path".

Scenario thực tế:
1. User chạy advisory-cron lần đầu (state file empty/fresh).
2. Agent scan → emit rows → `append` ghi vào inbox.md.
3. Sếp review, gạt một số row sang `processed` / `dismissed` trong inbox.md.
4. State file accidentally deleted/corrupted (hoặc user upgrade từ tarot bash heredoc không carry state).
5. Next scan → agent emit SAME advisories → `dedup` không thấy IDs trong empty state → re-append → **duplicate rows in inbox**.

`state-backfill` fix: walk inbox rows status `processed`/`dismissed` → extract advisory IDs → union với existing `seen_advisories[]` → atomic write state. Sau backfill, `dedup` sẽ skip các advisory Sếp đã quyết.

Pipeline (ARCHITECTURE §1 dòng 66-75):
```
advisory-inbox state-backfill --state <FILE> --inbox <FILE> [--dry-run]
→ behavior: extract IDs từ inbox rows status processed/dismissed → union vào state.seen_advisories[]
→ output:   { "backfilled_count": N, "total_seen_after": M }
→ exit:     0 success, 1 inbox unparseable, 2 write error
```

**Semantic decisions (locked by Architect):**

1. **Only `processed` + `dismissed` contribute** — `open` rows are still pending Sếp review; backfilling them would prematurely mark them "seen" before Sếp decided. Per phiếu Sếp brief.

2. **Union, never shrink (Sub-mech C invariant):** `seen_advisories[]` post-backfill = `pre_seen ∪ extracted_ids`. If extracted IDs are subset of pre_seen → `backfilled_count = 0`, file MAY still be re-written (idempotent normalization) OR skipped (Architect picks: re-write to canonicalize sort order via BTreeSet round-trip — see "Why always write" below).

3. **`last_scan_at` preserved, NOT updated.** Backfill is a RECOVERY operation, not a scan event. Setting `last_scan_at = Utc::now()` would falsely claim a fresh scan happened — could cause next cron to skip work. Locked.

4. **`agent_version` preserved.** Same reasoning — backfill doesn't represent the watch agent running.

5. **`schema_version` preserved.** Must equal 1 (the only supported version); if pre-existing state has different version, fail via `state::read` `SchemaMismatch` → exit 1.

6. **`--dry-run`**: print summary JSON to stdout, DO NOT touch file. Pattern matches P007 dry-run semantic (byte-identity pre/post).

**Why `inbox::parse_rows` is NEW surface:**

P006 shipped `inbox::insert_rows` (write path — take rows, produce new content). P008 needs the INVERSE: read content, extract rows. Architect chose `inbox::parse_rows` (sibling to `insert_rows`) instead of inline-in-cli per same reasoning as P006 `write_atomic` extraction:

- Future P011 MCP tool `state_backfill` needs same parser.
- Future composite operations (P009 scan-and-append in particular — if it ever needs to recount inbox rows) may share.
- Module boundary: inbox parser belongs in `inbox.rs`, not `cli/`.
- Centralization avoids per-callsite drift in skip-HTML-comment + skip-blank logic.

`row::parse_row` (P004 ship) handles SINGLE row parsing. `inbox::parse_rows` is the inbox-level driver: find `## Rows` section, iterate lines, skip HTML comment / blank, call `row::parse_row` per line, collect.

**Why extend `InboxError` (NOT new `BackfillError` sibling enum):**

`inbox::parse_rows` is an INBOX-level operation; its failure modes are inbox-domain:
- Io (already exists from P006).
- ParseRow (new variant — wraps `row::RowParseError` from P004).
- (Missing `## Rows` heading — Architect picks: tolerate, return empty Vec. Reason: backfill on a freshly-created inbox without rows section yet should NOT hard-fail; it should backfill nothing. Worker may self-decide alternative: hard-error → Tầng 2 self-decide ONLY if Worker discovers fixture testing reveals strong preference. Phiếu locked: tolerate-empty.)

CLI-level errors (backfill specifically) currently has zero new failure modes that aren't covered by `InboxError` / `StateReadError` / `StateWriteError`. No new `BackfillError` enum needed.

Reference BACKLOG.md P008:
- Lane: Guarded (filesystem state write).
- Scope: Extract advisory IDs from inbox rows status `processed`/`dismissed` → union with existing `seen_advisories[]`. P286 of tarot precedent.
- Acceptance: Fixture inbox 5 rows (3 processed) + state 1 ID → output state has 4 IDs.
- Sub-mech checks: B, C.

### Giải pháp

**4 unit công việc chính:**

1. **`src/inbox.rs` — ADD `pub fn parse_rows` + extend `InboxError` enum:**

   - Extend `InboxError` enum: ADD `ParseRow { path: PathBuf, line_number: usize, source: row::RowParseError }` variant. Maps to exit code 1 ("inbox unparseable" per ARCHITECTURE §1).
   - Signature: `pub fn parse_rows(content: &str) -> Result<Vec<AdvisoryRow>, InboxError>`.
     - Note: takes `&str` content (caller has already done `read_inbox`). path-context for `ParseRow` error is the CALLER's responsibility (they have `path`); for unit-test ergonomics, parse_rows itself doesn't know path → use `PathBuf::new()` placeholder, caller maps via downcast OR Architect chooses signature with optional `&Path`. Locked decision: `parse_rows(content: &str) -> Result<Vec<AdvisoryRow>, ParseRowsError>` where ParseRowsError is a SMALLER intermediate error (just `LineFailed { line_number, source }`), caller wraps into `InboxError::ParseRow` adding path. **Worker self-decides: either pattern OK.** Recommended: define `parse_rows` to return `Result<Vec<AdvisoryRow>, InboxError>` directly, accept `PathBuf::new()` placeholder for ParseRow, caller re-wraps with real path before bubbling. This matches P006 `insert_rows` precedent (PathBuf::new placeholder, caller fills).
   - Logic:
     1. Split content into lines: `let lines: Vec<&str> = content.lines().collect()`.
     2. Find `## Rows` heading: `lines.iter().position(|l| l.trim_end() == "## Rows")`. If None → return `Ok(Vec::new())` (tolerate-empty per locked decision above).
     3. Iterate `lines[heading_idx + 1..]`. Track HTML-comment block state via simple boolean flag (`in_comment`) toggled by `<!--` start and `-->` end on each line.
        - Edge: comment block spanning multiple lines — open marker on line A, close on line B; lines A+1..B-1 also inside.
        - Edge: comment start AND end on same line (`<!-- foo -->`) → toggle on, toggle off, line content NOT a row.
        - Heuristic OK (matches P006 `total_open` counter logic): substring detect `<!--` and `-->` per line. Worker reuses P006's helper if extracted, else inline same logic.
     4. For each line OUTSIDE comment block:
        - Trim. Skip if empty.
        - Skip if starts with `|---` or contains only `---|` (header separator row). Worker self-decides robust detection: line `starts_with("|---")` OR matches all-dashes pattern.
        - Skip if `starts_with("| Date |")` (column header row from ARCHITECTURE §3 example). Worker may use full match `line.trim() == "| Date | Advisory ID | ... |"` OR robust startswith.
        - Otherwise: treat as pipe-row, call `row::parse_row(line)`. On Err → return `InboxError::ParseRow { path: PathBuf::new(), line_number: <actual idx+1>, source: row_err }`.
        - On Ok → push to Vec.
     5. Stop iteration when encountering next `## ` heading (start of new section) OR `# ` heading. Worker scan via `line.trim_start().starts_with("## ")` and `idx > heading_idx` — terminate loop.
     6. Return Vec.
   - Unit tests (≥4):
     - **Test A — Happy 3 rows:** Inbox content with `## Rows` heading + 3 valid pipe-rows → parse_rows returns Vec len 3, each row's `advisory_id` matches expected.
     - **Test B — Empty Rows section:** Content has `## Rows` heading but no rows underneath (or just blank lines/header separator) → parse_rows returns Vec::new() (empty), no error.
     - **Test C — HTML comment placeholder skipped:** Content includes `<!-- | 2026-05-23 | GHSA-skip | ... | -->` comment block per ARCHITECTURE §3 example → parse_rows does NOT include that row in output Vec.
     - **Test D — Bad row format error:** Content has `## Rows` + valid row + malformed row (e.g., only 5 columns instead of 8) → parse_rows returns `Err(InboxError::ParseRow { line_number: <N>, source: ... })`. Worker test pattern-matches via `matches!(err, InboxError::ParseRow { .. })`.
     - **(Optional Test E)** — No `## Rows` heading → Vec::new() (tolerate per locked decision).
     - **(Optional Test F)** — Stop at next `## ` heading: content has `## Rows` + 2 rows + `## Other Section` + 1 pipe-line in Other → parse_rows returns Vec len 2 (NOT 3). Worker self-decides if include this test or leave for Discovery.

2. **`src/cli/state_backfill.rs` — stub → real impl:**

   - Signature unchanged: `pub fn run(state_path: PathBuf, inbox: PathBuf, dry_run: bool) -> anyhow::Result<()>`.
     - Note: rename param `state` → `state_path` to avoid shadowing `crate::state` module (P007 precedent — same fix Worker applied in `cli/migrate_state.rs` and `cli/dedup.rs`).
   - Logic:
     1. Read state via `state::read(&state_path)` → `StateFile`. (Bubbles `StateReadError` via anyhow → main.rs downcasts → exit 1 for Io/Json/SchemaMismatch per P005 contract; backfill maps these to exit 1 per ARCHITECTURE §1 "inbox unparseable" extended semantic — actually NO: state-read failure is exit 1 per P005 dedup precedent; ARCHITECTURE §1 backfill exit codes are 0/1/2 → keep state-read errors at exit 1 = "input invalid".)
     2. Read inbox via `inbox::read_inbox(&inbox)` → markdown String. (Bubbles `InboxError::Io` → main.rs downcast → exit 1 per backfill spec "inbox unparseable".)
     3. Parse rows: `let rows = inbox::parse_rows(&content)?;` — Worker wraps any returned `InboxError::ParseRow` to inject real path:
        ```rust
        let rows = match inbox::parse_rows(&content) {
            Ok(rs) => rs,
            Err(InboxError::ParseRow { line_number, source, .. }) => {
                return Err(InboxError::ParseRow {
                    path: inbox.clone(),
                    line_number,
                    source,
                }.into());
            }
            Err(e) => return Err(e.into()),
        };
        ```
        (Pattern OK because PathBuf::new() placeholder is only valid for the moment between parse_rows return and CLI re-wrap.)
     4. Filter rows: `rows.iter().filter(|r| matches!(r.status, Status::Processed | Status::Dismissed))`.
     5. Extract advisory IDs from filtered: collect into `BTreeSet<String>` (dedup automatically).
     6. Union with pre-existing state IDs:
        ```rust
        let mut union: BTreeSet<String> = state.seen_advisories.iter().cloned().collect();
        let pre_count = union.len();
        union.extend(extracted_ids);  // BTreeSet handles dedup
        let post_count = union.len();
        let backfilled_count = post_count - pre_count;
        ```
     7. Build updated state:
        ```rust
        let updated = StateFile {
            schema_version: state.schema_version,
            last_scan_at: state.last_scan_at,        // PRESERVED — backfill is NOT a scan event
            seen_advisories: union.into_iter().collect(),  // BTreeSet → sorted Vec
            agent_version: state.agent_version,      // PRESERVED
        };
        ```
     8. If `dry_run == false`: `state::write_atomic(&state_path, &updated)?`. (Writes even if backfilled_count == 0 — idempotent normalization to canonical sorted order; matches P007 idempotent-rewrite precedent for JSON v1 input branch.)
        - Note: Worker self-decides Tầng 2 — IF backfilled_count == 0 AND state.seen_advisories was already sorted, skip write (perf micro-opt). NOT required. Phiếu locked: always write (when not dry-run).
     9. Output JSON: `println!("{}", json!({ "backfilled_count": backfilled_count, "total_seen_after": post_count }))`.

   - Skip-write gate pattern (locked from P007):
     ```rust
     if !dry_run {
         state::write_atomic(&state_path, &updated)
             .with_context(|| format!("writing backfilled state to `{}`", state_path.display()))?;
     }
     ```

   - **No new `BackfillError` enum.** All errors propagate via existing `InboxError` + `StateReadError` + `StateWriteError`. Main.rs dispatches via downcast.

3. **`src/main.rs` dispatch arm — error → exit code map:**

   - Update `Commands::StateBackfill { state, inbox, dry_run }` dispatch (currently flat passthrough from P001 scaffold):
     ```rust
     Commands::StateBackfill { state, inbox, dry_run } => {
         if let Err(e) = cli::state_backfill::run(state, inbox, dry_run) {
             let code = if e.downcast_ref::<crate::inbox::InboxError>().is_some() {
                 1
             } else if e.downcast_ref::<crate::state::StateReadError>().is_some() {
                 1
             } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
                 2
             } else {
                 // Fallback: unexpected error category → exit 2.
                 2
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```

   - Per ARCHITECTURE §1 dòng 75 backfill exit codes: 0 success / 1 inbox unparseable / 2 write error. Mapping:
     - `InboxError::Io` (inbox read fail) → exit 1.
     - `InboxError::ParseRow` (row malformed) → exit 1.
     - `StateReadError::*` (state file unreadable / wrong schema) → exit 1. *(Architect re-reads §1 backfill: "1 inbox unparseable" — strictly says inbox; state failures aren't called out. Architect picks: extend "inbox unparseable" to "input file invalid" (state OR inbox). Rationale: user-facing semantic is "your inputs are wrong", exit 1 is the natural code. Worker may surface this to Sếp via Discovery if disagreement.)*
     - `StateWriteError::Io` → exit 2.

   - Tail `Ok(())` REQUIRED (P004/P005/P006/P007 precedent).

4. **Fixtures + integration test:**

   - `tests/fixtures/inbox-5rows-3processed.md`:
     ```markdown
     # Advisory Inbox

     > Test fixture for P008 state-backfill.

     ## Rows

     | Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
     |------|-------------|-----------|---------|-----------|----------|--------|------|
     | 2026-05-28 | CVE-2026-9001 | https://example.com/9001 | next@<15.5.17 | src/middleware.ts:42 | High | processed | reviewed |
     | 2026-05-28 | CVE-2026-9002 | https://example.com/9002 | flask@<2.3.5 | app.py:8 | Medium | dismissed | not applicable |
     | 2026-05-28 | CVE-2026-9003 | https://example.com/9003 | tokio@<1.40 | src/main.rs:1 | Critical | processed | patched |
     | 2026-05-28 | CVE-2026-9004 | https://example.com/9004 | serde@<1.0.200 | src/lib.rs:5 | Low | open | pending review |
     | 2026-05-28 | CVE-2026-9005 | https://example.com/9005 | clap@<4.5 | src/main.rs:10 | Medium | open | pending review |
     ```
     - 3 rows with status `processed`/`dismissed`: CVE-2026-9001, CVE-2026-9002, CVE-2026-9003.
     - 2 rows with status `open`: CVE-2026-9004, CVE-2026-9005 (MUST NOT contribute to backfill).

   - `tests/fixtures/state-1id.json`:
     ```json
     {
       "schema_version": 1,
       "last_scan_at": "2026-05-23T12:00:00Z",
       "seen_advisories": [
         "CVE-2026-7777"
       ],
       "agent_version": "advisory-watch@0.1.0"
     }
     ```
     - 1 pre-existing seen ID `CVE-2026-7777` (does NOT overlap with any inbox row — verifies pure union grows by exactly 3).

   - `tests/state_backfill_cli.rs` — ≥4 integration tests:
     - **Test A — Per acceptance (5 rows / 3 processed + state 1 ID → 4 IDs):**
       Copy both fixtures to tempdir → run `state-backfill --state <tmp-state> --inbox <tmp-inbox>` → exit 0, stdout JSON `backfilled_count: 3`, `total_seen_after: 4`. Read back state file → `seen_advisories` contains exactly: `["CVE-2026-7777", "CVE-2026-9001", "CVE-2026-9002", "CVE-2026-9003"]` (sorted via BTreeSet). `last_scan_at` UNCHANGED from `2026-05-23T12:00:00Z`. `agent_version` UNCHANGED.
     - **Test B — Already-backfilled (no new IDs, count = 0):**
       Pre-populate state with all 3 processed/dismissed IDs already present (+ the 1 original): `seen_advisories = ["CVE-2026-7777", "CVE-2026-9001", "CVE-2026-9002", "CVE-2026-9003"]`. Run state-backfill → exit 0, stdout `backfilled_count: 0`, `total_seen_after: 4`. State file (re-written for idempotent normalization, OK) STILL has same 4 IDs.
     - **Test C — `--dry-run` byte-identity:**
       Copy fixtures to tempdir, snapshot bytes of state file → run state-backfill `--dry-run` → exit 0, stdout `backfilled_count: 3`, `total_seen_after: 4`. Read state file bytes → IDENTICAL to snapshot (write skipped). Critical for Sub-mech F (dry-run no-touch contract; matches P007 Test E precedent).
     - **Test D — Only processed+dismissed contribute (open ignored):**
       Use same 5-row fixture. Verify the 2 `open` rows (CVE-2026-9004, CVE-2026-9005) are NOT added to seen_advisories. Inspect post-state.seen_advisories — assert `!contains("CVE-2026-9004")` and `!contains("CVE-2026-9005")`. This may be folded into Test A's assertion list (Worker self-decides) OR kept as standalone.
     - **(Optional Test E — Inbox unparseable → exit 1)**
       Craft inbox fixture with `## Rows` heading + a malformed pipe-row (e.g., 5 cols) → exit 1, stderr contains "parse" or "row" or "column". State file UNCHANGED. Tầng 2 self-decide; nice-to-have.

#### Sub-mech C migration completeness — semantic for P008

For state-backfill, "count preserved" = post.seen_advisories ⊇ pre.seen_advisories AND `last_scan_at == pre.last_scan_at`. Apply checks in Test A:
```rust
let post_state: StateFile = serde_json::from_str(&post_content).unwrap();
let pre_state: StateFile = serde_json::from_str(&pre_content).unwrap();
// Sub-mech C: pre IDs MUST all be present in post
for id in &pre_state.seen_advisories {
    assert!(post_state.seen_advisories.contains(id), "Sub-mech C: ID {} lost in backfill", id);
}
// Sub-mech C: last_scan_at preserved
assert_eq!(post_state.last_scan_at, pre_state.last_scan_at);
// Sub-mech C: post count >= pre count
assert!(post_state.seen_advisories.len() >= pre_state.seen_advisories.len());
```

#### Why always re-write (even when backfilled_count == 0)?

Mirrors P007 idempotent rewrite for JSON v1 input branch: re-writing canonicalizes:
- `serde_json::to_string_pretty` 2-space indent.
- BTreeSet → sorted Vec output (if pre-state had unsorted seen_advisories due to manual edit, backfill normalizes).
- Trailing newline per P007 convention.

Alternative considered: skip write when `backfilled_count == 0` — saves one disk write. Rejected because: (1) divergent code paths complicate testing; (2) atomic write is fast; (3) canonicalization is a feature. Worker may Tầng 2 self-decide skip-write-on-zero as optimization — OK, log Discovery if chosen.

#### `inbox::parse_rows` — line stopping condition

Architect picks: stop when next `## ` heading encountered (start of next section). Reason: inbox markdown may have sections like `## Archive` or `## Dismissed (archived)` AFTER `## Rows` in future schemas. Stopping at next heading avoids parsing those as live rows.

Alternative considered: parse to EOF. Rejected because false-positive risk if user appends free-form text after rows.

Worker self-decides Tầng 2: stop at `# ` heading too (e.g., `# Section`) — likely fine, ARCHITECTURE §3 shows only `## Rows` as the relevant heading.

#### Header / separator row detection

The fixture column-header row `| Date | Advisory ID | ... |` and separator `|------|...|` must NOT be parsed as `AdvisoryRow`. `row::parse_row` from P004 will fail on these (column count mismatch and/or non-parseable severity/status). Strategies:
- A — pre-filter heuristic (skip lines starting with `|---` or matching `| Date | Advisory ID |`).
- B — let parse_row fail, count as "skip on error" (LOSES test signal — bad row vs header indistinguishable).
- C — robust header detection: skip lines where first non-pipe-trimmed cell == "Date" or `:---` etc.

**Locked decision: A (pre-filter heuristic).** Worker implements simple skips:
```rust
if line.starts_with("|---") || line.starts_with("| ---") { continue; }
let trimmed = line.trim();
if trimmed.starts_with("| Date |") || trimmed.starts_with("|Date|") { continue; }
```
Test C explicitly covers header+separator skipping (the fixture inbox-5rows-3processed.md has both rows present).

Tầng 2 self-decide: Worker may use a single regex-based detection if cleaner; not required.

#### `--dry-run` output shape consistency

Same as P007: dry-run emits SAME summary JSON as non-dry-run. User who wants to inspect the full intended StateFile JSON can re-read state file post-non-dry-run. Locked decision.

### Scope

- CHỈ sửa: `src/inbox.rs` (ADD `pub fn parse_rows` + extend `InboxError` enum thêm `ParseRow` variant + ≥4 unit tests), `src/cli/state_backfill.rs` (stub → real impl), `src/main.rs` (update `Commands::StateBackfill` dispatch arm only).
- CHỈ tạo fixtures: `tests/fixtures/inbox-5rows-3processed.md`, `tests/fixtures/state-1id.json`.
- CHỈ tạo integration: `tests/state_backfill_cli.rs`.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (P008 scaffold-status entry — `inbox::parse_rows` shipped, `state-backfill` wired); `docs/CHANGELOG.md` (P008 entry); `README.md` (`state-backfill` quick-start nếu chưa cover — Anchor #14 conditional).
- KHÔNG sửa: `src/state.rs` (P002/P005/P007 lock — `state::write_atomic` ready, không cần thay đổi), `src/sentinel.rs` (P003 lock), `src/row.rs` (P004/P006 lock — `parse_row` + `Display` + `FromStr` đã có), `src/cli/parse_report.rs` (P004 lock), `src/cli/dedup.rs` (P005 lock), `src/cli/append.rs` (P006 lock), `src/cli/migrate_state.rs` (P007 lock), `Cargo.toml` (NO new dep — `serde_json`, `chrono`, `anyhow`, `thiserror`, `serde`, `tempfile`, `regex` đều đã có per P002-P007).
- KHÔNG sửa `inbox::read_inbox`, `inbox::insert_rows`, `inbox::write_atomic` (P006 lock — chỉ ADD `parse_rows` cùng module).
- KHÔNG đổi `InboxError` existing variants `MissingRowsHeading` + `Io` (P006 lock — chỉ ADD `ParseRow`). Worker MUST preserve serde/Display compat for existing variants.
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 state-backfill: 0/1/2).
- KHÔNG đổi state schema (`schema_version`, fields). P008 ONLY reads state + writes back same shape.
- KHÔNG bump `SCHEMA_VERSION` constant.
- KHÔNG đổi `StateFile` shape (P002 lock). KHÔNG đổi `StateReadError` (P005 lock) / `StateWriteError` (P007 lock).
- KHÔNG add `--force`, `--backup`, `--include-open` flags. Out-of-scope.
- KHÔNG implement `last_scan_at` bump on backfill — explicit non-feature per locked decision #3 in Context.
- KHÔNG update `last_scan_at` to `Utc::now()` (Anti-pattern — backfill is recovery, NOT a scan event).
- KHÔNG tạo `src/error.rs` (ARCHITECTURE §5 pending — not P008 scope).
- KHÔNG modify `cli/mod.rs` (P001 already registered `pub mod state_backfill;` — Worker verify Anchor #15).
- KHÔNG move/share `RowsEnvelope` (P005 → P009 deferred).
- KHÔNG xoá `#![allow(dead_code)]` từ `sentinel.rs` (P004 follow-up; cross-phiếu housekeeping; NOT P008 scope).
- KHÔNG add concurrency lock (ARCHITECTURE §10 deferred).
- KHÔNG implement reverse-parse `FromStr for AdvisoryRow` (P006 explicitly deferred; P008 uses `row::parse_row` which P004 shipped).

### Skills consulted

Architect Read `docs/ticket/P007-migrate-state.md` để tham khảo:
- `state::write_atomic` ready (P007 shipped as second concrete user of INV-LOCAL-002).
- `state::read` + `StateReadError` ready (P005).
- Skip-write gate pattern `if !dry_run { state::write_atomic(...)?; }` from P007 — direct reuse.
- anyhow downcast → exit code map idiom (main.rs dispatch arm — same pattern as P007).
- Integration test idiom (assert_cmd + tempdir + byte-snapshot for dry-run verification).
- Rename param `state` → `state_path` to avoid shadowing `crate::state` module.
- `predicates::prelude::PredicateBooleanExt` import needed if test uses `.or()` (P007 SD-1 Discovery).
- `std::io::Error::other()` preferred over `::new(ErrorKind::Other, ...)` (P007 SD-2 Discovery / clippy `io_other_error`).

Architect Read `docs/ticket/P006-append-atomic.md` để tham khảo:
- `inbox::read_inbox` + `InboxError::Io` ready (P006).
- `InboxError` enum 2-variant shape (P006) — P008 ADDS 3rd variant `ParseRow`.
- HTML comment skip heuristic — substring detect `<!--` / `-->` per line. P008 reuses identical logic in `parse_rows`.
- Pattern for `inbox.rs` module unit tests (≥4 tests, write_atomic + insert_rows + read_inbox).

Architect Read `docs/ticket/P004-parse-report.md` để tham khảo:
- `row::parse_row(line: &str) -> Result<AdvisoryRow, RowParseError>` ready (P004 ship).
- `RowParseError` shape (used by `row::parse_row` — propagated through `InboxError::ParseRow.source`).
- `Status::Processed` + `Status::Dismissed` enum variants exist (P002 + P004 confirmed via Anchor #4 transitive).

Architect Read `docs/ticket/P005-dedup.md` để tham khảo:
- `StateReadError` Io/Json/SchemaMismatch (P005 ship).
- anyhow downcast → exit code idiom in main.rs dispatch (P005 first established).

Architect Read `docs/discoveries/P007.md` để học:
- Test count baseline post-P007: 49 tests (33 unit + 16 integration). P008 target: ≥49 + ≥4 new unit (parse_rows) + ≥4 new integration = 57+ tests.
- `predicates::prelude::PredicateBooleanExt` import required for `.or()`.
- `std::io::Error::other()` preferred (clippy lint `io_other_error`).
- Dry-run byte-identity verification pattern via `std::fs::read` snapshot.

Architect Read `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 (covered via P006/P007 phiếu). P008 IS third concrete user via `state::write_atomic` call BUT does NOT add new write path → INVARIANTS doc note OPTIONAL (Worker self-decide; recommendation: append "P008 third caller via state::write_atomic" if INVARIANTS lists callers, otherwise skip — Tầng 2).

Architect did NOT use context7 for any library (per CLAUDE.md §4 preflight: architect agent envelope = Read/Write/Glob only; context7 NOT in envelope). All chrono/serde/tempfile APIs already exercised in P002-P007 — no new surface to verify.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> **BẮT BUỘC:** Kiến trúc sư PHẢI grep/verify code thật trước khi viết assumption.
> Thợ đọc bảng này để biết assumption nào đã verify, assumption nào chưa.
> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/state_backfill.rs` hiện là stub printf TODO (P001 ship). Signature `pub fn run(state: PathBuf, inbox: PathBuf, dry_run: bool) -> Result<()>` per P001 scaffold pattern (analogous to P004/P005/P006/P007 stub shape). | P007 Anchor #1 (confirmed migrate_state stub identical shape); transitive — state_backfill stub should match. | `[needs Worker verify]` | ⏳ TO VERIFY (Worker Task 0: `cat src/cli/state_backfill.rs`). |
| 2 | `src/main.rs` có `Commands::StateBackfill { state: PathBuf, inbox: PathBuf, dry_run: bool }` clap variant + dispatch arm `cli::state_backfill::run(state, inbox, dry_run)` từ P001 ship. | P007 Anchor #2 confirmed all 8 dispatch arms present at main.rs lines ~25-160; transitively P008's variant should be there. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "StateBackfill" src/main.rs`). |
| 3 | `Commands::StateBackfill` clap variant declares `--state <FILE>` REQUIRED + `--inbox <FILE>` REQUIRED + `--dry-run` flag (boolean, no value). | ARCHITECTURE §1 dòng 66-75 + P001 scaffold ship per CLI surface spec. | `[needs Worker verify]` | ⏳ TO VERIFY (`cargo run -- state-backfill --help`). |
| 4 | `src/row.rs` exports `pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError>` (P004 ship). `RowParseError` enum carries variants for column count + severity/status parse fail. | P004 ship + P007 Anchor reference (P004 wired parse_report); P006 Anchor #4 transitive. | `[verified]` | ✅ Pre-verified (P004 shipped; P006/P007 confirmed unchanged). |
| 5 | `src/row.rs` exports `pub enum Status { Open, Processed, Dismissed }` với serde `rename_all = "lowercase"` + `FromStr` impl (P004 ship). Variants exact names: `Open`, `Processed`, `Dismissed`. | P002 ship + P004 Discovery Anchor (status enum confirmed exact variant names + serde lowercase). | `[verified]` | ✅ Pre-verified (P004 shipped FromStr; P006/P007 unchanged). |
| 6 | `src/inbox.rs` exports `pub fn read_inbox(path: &Path) -> Result<String, InboxError>` (P006 ship). `InboxError` enum currently has 2 variants: `MissingRowsHeading { path: PathBuf }` and `Io { path: PathBuf, source: io::Error }`. | P006 ship + P007 Anchor reference. | `[verified]` | ✅ Pre-verified (P006 shipped, P007 didn't touch). |
| 7 | `src/inbox.rs` chưa có `pub fn parse_rows` (P006 only shipped `read_inbox`, `insert_rows`, `write_atomic`). | P006 scope explicit + ARCHITECTURE §5 P006 entry "inbox.rs (`read_inbox` + `insert_rows` + `write_atomic` + `InboxError`)" — no parse_rows mentioned. | `[unverified]` | ⏳ TO VERIFY (`grep -n "fn parse_rows" src/inbox.rs` → expect 0 hits). |
| 8 | `src/state.rs` exports `pub fn write_atomic(path: &Path, state: &StateFile) -> Result<(), StateWriteError>` (P007 ship). Implementation per INV-LOCAL-002. | P007 ship + Discovery Anchor confirmed second concrete user of INV-LOCAL-002. | `[verified]` | ✅ Pre-verified (P007 shipped). |
| 9 | `src/state.rs` exports `pub fn read(path: &Path) -> Result<StateFile, StateReadError>` (P005 ship). `StateReadError` enum has variants `Io`, `Json`, `SchemaMismatch`. | P005 ship + P007 Anchor #5. | `[verified]` | ✅ Pre-verified. |
| 10 | `src/state.rs` exports `pub struct StateFile` 4 fields + `pub const SCHEMA_VERSION: u32 = 1`. | P002 ship + P007 Anchor #4 transitive. | `[verified]` | ✅ Pre-verified. |
| 11 | `Cargo.toml` `[dependencies]` có `serde_json = "1"`, `chrono = "0.4"`, `anyhow = "1"`, `thiserror = "2"`, `serde = "1"`, `tempfile = "3"`, `regex` (sentinel parser dep). KHÔNG cần add dep mới. | P007 Anchor #8 confirmed all 6 core deps + regex (P003 ship); Cargo.toml unchanged post-P007. | `[verified]` | ✅ Pre-verified (Cargo.toml unchanged through P002-P007 chain except P002 added deps). |
| 12 | `Cargo.toml` `[dev-dependencies]` có `assert_cmd = "2"` + `predicates = "3"`. | P007 Anchor #9. | `[verified]` | ✅ Pre-verified. |
| 13 | `tests/fixtures/` directory tồn tại sau P007 ship; chứa: `inbox-baseline.md`, `rows-2.json`, `state-3ids.json`, `rows-5.json`, `agent-report-1.md`, `state-legacy.txt`, `state-json-v1.json`, `state-garbage.txt`. | P006 + P007 Discovery files-changed tables. | `[verified]` | ✅ Pre-verified. |
| 14 | `README.md` chưa có `state-backfill` quick-start section (P004→P007 covered parse-report/dedup/append/migrate-state respectively; state-backfill untouched). | P007 Discovery confirmed migrate-state section added; prior phiếu did NOT touch state-backfill. | `[unverified]` | ⏳ TO VERIFY (`grep -n "state-backfill" README.md` — expect at most 1 hit in stub list / exit-code table, NOT in quick-start). |
| 15 | `src/cli/mod.rs` đã có `pub mod state_backfill;` (P001 scaffold ship 8 subcmd modules). | P007 Anchor #21 confirmed `pub mod migrate_state;` at cli/mod.rs:10 — transitively all 8 modules registered per P001. | `[unverified]` | ⏳ TO VERIFY (`grep -n "pub mod state_backfill" src/cli/mod.rs`). |
| 16 | `docs/ARCHITECTURE.md` §1 state-backfill subcmd block (dòng 66-75) documents I/O contract correctly: input `--state <FILE>` + `--inbox <FILE>` + `--dry-run` flag; output `{ backfilled_count, total_seen_after }`; exit 0/1/2. | Architect Read ARCHITECTURE.md dòng 66-75 during load context. | `[verified]` | ✅ Dòng 66-75 exact match per phiếu Context spec. |
| 17 | `docs/ARCHITECTURE.md` §3 (dòng 162-187) inbox markdown format: 8 columns, pipe-delimited, Status enum `open/processed/dismissed`, Severity `Critical/High/Medium/Low`, HTML-comment placeholders skipped. | Architect Read ARCHITECTURE.md §3 during load context. | `[verified]` | ✅ Confirmed §3 spec. |
| 18 | `docs/ARCHITECTURE.md` §5 lists P007 scaffold-status entry; P008 entry pending — Worker adds. | Architect Read ARCHITECTURE.md §5 P007 entry (dòng 252). | `[verified]` | ✅ §5 P007 entry shipped; P008 line not yet present. |
| 19 | `src/main.rs` `Commands::StateBackfill` dispatch arm currently flat passthrough `cli::state_backfill::run(state, inbox, dry_run)` (P001 ship; no error mapping yet). | P007 Discovery — only `Commands::Append` + `Commands::Dedup` + `Commands::MigrateState` arms updated to error-map; other arms remain stub passthrough. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n -A 5 "StateBackfill" src/main.rs`). |
| 20 | `inbox.rs` HTML-comment skip logic shipped in P006 (used by `insert_rows` total_open counter). Worker can reuse identical heuristic for `parse_rows`. | P006 Discovery references HTML comment block detection via substring `<!--`/`-->` per line. | `[verified]` | ✅ Pre-verified (P006 shipped). |
| 21 | `row::parse_row` accepts pipe-delimited line including outer `|` pipes per ARCHITECTURE §3 format (`| Date | ... |`); fails on column count != 8 OR severity/status string not matching enum. | P004 ship; P004 Discovery confirmed via integration tests parsing fixture rows. | `[verified]` | ✅ Pre-verified. |
| 22 | `BTreeSet<String>` from `std::collections::BTreeSet` available without new dep (std lib). `Vec::extend(BTreeSet)` + `BTreeSet::into_iter().collect::<Vec<_>>()` standard operations. | std lib — no verification needed. | `[verified]` | ✅ Std lib. |

**Note:** Worker EXECUTE Task 0 runs ~6 grep/check verifications (Anchors #1/#2/#3/#7/#14/#15/#19 → 7 anchors `[needs Worker verify]` + `[unverified]`). If ANY anchor diverges from spec → STOP, escalate, write Discovery Report.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. Anchor #7 (`parse_rows` not yet in inbox.rs) flagged `[unverified]` — Worker grep confirms before adding.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Anchor verification:**
- Anchor #1 (`state_backfill.rs` stub shape): ✅ Stub confirmed — `pub fn run(state: PathBuf, inbox: PathBuf, dry_run: bool) -> Result<()>` at `src/cli/state_backfill.rs:6`. Note: params are named `state`/`inbox` (not yet renamed to `state_path`/`inbox_path`) — phiếu Task 2 correctly instructs rename.
- Anchor #2 (`Commands::StateBackfill` variant): ✅ `src/main.rs:59` — `StateBackfill { state: PathBuf, inbox: PathBuf, dry_run: bool }` with required `--state`, `--inbox`, boolean `--dry-run` flag.
- Anchor #3 (clap `--help` output): ✅ `--state <STATE>` REQUIRED, `--inbox <INBOX>` REQUIRED, `--dry-run` boolean flag — verified via `cargo run -- state-backfill --help`.
- Anchor #7 (`parse_rows` absent): ✅ `grep -n "fn parse_rows" src/inbox.rs` → 0 hits. Safe to add.
- Anchor #14 (README `state-backfill` absent): ✅ `grep -n "state-backfill" README.md` → 0 hits. Worker must add quick-start section.
- Anchor #15 (`pub mod state_backfill`): ✅ `src/cli/mod.rs:14` — confirmed present.
- Anchor #19 (`StateBackfill` dispatch arm — flat passthrough): ✅ `src/main.rs:160-164` — currently `=> cli::state_backfill::run(state, inbox, dry_run)` with no error mapping. Matches phiếu assumption.
- Baseline test count: ✅ 49 tests (cargo test --all -- --list confirms exact match to P007 Discovery baseline).

**Objections (Tầng 1 only):**

**[O1.1] [mechanical] — Constraint #10 cites wrong file for exhaustive match**

Phiếu Constraint #10 says: "Worker MUST update `cli/append.rs` match arm" when adding `ParseRow` variant. Reality: `src/cli/append.rs` does NOT match on `InboxError` variants at all — it propagates errors via `?` operator only (`src/cli/append.rs` contains no `match ie { ... }` block). The exhaustive match that WILL break at compile-time is in `src/main.rs:127-135`:
```rust
match ie {
    crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
    crate::inbox::InboxError::Io { .. } => 2,
}
```
This is the `Commands::Append` dispatch arm in `main.rs`. Adding `ParseRow` variant to `InboxError` enum will produce a compile error here — NOT in `cli/append.rs`.

Resolution (self-decide Tầng 2 — mechanical, no architectural impact): Worker updates `main.rs` Append arm to add `crate::inbox::InboxError::ParseRow { .. } => 1` (consistent with MissingRowsHeading semantic). This is already implicitly in-scope of Task 3 (`main.rs` touch). No change to `cli/append.rs` needed.

**No Tầng 1 blocking objections.** The mechanical discrepancy is self-resolvable within Task 3 scope.

**Worker accepted V1 — proceed to EXECUTE.**

Anchor verification summary: ✅ #1 ✅ #2 ✅ #3 ✅ #7 ✅ #14 ✅ #15 ✅ #19. No blocking issues.

**Status:** ✅ WORKER ACCEPTS V1 — Ready for execution (autonomous mode per pilot config)

### Turn 1 — Architect Response
*(Architect fill phần này khi invoked RESPOND mode. KHÔNG đọc source code — dựa vào Worker `file:line` citation.)*

- [O1.1] → ACCEPT / DEFEND / REFRAME (Tầng 2) / DEFER TO SẾP → action taken
- [O1.2] → …

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V1
- Total turns: 1 (Worker accepted — no Architect response needed)
- Approved (autonomous mode): 2026-05-28 — code execution may begin

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
| A (trigger) | (no hook/cron in this phiếu) | N/A | N/A | N/A |
| B (capability) | `cargo check` | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| B (capability) | `cargo test inbox` | ≥X+4 tests pass (existing P006 inbox tests + ≥4 new parse_rows) | 10 inbox tests pass (4 original + 6 new) | ✅ |
| B (capability) | `cargo test --test state_backfill_cli` | ≥4 integration tests pass (acceptance / already-backfilled / dry-run / open-ignored) | 4 tests pass | ✅ |
| B (capability) | `cargo build --release` | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| B (capability) | `cargo run --quiet -- state-backfill --state <tmp-state-1id> --inbox <tmp-inbox-5rows>` | exit 0, stdout JSON `backfilled_count: 3`, `total_seen_after: 4` | verified via Test A | ✅ |
| B (capability) | `cargo run --quiet -- state-backfill --state <state> --inbox <inbox> --dry-run` | exit 0, stdout same JSON; file bytes UNCHANGED | verified via Test C byte-identity | ✅ |
| C (migration completeness) | Test A — pre IDs all present in post | `CVE-2026-7777` survives backfill | asserted in Test A | ✅ |
| C (migration completeness) | Test A — `last_scan_at` preserved | `2026-05-23T12:00:00Z` unchanged | asserted in Test A | ✅ |
| C (migration completeness) | Test A — `backfilled_count == 3` matches arithmetic post-pre | exact match | `stdout contains "backfilled_count":3` | ✅ |
| C (migration completeness) | Test B — already-backfilled idempotent | `backfilled_count == 0`, file content stable | `stdout contains "backfilled_count":0` + 4 IDs in post | ✅ |
| D (persistence) | `grep -l "parse_rows" docs/ARCHITECTURE.md` | ≥1 hit | 1 hit (§5 P008 entry) | ✅ |
| D (persistence) | `grep -n "P008" docs/CHANGELOG.md` | ≥1 hit (entry at top) | line 7 — entry present | ✅ |
| D (persistence) | `grep -n "state-backfill" README.md` | ≥1 hit (quick-start section) | line 115 — section added | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise bump | 0 packages updated | ✅ |
| E (env drift) | `cargo build --release` from current target | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| F (runtime state) | `grep -E 'ghp_\|...' src/inbox.rs src/cli/state_backfill.rs tests/state_backfill_cli.rs tests/fixtures/*.md tests/fixtures/state-1id.json` | 0 hits | 0 hits — clean | ✅ |
| F (runtime state) | `bash scripts/session-start-banner.sh` | no forbidden key detected | (not run — runtime preflight, not CI gate) | N/A |
| F (runtime state) | Dry-run byte-identity: state file bytes pre vs post `--dry-run` invocation | identical | Test C asserts `pre_bytes == post_bytes` | ✅ |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + D + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code.

**Lệnh chạy (verify Anchors #1, #2, #3, #7, #14, #15, #19):**

```bash
# Anchor #1 — state_backfill stub state
cat src/cli/state_backfill.rs

# Anchor #2 — main.rs StateBackfill variant + dispatch
grep -n "StateBackfill" src/main.rs

# Anchor #3 — state-backfill clap help
cargo run --quiet -- state-backfill --help 2>&1 | head -25

# Anchor #7 — inbox.rs no existing parse_rows
grep -n "fn parse_rows\|pub fn parse_rows" src/inbox.rs

# Anchor #14 — README state-backfill coverage
grep -n "state-backfill" README.md

# Anchor #15 — cli/mod.rs registers state_backfill
grep -n "pub mod state_backfill" src/cli/mod.rs

# Anchor #19 — main.rs StateBackfill dispatch arm shape
grep -n -A 5 "StateBackfill" src/main.rs

# Sub-mech F preflight
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml || echo "clean"

# Baseline test count (post-P007)
cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l
# Expect ~49 (33 unit + 16 integration). Phiếu target: ≥57 after P008 (+4 unit + ≥4 integration).
```

**Output:** Worker fill vào Debate Log Turn 1 Anchor table.

**Hard Stop triggers:**
- Anchor #2 — nếu `Commands::StateBackfill` không tồn tại HOẶC field naming khác (`state: PathBuf` + `inbox: PathBuf` + `dry_run: bool`) → STOP, escalate P001 drift.
- Anchor #3 — nếu `--state` hoặc `--inbox` không REQUIRED HOẶC `--dry-run` không là boolean flag → STOP, escalate ARCHITECTURE §1 drift.
- Anchor #7 — nếu `pub fn parse_rows` ĐÃ tồn tại trong `src/inbox.rs` → STOP, unexpected (P006 không scope). Investigate before proceeding.
- Anchor #15 — nếu `pub mod state_backfill;` MISSING from `cli/mod.rs` → STOP, escalate P001 drift.
- Anchor #19 — nếu `Commands::StateBackfill` dispatch arm ALREADY error-mapped (e.g., touched by prior PR) → STOP, investigate; phiếu spec assumes flat passthrough start state.

---

### Task 1: `src/inbox.rs` — ADD `parse_rows` + extend `InboxError` enum

**File:** `src/inbox.rs`

**Tìm** (existing P006 surface — Worker grep verify post-Task 0):

```rust
// Existing P006 surface:
// - pub fn read_inbox(path: &Path) -> Result<String, InboxError>
// - pub fn insert_rows(content: &str, rows: &[AdvisoryRow]) -> Result<(String, usize), InboxError>
// - pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError>
// - pub enum InboxError { MissingRowsHeading { path }, Io { path, source } }
// - #[cfg(test)] mod tests { ... existing P006 unit tests ... }
```

**Thay đổi 1 — extend `InboxError` enum:** add `ParseRow` variant after existing variants:

```rust
#[derive(thiserror::Error, Debug)]
pub enum InboxError {
    #[error("inbox `{path}` is missing `## Rows` heading — cannot determine insert position")]
    MissingRowsHeading { path: std::path::PathBuf },
    #[error("inbox `{path}` I/O failure: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Row parse failure during `parse_rows`. `line_number` is 1-based line index
    /// in the full inbox content (NOT relative to `## Rows` section start).
    #[error("inbox `{path}` row parse failed at line {line_number}: {source}")]
    ParseRow {
        path: std::path::PathBuf,
        line_number: usize,
        #[source]
        source: crate::row::RowParseError,
    },
}
```

**Thay đổi 2 — ADD `pub fn parse_rows`** (placement: AFTER existing `insert_rows`, BEFORE `write_atomic`, OR placement at end of module per Worker taste — Tầng 2 self-decide):

```rust
/// Parse all rows under the `## Rows` heading from inbox markdown content.
///
/// Behavior:
/// - Returns empty `Vec` if `## Rows` heading is absent (tolerate-empty per spec).
/// - Skips blank lines, HTML-comment blocks (`<!-- ... -->`), column-header row
///   (`| Date | Advisory ID | ... |`), and separator row (`|---...|`).
/// - Stops at next `## ` heading after `## Rows` (preserves future schema extension).
/// - Returns `InboxError::ParseRow` with `path: PathBuf::new()` placeholder on row
///   parse failure — CALLER (e.g., `cli/state_backfill.rs`) MUST re-wrap with real
///   path before bubbling to main.rs.
///
/// First consumer: P008 `state-backfill` subcmd.
pub fn parse_rows(content: &str) -> Result<Vec<crate::row::AdvisoryRow>, InboxError> {
    let lines: Vec<&str> = content.lines().collect();
    let heading_idx = match lines.iter().position(|l| l.trim_end() == "## Rows") {
        Some(idx) => idx,
        None => return Ok(Vec::new()),  // tolerate-empty per locked decision
    };

    let mut rows = Vec::new();
    let mut in_comment = false;

    for (offset, &line) in lines.iter().enumerate().skip(heading_idx + 1) {
        let line_number = offset + 1; // 1-based line index in content

        // Stop at next `## ` heading (start of new section).
        if line.trim_start().starts_with("## ") {
            break;
        }

        // HTML comment block toggle (same-line open+close OR multi-line).
        let starts_comment = line.contains("<!--");
        let ends_comment = line.contains("-->");
        if in_comment {
            if ends_comment {
                in_comment = false;
            }
            continue;
        }
        if starts_comment && !ends_comment {
            in_comment = true;
            continue;
        }
        if starts_comment && ends_comment {
            // same-line comment — skip this line entirely
            continue;
        }

        // Skip blank.
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip separator row `|---|---|...|`.
        if trimmed.starts_with("|---") || trimmed.starts_with("| ---") {
            continue;
        }

        // Skip column-header row `| Date | Advisory ID | ... |`.
        if trimmed.starts_with("| Date |") || trimmed.starts_with("|Date|") {
            continue;
        }

        // Treat as pipe-row, parse via `row::parse_row`.
        match crate::row::parse_row(line) {
            Ok(row) => rows.push(row),
            Err(source) => {
                return Err(InboxError::ParseRow {
                    path: std::path::PathBuf::new(), // placeholder; caller fills
                    line_number,
                    source,
                });
            }
        }
    }

    Ok(rows)
}
```

**Thêm tests** (inside existing `#[cfg(test)] mod tests`):

```rust
#[test]
fn parse_rows_happy_3_rows() {
    let content = "\
# Inbox
## Rows
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-1 | https://x.com/1 | pkg1@<1 | f.rs:1 | High | open | - |
| 2026-05-28 | CVE-2026-2 | https://x.com/2 | pkg2@<2 | f.rs:2 | Medium | processed | reviewed |
| 2026-05-28 | CVE-2026-3 | https://x.com/3 | pkg3@<3 | f.rs:3 | Low | dismissed | n/a |
";
    let rows = parse_rows(content).expect("parse rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].advisory_id, "CVE-2026-1");
    assert_eq!(rows[2].advisory_id, "CVE-2026-3");
}

#[test]
fn parse_rows_empty_section() {
    let content = "\
# Inbox
## Rows

| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
";
    let rows = parse_rows(content).expect("parse rows");
    assert_eq!(rows.len(), 0);
}

#[test]
fn parse_rows_skips_html_comment() {
    let content = "\
# Inbox
## Rows
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-1 | https://x.com/1 | pkg1@<1 | f.rs:1 | High | open | - |
<!--
| 2026-05-23 | GHSA-skip | https://x.com/skip | pkg@<x | indirect | Medium | open | - |
-->
| 2026-05-28 | CVE-2026-2 | https://x.com/2 | pkg2@<2 | f.rs:2 | Medium | processed | reviewed |
";
    let rows = parse_rows(content).expect("parse rows");
    assert_eq!(rows.len(), 2, "comment block row must be skipped");
    assert!(rows.iter().all(|r| r.advisory_id != "GHSA-skip"));
}

#[test]
fn parse_rows_bad_row_returns_parse_row_error() {
    // 5 columns instead of 8 — row::parse_row should fail.
    let content = "\
## Rows
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-1 | bad-row-only-5-cols | High | open |
";
    let err = parse_rows(content).expect_err("should error on malformed row");
    assert!(
        matches!(err, InboxError::ParseRow { .. }),
        "expected ParseRow variant, got {:?}",
        err
    );
}

// Optional 5th test — Worker self-decide:
#[test]
fn parse_rows_no_heading_returns_empty() {
    let content = "# Inbox\n\nNo rows section here.\n";
    let rows = parse_rows(content).expect("tolerate missing heading");
    assert_eq!(rows.len(), 0);
}
```

**Lưu ý:**
- `parse_rows` returns `Result<Vec<_>, InboxError>` with `PathBuf::new()` placeholder for `ParseRow` variant — caller (`cli/state_backfill.rs`) PATTERN-MATCHES and re-wraps with real `inbox` path.
- HTML comment toggle logic — Worker may extract to a private helper if `insert_rows` already has similar logic (P006 ship). Tầng 2 self-decide: dedupe via private helper OR inline duplicate. Recommended: inline duplicate to keep P008 scope tight (no refactor of P006 code). If Worker discovers `insert_rows` has identical helper already exposed → reuse OK.
- Stop-at-next-heading: `line.trim_start().starts_with("## ")` catches `## `-prefixed lines. Worker can extend to `# ` heading too if desired (Tầng 2 self-decide).
- Column-header heuristic: `starts_with("| Date |")` matches ARCHITECTURE §3 example. If future inboxes use different header → row would fail to parse via `row::parse_row` (8 columns required), producing `ParseRow` error — surfaceable in Discovery if false-positive.
- NO `unsafe { }` block.
- New `InboxError::ParseRow` variant: existing P006 callers of `read_inbox`/`insert_rows`/`write_atomic` should NOT match `ParseRow` (none of them call `parse_rows`). Worker verify: `grep -n "InboxError::" src/cli/` — only `cli/append.rs` matches, on `MissingRowsHeading` + `Io` only.

---

### Task 2: `src/cli/state_backfill.rs` — stub → real impl

**File:** `src/cli/state_backfill.rs`

**Tìm** (current P001 stub):

```rust
use std::path::PathBuf;

use anyhow::Result;

pub fn run(state: PathBuf, inbox: PathBuf, dry_run: bool) -> Result<()> {
    println!("TODO: state-backfill subcmd ...");
    Ok(())
}
```

**Thay bằng:**

```rust
//! `state-backfill` subcommand — extract advisory IDs from inbox rows with
//! status `processed`/`dismissed` and union into state.seen_advisories.
//!
//! Recovery path for users whose state file was lost/corrupted but whose
//! inbox markdown retains review decisions. See `docs/ARCHITECTURE.md` §1
//! (CLI surface) for the I/O contract.
//!
//! Sub-mech C invariant: post.seen_advisories ⊇ pre.seen_advisories.
//! `last_scan_at` and `agent_version` are PRESERVED (backfill is not a scan event).

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::inbox::{self, InboxError};
use crate::row::Status;
use crate::state::{self, StateFile};

pub fn run(state_path: PathBuf, inbox_path: PathBuf, dry_run: bool) -> Result<()> {
    // 1. Read existing state (bubble StateReadError → main.rs maps to exit 1).
    let existing = state::read(&state_path)
        .with_context(|| format!("reading state file `{}`", state_path.display()))?;

    // 2. Read inbox markdown (bubble InboxError::Io → main.rs maps to exit 1).
    let inbox_content = inbox::read_inbox(&inbox_path)?;

    // 3. Parse rows; re-wrap ParseRow error with real path.
    let rows = match inbox::parse_rows(&inbox_content) {
        Ok(rs) => rs,
        Err(InboxError::ParseRow { line_number, source, .. }) => {
            return Err(InboxError::ParseRow {
                path: inbox_path.clone(),
                line_number,
                source,
            }
            .into());
        }
        Err(e) => return Err(e.into()),
    };

    // 4. Extract IDs from rows with status processed/dismissed.
    let extracted: BTreeSet<String> = rows
        .iter()
        .filter(|r| matches!(r.status, Status::Processed | Status::Dismissed))
        .map(|r| r.advisory_id.clone())
        .collect();

    // 5. Union with pre-existing seen_advisories.
    let mut union: BTreeSet<String> = existing.seen_advisories.iter().cloned().collect();
    let pre_count = union.len();
    union.extend(extracted);
    let post_count = union.len();
    let backfilled_count = post_count - pre_count;

    // 6. Build updated state (PRESERVE last_scan_at + agent_version + schema_version).
    let updated = StateFile {
        schema_version: existing.schema_version,
        last_scan_at: existing.last_scan_at,
        seen_advisories: union.into_iter().collect(),
        agent_version: existing.agent_version,
    };

    // 7. Write (unless dry-run).
    if !dry_run {
        state::write_atomic(&state_path, &updated)
            .with_context(|| format!("writing backfilled state to `{}`", state_path.display()))?;
    }

    // 8. Emit summary JSON.
    let summary = json!({
        "backfilled_count": backfilled_count,
        "total_seen_after": post_count,
    });
    println!("{}", summary);

    Ok(())
}
```

**Lưu ý:**
- Renamed params: `state` → `state_path`, `inbox` → `inbox_path` to avoid shadowing `crate::state` / `crate::inbox` modules (P005/P007 precedent).
- `anyhow::Context::with_context` adds path-context to state read/write errors — main.rs downcast still works (anyhow preserves inner `StateReadError`/`StateWriteError` types for `downcast_ref`).
- `BTreeSet` automatic dedup + sort. `into_iter().collect::<Vec<_>>()` produces sorted Vec.
- `serde_json::json!` emits keys alphabetical (`backfilled_count` → `total_seen_after`) — integration tests use substring match, NOT exact JSON equality (P004/P005/P006/P007 precedent).
- ParseRow re-wrap pattern: pattern destructure → reconstruct with `path: inbox_path.clone()`. Worker may inline simpler approach if discovered.
- Sub-mech C invariant: `union.extend(extracted)` is monotonic — `union.len() >= pre_count` always (BTreeSet semantics). `backfilled_count = post − pre` is non-negative; no underflow.
- Idempotent re-write: even when `backfilled_count == 0`, we call `state::write_atomic` to canonicalize sort + JSON format (P007 precedent).
- NO `unsafe { }` block.

---

### Task 3: `src/main.rs` dispatch arm — error → exit code map

**File:** `src/main.rs`

**Tìm** (current P001 scaffold passthrough — Worker confirm post-Task 0 via Anchor #19):

```rust
Commands::StateBackfill { state, inbox, dry_run } => {
    cli::state_backfill::run(state, inbox, dry_run)
}
```

**Thay bằng:**

```rust
Commands::StateBackfill { state, inbox, dry_run } => {
    if let Err(e) = cli::state_backfill::run(state, inbox, dry_run) {
        let code = if e.downcast_ref::<crate::inbox::InboxError>().is_some() {
            1
        } else if e.downcast_ref::<crate::state::StateReadError>().is_some() {
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
- Pattern matches `Commands::Append` arm (P006), `Commands::Dedup` arm (P005), `Commands::MigrateState` arm (P007).
- `InboxError` ALL variants (`MissingRowsHeading`, `Io`, `ParseRow`) map to exit 1. No need for nested match.
- `StateReadError` ALL variants (Io, Json, SchemaMismatch) map to exit 1.
- `StateWriteError::Io` → exit 2.
- Tail `Ok(())` REQUIRED per main.rs match-arm uniformity.
- Worker exact-match the existing `Commands::StateBackfill { state, inbox, dry_run } =>` line including brace style — verify via Anchor #19 grep before replacing.

---

### Task 4: Fixtures + integration test

**File:** `tests/fixtures/inbox-5rows-3processed.md` (NEW)

```markdown
# Advisory Inbox

> P008 state-backfill test fixture: 5 rows, 3 processed/dismissed, 2 open.

## Rows

| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-9001 | https://example.com/9001 | next@<15.5.17 | src/middleware.ts:42 | High | processed | reviewed |
| 2026-05-28 | CVE-2026-9002 | https://example.com/9002 | flask@<2.3.5 | app.py:8 | Medium | dismissed | not applicable |
| 2026-05-28 | CVE-2026-9003 | https://example.com/9003 | tokio@<1.40 | src/main.rs:1 | Critical | processed | patched |
| 2026-05-28 | CVE-2026-9004 | https://example.com/9004 | serde@<1.0.200 | src/lib.rs:5 | Low | open | pending review |
| 2026-05-28 | CVE-2026-9005 | https://example.com/9005 | clap@<4.5 | src/main.rs:10 | Medium | open | pending review |
```

**File:** `tests/fixtures/state-1id.json` (NEW)

```json
{
  "schema_version": 1,
  "last_scan_at": "2026-05-23T12:00:00Z",
  "seen_advisories": [
    "CVE-2026-7777"
  ],
  "agent_version": "advisory-watch@0.1.0"
}
```

**File:** `tests/state_backfill_cli.rs` (NEW)

```rust
//! Integration tests for `state-backfill` subcmd (P008).
//!
//! Sub-mech C verification: pre.seen_advisories MUST be subset of post.

use std::path::PathBuf;

use advisory_inbox::state::StateFile;  // OR via crate::state if pub re-exported; Worker self-decides import path
use assert_cmd::Command;
use predicates::str::contains;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_state(path: &std::path::Path) -> StateFile {
    let content = std::fs::read_to_string(path).expect("read state file");
    serde_json::from_str(&content).expect("parse state JSON")
}

#[test]
fn acceptance_5_rows_3_processed_plus_state_1id_produces_4_ids() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    let pre = read_state(&state_path);

    let assert = Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert.stdout(contains("\"backfilled_count\":3"));

    let post = read_state(&state_path);
    // Sub-mech C: pre IDs all present in post.
    for id in &pre.seen_advisories {
        assert!(
            post.seen_advisories.contains(id),
            "Sub-mech C violation: pre ID {} lost",
            id
        );
    }
    // total = 4 (1 pre + 3 new processed/dismissed).
    assert_eq!(post.seen_advisories.len(), 4);
    // Pre ID preserved.
    assert!(post.seen_advisories.contains(&"CVE-2026-7777".to_string()));
    // 3 backfilled IDs present.
    assert!(post.seen_advisories.contains(&"CVE-2026-9001".to_string()));
    assert!(post.seen_advisories.contains(&"CVE-2026-9002".to_string()));
    assert!(post.seen_advisories.contains(&"CVE-2026-9003".to_string()));
    // 2 open rows did NOT contribute.
    assert!(!post.seen_advisories.contains(&"CVE-2026-9004".to_string()));
    assert!(!post.seen_advisories.contains(&"CVE-2026-9005".to_string()));
    // last_scan_at PRESERVED.
    assert_eq!(post.last_scan_at, pre.last_scan_at);
    // agent_version PRESERVED.
    assert_eq!(post.agent_version, pre.agent_version);
}

#[test]
fn already_backfilled_zero_count() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    // Pre-populate state with all 4 expected IDs.
    let pre_state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        seen_advisories: vec![
            "CVE-2026-7777".to_string(),
            "CVE-2026-9001".to_string(),
            "CVE-2026-9002".to_string(),
            "CVE-2026-9003".to_string(),
        ],
        agent_version: "advisory-watch@0.1.0".to_string(),
    };
    let pre_json = serde_json::to_string_pretty(&pre_state).unwrap() + "\n";
    std::fs::write(&state_path, &pre_json).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"backfilled_count\":0"))
        .stdout(contains("\"total_seen_after\":4"));

    let post = read_state(&state_path);
    assert_eq!(post.seen_advisories.len(), 4);
}

#[test]
fn dry_run_does_not_touch_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    let pre_bytes = std::fs::read(&state_path).unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(contains("\"backfilled_count\":3"));

    let post_bytes = std::fs::read(&state_path).unwrap();
    assert_eq!(pre_bytes, post_bytes, "Sub-mech F: dry-run must NOT modify file");
}

#[test]
fn only_processed_and_dismissed_contribute() {
    // This test exists as standalone to make the open-row exclusion explicit.
    // (Already asserted in Test A; duplicate signal is OK for clarity.)
    let tmp = tempfile::tempdir().expect("tempdir");
    let state_path = tmp.path().join("state.json");
    let inbox_path = tmp.path().join("inbox.md");
    std::fs::copy(fixtures_dir().join("state-1id.json"), &state_path).unwrap();
    std::fs::copy(
        fixtures_dir().join("inbox-5rows-3processed.md"),
        &inbox_path,
    )
    .unwrap();

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "state-backfill",
            "--state",
            state_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let post = read_state(&state_path);
    // Open rows from fixture: CVE-2026-9004, CVE-2026-9005.
    assert!(!post.seen_advisories.contains(&"CVE-2026-9004".to_string()));
    assert!(!post.seen_advisories.contains(&"CVE-2026-9005".to_string()));
}
```

**Lưu ý:**
- Test crate name `advisory_inbox` — Worker verifies via `Cargo.toml [package].name` and existing tests `tests/migrate_state_cli.rs` for import conventions. If lib is NOT exposed (binary-only crate, no `lib.rs`), Worker may need to copy `StateFile` shape inline in test OR re-parse via raw `serde_json::Value`. Tầng 2 self-decide: if `advisory_inbox::state::StateFile` doesn't resolve → use `serde_json::Value::Object` and extract fields manually. Alternative: serde inline struct definition in test file mirroring `StateFile` shape (acceptable for integration test isolation).
- `predicates::str::contains` — single-string substring assert; no `.or()` combinator needed in these tests (avoids P007 SD-1 trait-import gotcha).
- `tempfile::tempdir()` — already in dev-deps via assert_cmd transitive OR add if missing. Worker verifies; if missing, escalate (no new dep without phiếu update).
- Test C byte-identity check is THE Sub-mech F dry-run safety contract.
- Worker may also add **Optional Test E** for unparseable inbox → exit 1 if desired; Tầng 2 self-decide.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/inbox.rs` | Task 1: ADD `pub fn parse_rows` + extend `InboxError` enum with `ParseRow` variant + ≥4 unit tests |
| `src/cli/state_backfill.rs` | Task 2: stub → real impl per ARCHITECTURE §1 contract |
| `src/main.rs` | Task 3: update `Commands::StateBackfill` dispatch arm with error→exit-code map |
| `tests/fixtures/inbox-5rows-3processed.md` | Task 4: NEW fixture (5 rows: 3 processed/dismissed + 2 open) |
| `tests/fixtures/state-1id.json` | Task 4: NEW fixture (JSON v1 với 1 pre-existing seen ID) |
| `tests/state_backfill_cli.rs` | Task 4: NEW integration test file (≥4 tests) |
| `docs/ARCHITECTURE.md` | Docs Gate: §5 P008 scaffold-status entry + note `inbox::parse_rows` shipped |
| `docs/CHANGELOG.md` | Docs Gate: P008 entry (top of "Unreleased" or current section) |
| `README.md` | Docs Gate: `state-backfill` quick-start (conditional per Anchor #14) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/state.rs` | `state::read` + `state::write_atomic` + `StateFile` + `StateReadError` + `StateWriteError` — call sites only; no signature change |
| `src/row.rs` | `row::parse_row` + `AdvisoryRow` + `Status` enum (Processed/Dismissed variants) — call sites only |
| `src/sentinel.rs` | Unchanged from P003 |
| `src/cli/parse_report.rs` | Unchanged from P004 |
| `src/cli/dedup.rs` | Unchanged from P005 |
| `src/cli/append.rs` | Unchanged from P006 — verify still compiles after `InboxError` enum extension (Worker grep: `cli/append.rs` matches `InboxError::MissingRowsHeading` + `Io` ONLY, NOT `ParseRow` — exhaustive match fail would compile-error if missing) |
| `src/cli/migrate_state.rs` | Unchanged from P007 |
| `src/cli/mod.rs` | `pub mod state_backfill;` already registered (P001 ship) — verify Anchor #15 |
| `Cargo.toml` | NO new dep added — verify Anchor #11 |
| `docs/security/INVARIANTS.md` | INV-LOCAL-002 unchanged unless Worker decides to append "P008 third caller via state::write_atomic" (Tầng 2 self-decide) |

---

## Luật chơi (Constraints)

1. **No new Cargo deps.** Cargo.toml `[dependencies]` + `[dev-dependencies]` MUST remain identical post-P008. All needed crates (serde_json, chrono, anyhow, thiserror, BTreeSet from std, assert_cmd, predicates, tempfile) already present per P002-P007 chain.

2. **INV-LOCAL-002 atomic write enforced.** State write goes through `state::write_atomic` (P007 ship). FORBIDDEN: `std::fs::write(state_path, ...)`, `OpenOptions::append(true)`, direct `std::fs::rename`. Worker verifies Sub-mech F grep clean.

3. **`--dry-run` is byte-identity contract.** File on disk MUST be bit-identical before and after a `state-backfill --dry-run` invocation. Test C verifies. NO debug print of state content. NO temp file leftover. Sub-mech F invariant.

4. **Sub-mech C invariant — seen_advisories monotonic non-shrink.** `post.seen_advisories ⊇ pre.seen_advisories`. Integration Test A asserts every pre-ID survives. Logic enforced by `BTreeSet::extend` semantics (union, never subtract).

5. **`last_scan_at` PRESERVED — never `Utc::now()` on backfill.** Backfill is a RECOVERY operation, not a scan event. Setting `last_scan_at = Utc::now()` would falsely indicate fresh scan happened → cron may skip work. Worker MUST preserve from `existing.last_scan_at`. Test A asserts pre.last_scan_at == post.last_scan_at.

6. **`agent_version` PRESERVED — never overwrite.** Same rationale as #5. Worker MUST preserve.

7. **Only `Status::Processed` + `Status::Dismissed` rows contribute.** `Status::Open` rows MUST NOT be added to seen_advisories. Test D (or Test A multi-assert) verifies.

8. **No `unsafe { }` blocks.** INV-LOCAL-001 enforce. Escalate if tempted (Hard Stop per RULES.md §12).

9. **No `--force` / `--backup` / `--include-open` flags.** Out-of-scope; do NOT add even if "obviously useful". Hard Stop per RULES.md §12 (CLI surface change ngoài scope).

10. **`InboxError` enum extension is additive.** `MissingRowsHeading` + `Io` variants MUST remain unchanged (P006 lock). Only ADD `ParseRow` variant. Existing match arms in `cli/append.rs` (Worker verifies via grep — currently exhaustive on 2 variants) MUST be updated to handle `ParseRow` OR use `_ =>` wildcard. Worker self-decides: per Rust exhaustive match enforcement, adding 3rd variant WILL break existing exhaustive matches → Worker MUST update `cli/append.rs` match arm to either explicitly handle `ParseRow` (route to exit 1 same as MissingRowsHeading) OR change to wildcard. **Recommended: explicit handle `ParseRow` → exit 1 (consistent with MissingRowsHeading semantic "inbox can't be parsed").** This is a CROSS-PHIẾU TOUCH on `cli/append.rs` — small, mechanical, and forced by enum extension. Tầng 2 self-decide IF Worker discovers `cli/append.rs` does NOT currently match on InboxError variants explicitly (e.g., uses generic `if downcast.is_some()` only) → no change needed. Worker grep `cli/append.rs` for `InboxError::` pattern at Task 0 to decide.

11. **Match-arm uniformity in main.rs.** Dispatch arm ends with `Ok(())` per P004 Turn 1 O1.1 precedent. All sibling arms have identical shape.

12. **Voice: Vietnamese in phiếu body, English in code comments, English in CLI stdout/stderr.** Per CLAUDE.md language convention.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (≥57 tests total post-P008; baseline 49 from P007 + ≥4 unit parse_rows + ≥4 integration state_backfill)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] Acceptance scenario: copy `tests/fixtures/state-1id.json` + `tests/fixtures/inbox-5rows-3processed.md` to tempdir → `cargo run --quiet -- state-backfill --state <tmp-state> --inbox <tmp-inbox>` → exit 0, stdout JSON `{"backfilled_count":3,"total_seen_after":4}`, state file post-condition: 4 seen IDs sorted alphabetically `["CVE-2026-7777", "CVE-2026-9001", "CVE-2026-9002", "CVE-2026-9003"]`, `last_scan_at == "2026-05-23T12:00:00Z"` unchanged.
- [ ] Dry-run safety: same setup → `... --dry-run` → exit 0, stdout same JSON, state file bytes IDENTICAL to pre-invocation (verify via `cmp <pre> <post>` or `diff`).
- [ ] Already-backfilled idempotent: pre-populate state with all 4 IDs → run again → `backfilled_count: 0`, file content stable (sorted JSON form).
- [ ] Inbox unparseable: craft inbox with malformed pipe-row (e.g., truncated to 5 cols) → exit 1, stderr contains "parse" or "row".

### Regression
- [ ] `cargo test --test parse_report_cli` — 3 tests pass (P004 ship unchanged)
- [ ] `cargo test --test dedup_cli` — 4 tests pass (P005 ship unchanged)
- [ ] `cargo test --test append_cli` — 4 tests pass (P006 ship unchanged) — special attention if `cli/append.rs` had to be updated for `InboxError::ParseRow` exhaustive match (constraint #10): rerun and confirm exit codes unchanged.
- [ ] `cargo test --test migrate_state_cli` — 5 tests pass (P007 ship unchanged)
- [ ] `cargo run -- parse-report < tests/fixtures/agent-report-1.md` — unchanged behavior
- [ ] `cargo run -- migrate-state --state <fresh-path>` — unchanged behavior (no regression on state write)

### Docs Gate
- [ ] `docs/CHANGELOG.md` — entry P008 at top (citing `inbox::parse_rows` new, `InboxError::ParseRow` variant new, `state-backfill` wired)
- [ ] `docs/ARCHITECTURE.md` §5 — P008 scaffold-status entry added (after P007 entry): mentions `inbox::parse_rows` shipped + `InboxError::ParseRow` variant + `cli::state_backfill::run` wired + `state::write_atomic` is third caller (P007 second, P008 third — incrementing count if §5 tracks; otherwise just append entry)
- [ ] `README.md` — `state-backfill` quick-start section (conditional per Anchor #14; if section missing add after `migrate-state` quick-start)
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P008.md` — full report written (anchors verified, Sub-mech checks fired, sai lệch documented if any)
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `- 2026-MM-DD P008: state-backfill subcmd shipped, inbox::parse_rows added, 4 IDs union from fixture → see docs/discoveries/P008.md`
- [ ] Sub-mechanism B/C/D/E/F Verification Trace filled (table above)

### Lane assignment
- Classifier output: **Guarded** (filesystem state write via `state::write_atomic`, dry-run no-touch contract, Sub-mech C invariant on seen_advisories).
- Reason files: `src/cli/state_backfill.rs` (NEW filesystem persistence call site), `src/inbox.rs` (extends error enum), `src/main.rs` (exit-code map).
- Override: N/A (no override requested).
