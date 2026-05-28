# PHIẾU P009: scan-and-append composite subcmd

> **ID format:** `P009` — counter `.phieu-counter` = 9 sau P008 ship.
> **Filename:** `docs/ticket/P009-scan-and-append.md`
> **Branch:** `feat/P009-scan-and-append`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (Phase 2 capstone — composite gắn 3 subcmd parse → dedup → append vào 1 lệnh đơn; foundation cho P011 MCP tool `scan_and_append` + P013 tarot install — slash command sẽ gọi 1 lệnh composite thay vì chain 3 lệnh shell)
> **Ảnh hưởng:** `src/cli/scan_and_append.rs` (stub → real impl — compose underlying lib fns, NOT subcmd run() functions), `src/main.rs` (update `Commands::ScanAndAppend { report, inbox, state }` dispatch arm — error → exit code map cover 4 error families: SentinelError/RowParseError/InboxError/StateRead+Write), `tests/scan_and_append_cli.rs` (NEW integration test ≥3 tests), `docs/ARCHITECTURE.md` §5 (P009 scaffold-status entry — `scan-and-append` wired, no new lib module), `docs/CHANGELOG.md` (entry P009 — note atomicity caveat: NOT cross-file atomic; inbox written FIRST, then state; partial-failure recovery = `state-backfill`), `README.md` (`scan-and-append` quick-start nếu chưa cover — Worker check Anchor #14)
> **Dependency:** P001 (CLI scaffold + `Commands::ScanAndAppend` variant + `cli/scan_and_append.rs` stub), P002 (`AdvisoryRow` + `StateFile` + `Status` enum), P003 (`sentinel::extract_block` + `SentinelError`), P004 (`row::parse_row` + `RowParseError`), P005 (`state::read` + `StateReadError`), P006 (`inbox::read_inbox` + `inbox::insert_rows` + `inbox::write_atomic` + `InboxError`), P007 (`state::write_atomic` + `StateWriteError`), P008 (`inbox::parse_rows` — NOT used by P009; just confirms `InboxError::ParseRow` variant exists in exhaustive match patterns) — all shipped 2026-05-28
> **Lane:** **Guarded** (composite of 3 file-write surfaces — writes BOTH `inbox` markdown AND `state` JSON in single invocation; INV-LOCAL-002 applies to both writes; cross-file atomicity caveat documented; Sub-mech F runtime-state check required)
> **Sub-mech áp dụng:** **B** (capability — `cargo check` + `cargo test --test scan_and_append_cli` + `cargo build --release`), **C** (state migration completeness — `seen_advisories[]` post = pre ∪ observed_ids; monotonic non-shrink invariant; `schema_version` + `agent_version` preserved), **F** (runtime state — write-order discipline `inbox first, state second`; no token leak in error wording; grep clean across new code)

---

## Context

### Vấn đề hiện tại

P004-P008 ship 5 đơn subcmd: `parse-report`, `dedup`, `append`, `migrate-state`, `state-backfill`. Pipeline production của tarot advisory-cron là:

```
agent emit report → parse-report → dedup → append + state update
```

Hiện tại Sếp phải shell-pipe 3 lệnh (Bash heredoc 142 dòng trong tarot `/advisory-scan`). P013 sẽ replace heredoc bằng 1 lệnh composite `advisory-inbox scan-and-append`. Phiếu này wire composite logic.

Pipeline (ARCHITECTURE §1 dòng 77-88):
```
advisory-inbox scan-and-append \
  --report <STDIN_OR_FILE> \
  --inbox <FILE> \
  --state <FILE>

→ Behavior: parse-report → dedup → append → state update (atomic), 1 lệnh
→ Output:   { "appended": N, "skipped_dedup": M, "total_open": K }
→ Exit:     0 success, 1..3 per subcmd error mapping
```

Stub hiện tại của `cli/scan_and_append.rs` (P001 scaffold) printf TODO. Sau P009:
- `cargo run -- scan-and-append --report fixtures/agent-report-1.md --inbox <tmp-inbox> --state <tmp-state>` → modified inbox + updated state + JSON stdout, exit 0.

**Composite semantics — KEY ARCHITECTURAL DECISIONS (locked by Architect):**

1. **Compose underlying lib functions, NOT subcmd `run()` functions.** Subcmd `run()` print JSON to stdout (each emits its own envelope). Composite must emit ONE final JSON. Calling 3 `run()` → 3 JSON blobs interleaved on stdout → broken. Worker MUST use `sentinel::extract_block`, `row::parse_row`, `state::read`, `inbox::read_inbox`, `inbox::insert_rows`, `inbox::write_atomic`, `state::write_atomic` directly.

2. **NOT cross-file atomic.** Composite writes 2 files (`inbox` markdown + `state` JSON). Each write is individually atomic via INV-LOCAL-002 (temp+fsync+rename). But the PAIR is not transactional. Failure modes:
   - Inbox write fails → state unchanged → next run will see same observed_ids → no harm. Exit 2 (write error).
   - Inbox write succeeds, state write fails → inbox has new rows but state lacks IDs → next run will RE-APPEND same advisories (duplicate rows!). Recovery: `state-backfill` reconciles state from inbox. Exit 2.
   - Worker MUST document this in CHANGELOG entry + Constraint #3.

3. **Write order: inbox FIRST, then state.** Rationale: if state writes first then inbox fails, state has IDs that inbox lacks → next run silently skips them via dedup → Sếp NEVER sees the advisories → security gap. Inbox-first means worst case is duplicate rows (visible to Sếp, recoverable via state-backfill). Inbox-first is the SAFER ordering. Locked.

4. **`last_scan_at` UPDATED to `Utc::now()`.** Unlike `state-backfill` (recovery — preserve last_scan_at), `scan-and-append` IS a scan event. Setting `last_scan_at = Utc::now()` correctly records that a fresh scan ran. Worker uses `chrono::Utc::now()` (P007 precedent for fresh state file).

5. **`agent_version` PRESERVED — no flag to update.** Architect explicitly REJECTED adding `--agent-version <STR>` flag for MVP. Reason: keep CLI surface minimal. Sếp can manually edit state file if version changes; alternatively, future `migrate-state --agent-version <STR>` could be added (out-of-scope P009). Locked.

6. **`observed_ids` = ALL parsed row IDs (including ones already in state).** This union goes into `state.seen_advisories[]`. Reasoning: P005 dedup semantic — `observed_ids` is the FULL set, used to update state regardless of kept/skipped split. Sub-mech C invariant: post.seen_advisories ⊇ pre.seen_advisories.

7. **Empty sentinel block → exit 0 with `appended: 0, skipped_dedup: 0`.** Treat as "no advisories found". Do NOT exit non-zero just because the agent had nothing to report. State still updated with `last_scan_at = now`.

8. **Missing sentinel block (`SENTINEL_START` / `END` absent from report) → exit 1.** Per `sentinel::extract_block` contract (P003); maps to phiếu spec "1 sentinel block missing".

9. **Stdin vs file:** if `--report` flag absent OR set to `-` (Worker self-decide convention) → read from `std::io::stdin()`. Else `std::fs::read_to_string(&path)`. Phiếu locks: `report: Option<PathBuf>` — None = stdin, Some(path) = file. Worker self-decides if also supporting `-` (Tầng 2). Recommended: only None=stdin for simplicity.

**Pipeline implementation outline:**

```
1. Read report text (stdin or file)
2. sentinel::extract_block(&text)       → Vec<String> raw row lines | SentinelError → exit 1
3. row::parse_row per line              → Vec<AdvisoryRow>          | RowParseError → exit 2
4. state::read(&state_path)             → StateFile                 | StateReadError → exit 1
5. Partition rows into (kept, skipped) vs state.seen_advisories
   observed_ids = ALL row.advisory_id (kept + skipped)
6. inbox::read_inbox(&inbox_path)       → String                    | InboxError::Io → exit 1
7. inbox::insert_rows(&content, &kept)  → (new_content, total_open) | InboxError::MissingRowsHeading → exit 1
8. inbox::write_atomic(&inbox_path, &new_content)                   | InboxError::Io → exit 2
9. Build updated state:
   - seen_advisories = pre.seen_advisories ∪ observed_ids  (BTreeSet)
   - last_scan_at    = Utc::now()
   - schema_version  = pre.schema_version  (preserved, MUST == 1)
   - agent_version   = pre.agent_version   (preserved)
10. state::write_atomic(&state_path, &updated)                      | StateWriteError → exit 2
11. Emit JSON: { "appended": N_kept, "skipped_dedup": N_skipped, "total_open": M }
```

Reference BACKLOG.md P009:
- Lane: Guarded.
- Scope: Compose 3 subcmd (parse → dedup → append + state update) in 1 atomic operation.
- Acceptance: End-to-end fixture → final state + final inbox match expected.
- Sub-mech checks: B, C, F.

### Giải pháp

**3 unit công việc chính:**

1. **`src/cli/scan_and_append.rs` — stub → real impl (compose lib fns):**

   - Signature: `pub fn run(report: Option<PathBuf>, inbox_path: PathBuf, state_path: PathBuf) -> anyhow::Result<()>`.
     - Note: rename param `state` → `state_path` and `inbox` → `inbox_path` to avoid shadowing `crate::state` / `crate::inbox` modules (P005/P007/P008 precedent).
   - Read report:
     - `let report_text = match report { Some(path) => std::fs::read_to_string(&path).with_context(...)?, None => { let mut s = String::new(); std::io::stdin().read_to_string(&mut s)?; s } };`
   - Extract sentinel block: `let raw_lines = sentinel::extract_block(&report_text)?;` — bubbles `SentinelError` → main.rs downcast → exit 1.
   - Parse each row: collect into `Vec<AdvisoryRow>`. If any `parse_row` fails → bubble `RowParseError` → exit 2.
   - Read state: `let pre_state = state::read(&state_path)?;` — bubbles `StateReadError` → exit 1.
   - Partition kept vs skipped + collect observed_ids:
     ```rust
     let seen: std::collections::HashSet<&String> = pre_state.seen_advisories.iter().collect();
     let (kept, skipped): (Vec<AdvisoryRow>, Vec<AdvisoryRow>) = parsed_rows
         .into_iter()
         .partition(|r| !seen.contains(&r.advisory_id));
     let mut observed_ids: BTreeSet<String> = BTreeSet::new();
     observed_ids.extend(kept.iter().map(|r| r.advisory_id.clone()));
     observed_ids.extend(skipped.iter().map(|r| r.advisory_id.clone()));
     ```
     - Worker may use `Vec.contains` if HashSet feels heavy for small N — Tầng 2 self-decide. Spec is semantic, not implementation.
   - Read inbox: `let inbox_content = inbox::read_inbox(&inbox_path)?;` — bubbles `InboxError::Io` → exit 1.
   - Insert rows: `let (new_content, total_open) = inbox::insert_rows(&inbox_content, &kept)?;` — bubbles `InboxError::MissingRowsHeading` → exit 1.
   - **Write inbox FIRST (locked order per Architecture Decision #3):**
     ```rust
     inbox::write_atomic(&inbox_path, &new_content)
         .with_context(|| format!("writing inbox to `{}`", inbox_path.display()))?;
     ```
     Bubbles `InboxError::Io` → exit 2.
   - Build updated state:
     ```rust
     let mut union: BTreeSet<String> = pre_state.seen_advisories.iter().cloned().collect();
     union.extend(observed_ids);
     let updated = StateFile {
         schema_version: pre_state.schema_version,
         last_scan_at:   chrono::Utc::now(),                    // scan event — UPDATE
         seen_advisories: union.into_iter().collect(),          // BTreeSet → sorted Vec
         agent_version:  pre_state.agent_version,               // PRESERVED
     };
     ```
   - Write state SECOND:
     ```rust
     state::write_atomic(&state_path, &updated)
         .with_context(|| format!("writing state to `{}`", state_path.display()))?;
     ```
     Bubbles `StateWriteError::Io` → exit 2.
   - Emit JSON:
     ```rust
     let summary = serde_json::json!({
         "appended":      kept.len(),
         "skipped_dedup": skipped.len(),
         "total_open":    total_open,
     });
     println!("{}", summary);
     ```
   - Return `Ok(())`.

2. **`src/main.rs` — dispatch arm update:**

   - Current state (Anchor #2): `Commands::ScanAndAppend { report, inbox, state }` clap variant exists from P001 scaffold; dispatch arm is flat passthrough `cli::scan_and_append::run(report, inbox, state)` (Anchor #19 verify).
   - Replace with error → exit code map covering 4 error families:
     ```rust
     Commands::ScanAndAppend { report, inbox, state } => {
         if let Err(e) = cli::scan_and_append::run(report, inbox, state) {
             let code = if e.downcast_ref::<crate::sentinel::SentinelError>().is_some() {
                 1
             } else if e.downcast_ref::<crate::row::RowParseError>().is_some() {
                 2
             } else if e.downcast_ref::<crate::state::StateReadError>().is_some() {
                 1
             } else if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
                 match ie {
                     crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                     crate::inbox::InboxError::Io { .. } => 2,
                     crate::inbox::InboxError::ParseRow { .. } => 1,
                 }
             } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
                 2
             } else {
                 // Fallback: stdin read fail / serde err / unexpected → exit 2 (write/IO bucket).
                 2
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```
   - Tail `Ok(())` REQUIRED (P004 Turn 1 O1.1 precedent — match-arm uniformity).
   - Note on clippy `if_same_then_else` (P008 SD-2 precedent): Worker may need to collapse equal-result branches via `||` to satisfy `clippy -D warnings`. Example: `SentinelError || StateReadError || InboxError::MissingRowsHeading || InboxError::ParseRow` all → 1 may collapse. Tầng 2 self-decide; recommended: keep explicit per-variant branches IF clippy doesn't complain (4 distinct error types is acceptable); only collapse if `if_same_then_else` fires.

3. **`tests/scan_and_append_cli.rs` (NEW integration test ≥3 tests):**

   - **Test A — End-to-end happy (acceptance scenario):**
     - Setup: copy `tests/fixtures/agent-report-1.md` (P004 ship — contains valid sentinel block) to tempdir. Copy `tests/fixtures/inbox-baseline.md` (P006 ship — has `## Rows` + 2 existing rows). Create fresh state JSON with 1 pre-existing seen ID that overlaps ONE row from agent-report-1.md (so dedup will skip 1 + keep N-1).
     - Worker self-decide: reuse `tests/fixtures/state-3ids.json` (P005 ship — 3 seen IDs) IF those IDs overlap report rows; OR create new combo fixture `tests/fixtures/state-scan-pre.json` with controlled overlap. Recommended: create new fixture with exactly 1 overlap so kept count is predictable.
     - Run: `Command::cargo_bin("advisory-inbox").args(["scan-and-append", "--report", "<tmp-report>", "--inbox", "<tmp-inbox>", "--state", "<tmp-state>"])`.
     - Assert: exit 0, stdout JSON contains `"appended":<K>`, `"skipped_dedup":<S>`, `"total_open":<M>` (K+S = total parsed rows; values depend on chosen fixture combo).
     - Sub-mech C: pre-state IDs all present in post-state (`for id in pre.seen_advisories { assert!(post.seen_advisories.contains(id)); }`).
     - Sub-mech C: post.seen_advisories.len() >= pre.seen_advisories.len().
     - Inbox post-condition: kept rows present at top of `## Rows`; existing baseline rows preserved.
     - State post-condition: `last_scan_at` BUMPED (different from pre.last_scan_at — assert `post.last_scan_at > pre.last_scan_at`); `agent_version` UNCHANGED; `schema_version == 1`.

   - **Test B — All skipped (full overlap with state):**
     - Setup: state pre-populated with ALL advisory_ids that agent-report-1.md will emit. Inbox baseline copied.
     - Run scan-and-append.
     - Assert: exit 0, stdout `"appended":0`, `"skipped_dedup":<N>` where N = total rows in report.
     - Inbox post-condition: NO new rows added (file content equals baseline structurally — only timestamp-bearing metadata may differ if any).
     - State post-condition: seen_advisories UNCHANGED in count (already had all IDs); `last_scan_at` BUMPED.

   - **Test C — Empty sentinel block:**
     - Setup: craft minimal report markdown with `<!-- INBOX_APPEND_START -->\n<!-- INBOX_APPEND_END -->` (markers present, no rows between). Inbox baseline + minimal state.
     - Run scan-and-append.
     - Assert: exit 0, stdout `"appended":0`, `"skipped_dedup":0`. State `last_scan_at` BUMPED. Inbox UNCHANGED structurally.

   - **(Optional Test D — Atomic write smoke):**
     - Verify post-condition file integrity: state file is parseable JSON, inbox file is non-empty markdown with `## Rows` heading still present, no `.tmp` leftover in parent dir.
     - Worker self-decide; nice-to-have but Test A's read-back assertions cover most of this.

   - **(Optional Test E — Missing sentinel → exit 1):**
     - Report markdown without `INBOX_APPEND_START` marker → exit 1, stderr contains "sentinel" or "marker". State + inbox UNCHANGED (no writes happened — step 2 fails before step 8). Tầng 2 self-decide.

   - Use `tempfile::tempdir()` (already in dev-deps per P006/P007/P008 precedent). Use `assert_cmd::Command::cargo_bin("advisory-inbox")` + `predicates::str::contains` (P004-P008 precedent).

#### Why not call `parse_report::run` / `dedup::run` / `append::run` directly?

Each `run()` function prints its own JSON envelope to stdout. Composing them by calling all three would produce 3 interleaved JSON blobs on stdout — invalid for downstream consumers (P011 MCP tool, tarot slash command). The composite MUST emit ONE final summary JSON. Therefore Worker uses lib-level primitives directly. This duplicates ~15 lines of glue (read state, partition kept/skipped, etc.) — acceptable cost vs. alternative (refactor subcmd `run()` to return values instead of printing — out-of-scope P009).

Note: P011 MCP tool dispatch will share this same logic (P011 will likely extract a `mod scan` helper). For P009, inline in `cli/scan_and_append.rs` is fine. Future refactor when MCP arrives — log Discovery if Worker notices opportunity.

#### Cross-file atomicity caveat (Architectural Decision #2)

`state::write_atomic` + `inbox::write_atomic` each enforce INV-LOCAL-002 individually (temp+fsync+rename, no partial-write within a single file). But the PAIR is NOT transactional. Two-phase commit across filesystem boundaries is out-of-scope (would require WAL or sentinel file — over-engineering per CLAUDE.md AI Bias Warning §1).

Worker MUST:
1. Order writes: inbox first, then state (safer failure mode — see Decision #3).
2. Document caveat in CHANGELOG entry: `"scan-and-append is NOT cross-file atomic. If state write fails after inbox write succeeded, run `advisory-inbox state-backfill` to reconcile."`
3. Constraint #3 enforces write order.

#### Why update `last_scan_at` (vs preserve)?

`scan-and-append` IS a scan event by definition. `last_scan_at` is the "when did we last actually scan" timestamp — used by future watch/cron logic to decide whether to skip work. Setting it to `Utc::now()` is correct. Compare with `state-backfill` (P008) which preserves because backfill is RECOVERY not SCAN.

`schema_version` MUST be preserved (NEVER bumped here — schema migration is `migrate-state`'s job). Worker MUST NOT touch schema_version.

#### `BTreeSet` for dedup + sort

`pre_state.seen_advisories` is `Vec<String>` (per StateFile schema). To union with new observed_ids and emit sorted output, use `BTreeSet<String>` (std lib, no new dep). Pattern matches P008 state-backfill exactly:
```rust
let mut union: BTreeSet<String> = pre_state.seen_advisories.iter().cloned().collect();
union.extend(observed_ids);
let sorted_vec: Vec<String> = union.into_iter().collect();
```
BTreeSet semantics guarantee Sub-mech C monotonic non-shrink: `union.len() >= pre_count` always.

### Scope

- CHỈ sửa: `src/cli/scan_and_append.rs` (stub → real impl), `src/main.rs` (`Commands::ScanAndAppend` dispatch arm only).
- CHỈ tạo: `tests/scan_and_append_cli.rs` (NEW integration test ≥3 tests). Worker self-decides if also creating new combo fixture (e.g., `tests/fixtures/state-scan-pre.json`) OR reusing existing `state-3ids.json` + `inbox-baseline.md` + `agent-report-1.md` with adjusted assertions.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (P009 scaffold-status entry — `scan-and-append` wired, NO new lib module — composite reuses sentinel/row/state/inbox modules); `docs/CHANGELOG.md` (P009 entry + atomicity caveat note); `README.md` (`scan-and-append` quick-start nếu chưa cover — Anchor #14 conditional).
- KHÔNG sửa: `src/sentinel.rs` (P003 lock), `src/row.rs` (P004 lock — `parse_row` + `Display` + `FromStr` + `RowParseError` đã có), `src/state.rs` (P002/P005/P007 lock — `read` + `write_atomic` + `StateFile` + error enums ready), `src/inbox.rs` (P006/P008 lock — `read_inbox` + `insert_rows` + `write_atomic` + `parse_rows` + 3-variant `InboxError` ready), `src/cli/parse_report.rs` (P004 lock), `src/cli/dedup.rs` (P005 lock), `src/cli/append.rs` (P006 lock), `src/cli/migrate_state.rs` (P007 lock), `src/cli/state_backfill.rs` (P008 lock), `src/cli/mod.rs` (P001 already registered `pub mod scan_and_append;` — Worker verify Anchor #15), `Cargo.toml` (NO new dep — all needed crates already present).
- KHÔNG add new `BackfillError`-style enum for scan_and_append. All errors propagate via existing 5 error types: `SentinelError`, `RowParseError`, `StateReadError`, `InboxError`, `StateWriteError`. Main.rs downcasts.
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 scan-and-append: 0/1/2 per subcmd error mapping).
- KHÔNG đổi state schema (`schema_version`, fields). P009 ONLY reads state + writes back same shape with updated `last_scan_at` + extended `seen_advisories`.
- KHÔNG bump `SCHEMA_VERSION` constant.
- KHÔNG đổi `StateFile` shape (P002 lock). KHÔNG đổi `StateReadError` (P005 lock) / `StateWriteError` (P007 lock) / `InboxError` (P006/P008 lock) / `SentinelError` (P003 lock) / `RowParseError` (P004 lock).
- KHÔNG add `--dry-run` flag. Out-of-scope (composite touches 2 files — dry-run semantics would need careful design; defer to future phiếu if requested).
- KHÔNG add `--agent-version <STR>` flag. Out-of-scope per Architectural Decision #5.
- KHÔNG add `--force` / `--no-state-update` / similar flags. Out-of-scope.
- KHÔNG implement 2-phase commit / WAL / cross-file atomicity. Out-of-scope (AI Bias Warning — over-engineering for solo workflow).
- KHÔNG refactor subcmd `run()` functions to return values. Composite uses lib primitives directly. Refactor is separate phiếu (likely P011 when MCP tool needs same pattern).
- KHÔNG tạo `src/scan.rs` helper module. Inline composite logic in `cli/scan_and_append.rs` is sufficient for P009 single consumer. If P011 MCP tool needs same logic, future refactor will extract.
- KHÔNG đụng `tests/fixtures/` existing files (only ADD new combo fixture if needed; do NOT modify `agent-report-1.md`, `inbox-baseline.md`, `state-3ids.json`, etc.).
- KHÔNG add concurrency lock (ARCHITECTURE §10 deferred).
- KHÔNG modify `docs/security/INVARIANTS.md` (INV-LOCAL-002 callers list: P006 inbox, P007 migrate-state, P008 state-backfill — Tầng 2 self-decide if Worker updates to add P009 as 4th caller; recommendation: skip per P008 Discovery precedent).

### Skills consulted

Architect Read `docs/ticket/P008-state-backfill.md` để tham khảo:
- `BTreeSet<String>` union pattern (P008 state-backfill code exact precedent for monotonic union).
- Param rename `state` → `state_path` + `inbox` → `inbox_path` to avoid module shadowing.
- `anyhow::Context::with_context` pattern for path-context on read/write errors.
- Skip-write gate pattern (P009 has no dry-run, so always writes — but pattern reference).
- Test fixture conventions (tempdir + cargo_bin + predicates::str::contains).
- P008 Discovery SD-2 — clippy `if_same_then_else` may force `||` collapse on equal-result branches.
- P008 Discovery Code Reality Map — `inbox::parse_rows` shipped, `InboxError` now 3-variant, exhaustive match in main.rs MUST handle all 3 (including new `ParseRow`).

Architect Read `docs/ticket/P006-append-atomic.md` để tham khảo:
- `inbox::read_inbox` + `inbox::insert_rows` + `inbox::write_atomic` signatures + `InboxError` 2-variant base shape.
- `insert_rows` returns `(String, usize)` — second element is `total_open` count used for stdout JSON.
- main.rs `Commands::Append` dispatch arm exhaustive match shape (precedent for P009 dispatch).
- Atomic write protocol (INV-LOCAL-002) — Worker uses pre-built `inbox::write_atomic` and `state::write_atomic`, no new write code.

Architect Read `docs/ticket/P007-migrate-state.md` để tham khảo:
- `state::write_atomic` ready (P007 ship — second INV-LOCAL-002 user).
- `chrono::Utc::now()` API confirmed available (P007 used it for fresh state file last_scan_at).
- anyhow downcast → exit code idiom in main.rs dispatch.

Architect Read `docs/ticket/P005-dedup.md` để tham khảo:
- `state::read` + `StateReadError` ready (P005).
- Dedup partition logic: `kept` vs `skipped` vs `observed_ids` semantic.
- `observed_ids` = ALL row IDs (kept + skipped), used downstream for state update — P009 IS that downstream consumer.

Architect Read `docs/ticket/P004-parse-report.md` để tham khảo:
- `row::parse_row(&str) -> Result<AdvisoryRow, RowParseError>` + `RowParseError` enum.
- `sentinel::extract_block(&str) -> Result<Vec<String>, SentinelError>` confirmed via P003 reference.
- `tests/fixtures/agent-report-1.md` shipped (P004 fixture — has valid sentinel block).

Architect Read `docs/discoveries/P008.md` Code Reality Map:
- `InboxError` enum has 3 variants `{ MissingRowsHeading, Io, ParseRow }`. P009 main.rs exhaustive match MUST cover all 3 (P009 does not call `parse_rows` so `ParseRow` never surfaces, but compile-time exhaustive match requires the arm).
- `state::write_atomic` callers: P006 inbox-side, P007, P008 — P009 will be 4th caller (Tầng 2: optional INVARIANTS.md update — skip per P008 precedent).
- Fixtures inventory: `inbox-baseline.md`, `rows-2.json`, `state-3ids.json`, `rows-5.json`, `agent-report-1.md`, `state-legacy.txt`, `state-json-v1.json`, `state-garbage.txt`, `inbox-5rows-3processed.md`, `state-1id.json`. Worker self-decides combo for Test A.

Architect Read `docs/discoveries/P007.md` để học:
- `predicates::prelude::PredicateBooleanExt` required for `.or()` combinator (P007 SD-1).
- `std::io::Error::other()` preferred (P007 SD-2 clippy `io_other_error`).
- Skip-write gate pattern (informational — P009 has no dry-run).

Architect Read `docs/ARCHITECTURE.md` §1 dòng 77-88 (scan-and-append spec) + §3 (inbox markdown format) + §5 (module status — confirms no new module needed for P009).

Architect did NOT use context7 for any library (per CLAUDE.md §4 preflight: architect agent envelope = Read/Write/Glob only; context7 NOT in envelope). All chrono/serde/std::collections::BTreeSet APIs already exercised in P002-P008 — no new surface to verify.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> **BẮT BUỘC:** Kiến trúc sư PHẢI grep/verify code thật trước khi viết assumption.
> Thợ đọc bảng này để biết assumption nào đã verify, assumption nào chưa.
> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/scan_and_append.rs` hiện là stub printf TODO (P001 ship). Signature `pub fn run(report: Option<PathBuf>, inbox: PathBuf, state: PathBuf) -> Result<()>` per P001 scaffold pattern (analogous to migrate_state/state_backfill stub shape). | P001 ship; P008 Discovery confirmed all 8 dispatch arms exist; P009 stub should match. | `[needs Worker verify]` | ⏳ TO VERIFY (Worker Task 0: `cat src/cli/scan_and_append.rs`). |
| 2 | `src/main.rs` có `Commands::ScanAndAppend { report: Option<PathBuf>, inbox: PathBuf, state: PathBuf }` clap variant + dispatch arm `cli::scan_and_append::run(report, inbox, state)`. | P008 Discovery confirmed all 8 dispatch arms present; transitively P009's variant should be there at main.rs. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "ScanAndAppend" src/main.rs`). |
| 3 | `Commands::ScanAndAppend` clap variant declares `--report <FILE>` OPTIONAL (Option<PathBuf>) + `--inbox <FILE>` REQUIRED + `--state <FILE>` REQUIRED. | ARCHITECTURE §1 dòng 77-88 spec: `--report <STDIN_OR_FILE>` (optional, stdin fallback); `--inbox <FILE>` + `--state <FILE>` required. | `[needs Worker verify]` | ⏳ TO VERIFY (`cargo run -- scan-and-append --help`). If `--report` declared as REQUIRED → STOP escalate P001 drift. |
| 4 | `src/sentinel.rs` exports `pub fn extract_block(text: &str) -> Result<Vec<String>, SentinelError>` (P003 ship). `SentinelError` enum has variants for missing markers. | P003 ship; P004 wired it; P004 Discovery confirmed signature. | `[verified]` | ✅ Pre-verified (P003 shipped). |
| 5 | `src/row.rs` exports `pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError>` (P004 ship). `RowParseError` enum is `thiserror::Error` carrying parse failure variants. | P004 ship + P008 Anchor #4 transitive confirmation. | `[verified]` | ✅ Pre-verified. |
| 6 | `src/state.rs` exports `pub fn read(path: &Path) -> Result<StateFile, StateReadError>` (P005) and `pub fn write_atomic(path: &Path, state: &StateFile) -> Result<(), StateWriteError>` (P007). `StateFile` 4 fields {schema_version, last_scan_at, seen_advisories, agent_version}. | P005/P007 ship; P008 Anchors #8/#9/#10 confirmed. | `[verified]` | ✅ Pre-verified. |
| 7 | `src/inbox.rs` exports `pub fn read_inbox(path: &Path) -> Result<String, InboxError>`, `pub fn insert_rows(content: &str, rows: &[AdvisoryRow]) -> Result<(String, usize), InboxError>`, `pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError>`. `insert_rows` returns `(new_content, total_open_count)`. | P006 ship; P008 Anchor #6 confirmed. | `[verified]` | ✅ Pre-verified. |
| 8 | `InboxError` enum has 3 variants post-P008: `MissingRowsHeading { path }`, `Io { path, source }`, `ParseRow { path, line_number, source }`. P009 main.rs exhaustive match MUST handle all 3 (compile-time enforcement). | P008 Discovery Code Reality Map line 73 explicit. | `[verified]` | ✅ Pre-verified (P008 shipped). |
| 9 | `chrono::Utc::now()` returns `DateTime<Utc>` matching `StateFile.last_scan_at` field type. | P007 used `Utc::now()` for fresh state file; P007 Anchor #11 confirmed. | `[verified]` | ✅ Pre-verified. |
| 10 | `std::collections::BTreeSet<String>` standard API (`new`, `insert`, `extend`, `into_iter().collect::<Vec<_>>()`). No new dep needed. | P008 used `BTreeSet` exact same pattern. | `[verified]` | ✅ std lib. |
| 11 | `Cargo.toml` `[dependencies]` has `serde_json`, `chrono`, `anyhow`, `thiserror`, `serde`, `tempfile`, `regex`. NO new dep needed. `[dev-dependencies]` has `assert_cmd`, `predicates`, `tempfile` (or transitive). | P008 Anchor #11 confirmed; Cargo.toml unchanged through P002-P008 chain. | `[verified]` | ✅ Pre-verified. |
| 12 | `tests/fixtures/agent-report-1.md` exists with valid sentinel block + parseable rows (P004 ship). | P004 ship + P008 Anchor #13 (fixtures inventory). | `[verified]` | ✅ Pre-verified. |
| 13 | `tests/fixtures/inbox-baseline.md` exists with `## Rows` heading + 2 existing rows (P006 ship). | P006 ship + P008 Anchor #13. | `[verified]` | ✅ Pre-verified. |
| 14 | `README.md` chưa có `scan-and-append` quick-start section (P004→P008 covered the 5 atomic subcmds; scan-and-append untouched). | P008 Discovery sequence — no prior phiếu mentioned scan-and-append quick-start. | `[unverified]` | ⏳ TO VERIFY (`grep -n "scan-and-append" README.md` — expect at most 1 hit in stub list / exit-code table, NOT in quick-start). |
| 15 | `src/cli/mod.rs` đã có `pub mod scan_and_append;` (P001 scaffold ship 8 subcmd modules). | P008 Anchor #15 confirmed `pub mod state_backfill;` at cli/mod.rs:14; transitively all 8 modules registered per P001. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "pub mod scan_and_append" src/cli/mod.rs`). |
| 16 | `docs/ARCHITECTURE.md` §1 scan-and-append subcmd block (dòng 77-88) documents I/O contract: input `--report` optional/stdin, `--inbox` + `--state` required; output `{ appended, skipped_dedup, total_open }`; exit 0/1/2. | Architect Read ARCHITECTURE.md dòng 77-88 during load context. | `[verified]` | ✅ Dòng 77-88 exact match per phiếu Context spec. |
| 17 | `docs/ARCHITECTURE.md` §5 lists P008 scaffold-status entry; P009 entry pending — Worker adds. | Architect Read ARCHITECTURE.md §5 P007 entry; P008 Discovery confirmed §5 updated post-P008. | `[needs Worker verify]` | ⏳ TO VERIFY Worker reads §5 to confirm P008 entry present; then appends P009 entry below. |
| 18 | `src/main.rs` `Commands::ScanAndAppend` dispatch arm currently flat passthrough `cli::scan_and_append::run(report, inbox, state)` (P001 ship; no error mapping yet). | P008 Discovery confirmed only `Commands::Append`/`Dedup`/`MigrateState`/`StateBackfill` arms have error mapping; ScanAndAppend remains stub. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n -A 5 "ScanAndAppend" src/main.rs`). |
| 19 | `tests/fixtures/agent-report-1.md` contains N rows where N is known from P004 Discovery. Worker may need to either: (a) reuse + pre-populate state with overlap for predictable test counts, OR (b) create new combo fixture. | P004 ship; exact row count from agent-report-1.md content. | `[needs Worker verify]` | ⏳ TO VERIFY (`cargo run --quiet -- parse-report < tests/fixtures/agent-report-1.md \| jq '.rows \| length'` to know row count for Test A planning). |
| 20 | `serde_json::json!` macro emits keys in insertion order for object literal (Rust nightly+stable; matches P004-P008 stdout pattern). Integration tests assert via substring `contains("\"appended\":")`, NOT exact JSON equality. | P004-P008 precedent. | `[verified]` | ✅ Pre-verified. |
| 21 | `assert_cmd::Command::cargo_bin("advisory-inbox")` + `predicates::str::contains` standard pattern for integration tests (P004-P008 precedent). | P008 integration test ship. | `[verified]` | ✅ Pre-verified. |
| 22 | Worker MAY create new combo fixture `tests/fixtures/state-scan-pre.json` if existing state fixtures (`state-3ids.json`, `state-1id.json`) don't have correct overlap with `agent-report-1.md` advisory_ids. Tầng 2 self-decide; phiếu allows but does NOT require. | Locked Architect decision: Worker self-decides fixture strategy as long as Test A acceptance assertions hold. | `[needs Worker verify]` | ⏳ TO VERIFY Worker inspects agent-report-1.md row IDs vs state-3ids.json seen_advisories; picks reuse OR new-fixture path. |

**Note:** Worker EXECUTE Task 0 runs ~7 grep/check verifications (Anchors #1/#2/#3/#14/#15/#17/#18/#19/#22 → 9 anchors `[needs Worker verify]` + `[unverified]`). If ANY anchor diverges from spec → STOP, escalate, write Discovery Report.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. Anchors #14, #17 flagged `[unverified]` — Worker grep confirms before adding docs sections.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Anchor verification (Task 0 results):**

- Anchor #1 ✅ — `src/cli/scan_and_append.rs` is stub, signature: `pub fn run(report: Option<PathBuf>, inbox: PathBuf, state: PathBuf) -> Result<()>` — confirmed at `src/cli/scan_and_append.rs:6`.
- Anchor #2 ✅ — `Commands::ScanAndAppend { report, inbox, state }` exists at `src/main.rs:71-81` with `report: Option<PathBuf>`, `inbox: PathBuf`, `state: PathBuf`. Dispatch arm at `src/main.rs:179-183` is flat passthrough `cli::scan_and_append::run(report, inbox, state)` — no error mapping yet. Matches spec.
- Anchor #3 ✅ — `--report` is `Option<PathBuf>` (optional), `--inbox` + `--state` are required `PathBuf`. Matches ARCHITECTURE §1.
- Anchor #14 ✅ — `grep "scan-and-append" README.md` → 0 hits. README does not have quick-start; Worker must add.
- Anchor #15 ✅ — `src/cli/mod.rs:12` has `pub mod scan_and_append;`.
- Anchor #17 ✅ — `docs/ARCHITECTURE.md` has P008 entry at line 253. No P009 entry yet — Worker must add.
- Anchor #18 ✅ — Dispatch arm is flat passthrough, no error mapping. Ready for replacement.
- Anchor #19 ✅ — `agent-report-1.md` yields 2 rows: `CVE-2026-9999` + `GHSA-aaaa-bbbb`. Total N=2.
- Anchor #22 ✅ — `state-3ids.json` has `GHSA-aaaa-bbbb` (overlaps report). So: 1 overlap (skip) + 1 new (keep). Controlled split achievable by reusing `state-3ids.json`.
- Sub-mech F ✅ — Token scan clean (`grep -E 'ghp_|...'` → 0 hits in src/ + tests/ + Cargo.toml).
- Baseline test count: 59 tests (matches phiếu expectation "~59"). Target post-P009: ≥62.

**Objection:**

### O1.1 [mechanical] — `inbox::insert_rows` call-site in phiếu examples omits the `path` argument

**Phiếu spec** (Task 1 pipeline outline step 7 + Task 1 code block):
```
inbox::insert_rows(&inbox_content, &kept)   // 2-param call — copy error
```

**Code reality** (`src/inbox.rs:66-70`):
```rust
pub fn insert_rows(
    content: &str,
    rows: &[AdvisoryRow],
    path: &Path,
) -> Result<(String, usize), InboxError>
```

The real `insert_rows` takes a 3rd `path: &Path` parameter (used to populate `InboxError::MissingRowsHeading { path }` for user-facing messages). The function signature is locked by P006 — unchanged. The phiếu's call-site examples are copy errors that omit `&inbox_path`. Does NOT change behavior or API — the library API is already correct. Worker self-applies: call `insert_rows(&inbox_content, &kept, &inbox_path)` in Task 1 implementation.

**Classification [mechanical]:** Library API unchanged (P006 lock). No behavior change. No new surface. Phiếu's code examples have a typo — Worker self-corrects in implementation.

**Self-applied fix:** Pass `&inbox_path` as third argument to `inbox::insert_rows`. No Architect re-spec required per taxonomy.

**Status:** ✅ RESOLVED (mechanical — Worker self-applied)

### Turn 1 — Architect Response
Not required for `[mechanical]` objection — Worker self-applied per taxonomy doctrine.

**Status:** ✅ V1 ACCEPTED with mechanical correction noted

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V1 (mechanical correction self-applied by Worker in implementation)
- Total turns: 1 (mechanical objection — no Architect respond cycle needed)
- Approved: 2026-05-28 — code execution may begin

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
| B (capability) | `cargo test --test scan_and_append_cli` | ≥3 integration tests pass (happy / all-skipped / empty-block) | 3 pass | ✅ |
| B (capability) | `cargo build --release` | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| B (capability) | `cargo run --quiet -- scan-and-append --report tests/fixtures/agent-report-1.md --inbox <tmp> --state <tmp>` | exit 0, stdout JSON `{appended, skipped_dedup, total_open}` | verified via Test A | ✅ |
| C (state monotonic) | Test A: pre IDs all present in post | pre.seen_advisories ⊆ post.seen_advisories | 3 pre IDs all in 4-ID post | ✅ |
| C (state monotonic) | Test A: `last_scan_at` bumped to Utc::now() | post.last_scan_at > pre.last_scan_at | 2026-01-01 → 2026-05-28 | ✅ |
| C (state monotonic) | Test A: `agent_version` preserved | post.agent_version == pre.agent_version | "advisory-watch@0.1.0" | ✅ |
| C (state monotonic) | Test A: `schema_version` preserved == 1 | unchanged | 1 | ✅ |
| C (state monotonic) | Test B: all-skipped → seen_advisories.len() unchanged | post.len() == pre.len() | 2 == 2 | ✅ |
| C (state monotonic) | Test C: empty block → state still bumps last_scan_at | post.last_scan_at > pre.last_scan_at | ✅ | ✅ |
| D (persistence) | `grep -n "P009" docs/CHANGELOG.md` | ≥1 hit (entry at top) | ✅ | ✅ |
| D (persistence) | `grep -l "scan-and-append" docs/ARCHITECTURE.md` | ≥1 hit (§5 P009 entry) | ✅ | ✅ |
| D (persistence) | `grep -n "scan-and-append" README.md` | ≥1 hit (quick-start section) | ✅ | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise bump (0 packages updated) | N/A (clean build) | ✅ |
| E (env drift) | `cargo build --release` from current target | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/cli/scan_and_append.rs src/main.rs tests/scan_and_append_cli.rs` | 0 hits | 0 hits | ✅ |
| F (runtime state) | Write order: inbox first then state (assert via code inspection — order of `inbox::write_atomic` vs `state::write_atomic` calls) | inbox before state in source | `inbox::write_atomic` step 8 before `state::write_atomic` step 10 | ✅ |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + D + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code.

**Lệnh chạy (verify Anchors #1, #2, #3, #14, #15, #17, #18, #19, #22):**

```bash
# Anchor #1 — scan_and_append stub state
cat src/cli/scan_and_append.rs

# Anchor #2 + #18 — main.rs ScanAndAppend variant + dispatch arm
grep -n -A 5 "ScanAndAppend" src/main.rs

# Anchor #3 — clap help confirms --report optional, --inbox/--state required
cargo run --quiet -- scan-and-append --help 2>&1 | head -25

# Anchor #14 — README scan-and-append coverage
grep -n "scan-and-append" README.md

# Anchor #15 — cli/mod.rs registers scan_and_append
grep -n "pub mod scan_and_append" src/cli/mod.rs

# Anchor #17 — ARCHITECTURE §5 has P008 entry; P009 not yet
grep -n "P008\|P009" docs/ARCHITECTURE.md

# Anchor #19 — row count in agent-report-1.md (decides fixture strategy)
cargo run --quiet -- parse-report < tests/fixtures/agent-report-1.md 2>&1 | head -50
# Worker counts rows in output, notes advisory_ids for fixture planning.

# Anchor #22 — overlap check between agent-report-1.md row IDs and state-3ids.json seen_advisories
cat tests/fixtures/state-3ids.json
# Compare seen_advisories[] vs row.advisory_id values from previous command.

# Sub-mech F preflight
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml || echo "clean"

# Baseline test count (post-P008)
cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l
# Expect ~59 (39 unit + 20 integration per P008 Discovery). Phiếu target: ≥62 after P009 (+3 integration minimum).
```

**Output:** Worker fill vào Debate Log Turn 1 Anchor table.

**Hard Stop triggers:**
- Anchor #2 — nếu `Commands::ScanAndAppend` không tồn tại HOẶC field naming khác (`report: Option<PathBuf>` + `inbox: PathBuf` + `state: PathBuf`) → STOP, escalate P001 drift.
- Anchor #3 — nếu `--report` declared REQUIRED (not Option), OR `--inbox`/`--state` không REQUIRED → STOP, escalate ARCHITECTURE §1 drift.
- Anchor #15 — nếu `pub mod scan_and_append;` MISSING from `cli/mod.rs` → STOP, escalate P001 drift.
- Anchor #18 — nếu `Commands::ScanAndAppend` dispatch arm ALREADY error-mapped (e.g., touched by prior PR) → STOP, investigate; phiếu spec assumes flat passthrough start state.

---

### Task 1: `src/cli/scan_and_append.rs` — stub → real impl

**File:** `src/cli/scan_and_append.rs`

**Tìm** (current P001 stub):

```rust
use std::path::PathBuf;

use anyhow::Result;

pub fn run(report: Option<PathBuf>, inbox: PathBuf, state: PathBuf) -> Result<()> {
    println!("TODO: scan-and-append composite subcmd ...");
    Ok(())
}
```

(Worker verify exact stub shape via Task 0 Anchor #1; param naming may already be `state`/`inbox` — Task 1 renames to `state_path`/`inbox_path`.)

**Thay bằng:**

```rust
//! `scan-and-append` composite subcommand — compose parse → dedup → append + state update
//! into a single invocation.
//!
//! See `docs/ARCHITECTURE.md` §1 (CLI surface) for the I/O contract.
//!
//! **Atomicity note:** This composite writes TWO files (`inbox` markdown + `state` JSON).
//! Each write is individually atomic via INV-LOCAL-002 (temp+fsync+rename), but the PAIR
//! is NOT cross-file transactional. Write order is inbox FIRST, then state — if state
//! write fails after inbox succeeded, inbox has new rows but state lacks IDs; recovery
//! path is `advisory-inbox state-backfill`. See phiếu P009 Constraint #3.
//!
//! Sub-mech C invariant: post.seen_advisories ⊇ pre.seen_advisories (BTreeSet union).
//! `last_scan_at` UPDATED to Utc::now() (scan event); `agent_version` PRESERVED.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::inbox;
use crate::row::{self, AdvisoryRow};
use crate::sentinel;
use crate::state::{self, StateFile};

pub fn run(report: Option<PathBuf>, inbox_path: PathBuf, state_path: PathBuf) -> Result<()> {
    // 1. Read report text (stdin if report is None, else file).
    let report_text = match report {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading report file `{}`", path.display()))?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .context("reading report from stdin")?;
            s
        }
    };

    // 2. Extract sentinel block → raw row lines. Bubble SentinelError → main.rs maps to exit 1.
    let raw_lines = sentinel::extract_block(&report_text)?;

    // 3. Parse each row. Bubble RowParseError → main.rs maps to exit 2.
    let parsed_rows: Vec<AdvisoryRow> = raw_lines
        .iter()
        .map(|line| row::parse_row(line))
        .collect::<Result<Vec<_>, _>>()?;

    // 4. Read existing state. Bubble StateReadError → main.rs maps to exit 1.
    let pre_state = state::read(&state_path)
        .with_context(|| format!("reading state file `{}`", state_path.display()))?;

    // 5. Partition into (kept, skipped) vs pre_state.seen_advisories.
    //    observed_ids = ALL row.advisory_id (used downstream to update state).
    let seen: std::collections::HashSet<String> =
        pre_state.seen_advisories.iter().cloned().collect();
    let (kept, skipped): (Vec<AdvisoryRow>, Vec<AdvisoryRow>) = parsed_rows
        .into_iter()
        .partition(|r| !seen.contains(&r.advisory_id));
    let observed_ids: BTreeSet<String> = kept
        .iter()
        .chain(skipped.iter())
        .map(|r| r.advisory_id.clone())
        .collect();

    // 6. Read inbox markdown. Bubble InboxError::Io → main.rs maps to exit 1.
    let inbox_content = inbox::read_inbox(&inbox_path)?;

    // 7. Insert kept rows into inbox content (newest at top of `## Rows`).
    //    Bubble InboxError::MissingRowsHeading → exit 1.
    let (new_content, total_open) = inbox::insert_rows(&inbox_content, &kept)?;

    // 8. Write inbox FIRST (locked order per Architecture Decision #3).
    //    InboxError::Io on write → main.rs maps to exit 2.
    inbox::write_atomic(&inbox_path, &new_content)
        .with_context(|| format!("writing inbox to `{}`", inbox_path.display()))?;

    // 9. Build updated state: union seen_advisories with observed_ids, bump last_scan_at.
    let mut union: BTreeSet<String> = pre_state.seen_advisories.iter().cloned().collect();
    union.extend(observed_ids);
    let updated = StateFile {
        schema_version: pre_state.schema_version,
        last_scan_at: chrono::Utc::now(), // scan event — UPDATE timestamp
        seen_advisories: union.into_iter().collect(),
        agent_version: pre_state.agent_version,
    };

    // 10. Write state SECOND. StateWriteError::Io → main.rs maps to exit 2.
    state::write_atomic(&state_path, &updated)
        .with_context(|| format!("writing state to `{}`", state_path.display()))?;

    // 11. Emit summary JSON to stdout.
    let summary = json!({
        "appended": kept.len(),
        "skipped_dedup": skipped.len(),
        "total_open": total_open,
    });
    println!("{}", summary);

    Ok(())
}
```

**Lưu ý:**
- Renamed params: `state` → `state_path`, `inbox` → `inbox_path` to avoid shadowing `crate::state` / `crate::inbox` modules (P005/P007/P008 precedent).
- `std::io::Read` trait import REQUIRED for `stdin().read_to_string(&mut s)`. Worker verify import in `use` block.
- `anyhow::Context::with_context` adds path-context to errors — main.rs downcast still works (anyhow preserves inner concrete error types).
- `BTreeSet<String>` automatic dedup + sort; `into_iter().collect::<Vec<_>>()` produces sorted Vec for `seen_advisories`.
- `HashSet<String>` used for O(1) `contains` lookup during partition — small N (~5-50 rows) so `Vec::contains` also OK; Tầng 2 self-decide.
- `chrono::Utc::now()` available (P007 precedent confirmed).
- NO `unsafe { }` block.
- NO `process::exit` in `run()` — bubble via `anyhow::Result`. main.rs maps.
- Order of writes: inbox FIRST (step 8), state SECOND (step 10). Constraint #3 enforces this; Worker MUST NOT swap.
- `serde_json::json!` emits keys in insertion order (`appended` → `skipped_dedup` → `total_open`); integration tests use substring match (P004-P008 precedent).
- If `kept` is empty (all rows already seen) → `insert_rows` returns `(content.to_string(), recounted_total_open)` per P006 contract (no-op clone); still proceed with state write (still bump last_scan_at).
- If `raw_lines` is empty (empty sentinel block) → `parsed_rows` empty → `kept` empty → no-op inbox; state still bumps last_scan_at. Test C verifies.

---

### Task 2: `src/main.rs` — `Commands::ScanAndAppend` dispatch arm error → exit code map

**File:** `src/main.rs`

**Tìm** (current P001 scaffold passthrough — Worker confirm post-Task 0 via Anchor #18):

```rust
Commands::ScanAndAppend { report, inbox, state } => {
    cli::scan_and_append::run(report, inbox, state)
}
```

**Thay bằng:**

```rust
Commands::ScanAndAppend { report, inbox, state } => {
    if let Err(e) = cli::scan_and_append::run(report, inbox, state) {
        let code = if e.downcast_ref::<crate::sentinel::SentinelError>().is_some() {
            1
        } else if e.downcast_ref::<crate::row::RowParseError>().is_some() {
            2
        } else if e.downcast_ref::<crate::state::StateReadError>().is_some() {
            1
        } else if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
            match ie {
                crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                crate::inbox::InboxError::Io { .. } => 2,
                crate::inbox::InboxError::ParseRow { .. } => 1,
            }
        } else if e.downcast_ref::<crate::state::StateWriteError>().is_some() {
            2
        } else {
            // Fallback: stdin read fail / unexpected → exit 2 (write/IO bucket).
            2
        };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

**Lưu ý:**
- Pattern matches `Commands::Append` arm (P006), `Commands::Dedup` arm (P005), `Commands::MigrateState` arm (P007), `Commands::StateBackfill` arm (P008).
- 4 distinct error families covered: SentinelError + RowParseError + StateReadError + InboxError (3 variants) + StateWriteError + fallback.
- `InboxError::ParseRow` arm is for exhaustive match compile-time enforcement; `scan-and-append` never calls `parse_rows` so this variant should not surface in practice. Worker MUST include the arm anyway (compile error otherwise per P008 Code Reality Map).
- Tail `Ok(())` REQUIRED per main.rs match-arm uniformity.
- Worker exact-match the existing `Commands::ScanAndAppend { report, inbox, state } =>` line including brace style — verify via Anchor #18 grep before replacing.
- **Clippy `if_same_then_else` watchout (P008 SD-2 precedent):** Three branches return `1` (SentinelError, StateReadError, the two InboxError sub-variants). If `cargo clippy --all-targets -- -D warnings` fires `if_same_then_else`, Worker may need to collapse some branches via `||` operator. Recommended approach: keep explicit per-type branches IF clippy passes (4 distinct error TYPES is usually accepted); only collapse if clippy explicitly complains. Tầng 2 self-decide collapse strategy.

---

### Task 3: `tests/scan_and_append_cli.rs` — NEW integration test ≥3 tests

**File:** `tests/scan_and_append_cli.rs`

**Tìm:** (file does not exist yet)

**Tạo mới:**

```rust
//! Integration tests for `scan-and-append` composite subcmd (P009).
//!
//! Sub-mech C verification: state.seen_advisories monotonic non-shrink across composite invocation.
//! Sub-mech F verification: write order is inbox first then state (no token leak in error wording).

use std::path::PathBuf;

use advisory_inbox::state::StateFile;  // OR Worker self-decides import path if not pub re-exported
use assert_cmd::Command;
use predicates::str::contains;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_state(path: &std::path::Path) -> StateFile {
    let content = std::fs::read_to_string(path).expect("read state file");
    serde_json::from_str(&content).expect("parse state JSON")
}

/// Test A — Happy end-to-end: parse → dedup (some overlap) → append (some kept) → state union + last_scan_at bump.
///
/// Worker self-decides fixture combination per Anchor #19 + #22:
/// - Option A1: reuse agent-report-1.md + inbox-baseline.md + state-3ids.json IF overlap is non-trivial.
/// - Option A2: create new combo fixture (e.g., state-scan-pre.json) with controlled overlap.
///
/// Either way, assert:
/// - exit 0
/// - stdout JSON contains "appended":K, "skipped_dedup":S, "total_open":M
/// - K + S == total rows parsed from report
/// - Sub-mech C: every pre.seen_advisories ID survives in post.seen_advisories
/// - post.seen_advisories.len() >= pre.seen_advisories.len()
/// - post.last_scan_at > pre.last_scan_at  (UPDATED)
/// - post.agent_version == pre.agent_version  (PRESERVED)
/// - post.schema_version == 1  (PRESERVED)
/// - Inbox post-condition: K new rows present at top of `## Rows`; original baseline rows preserved.
#[test]
fn acceptance_end_to_end_some_kept_some_skipped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    // Copy fixtures.
    std::fs::copy(fixtures_dir().join("agent-report-1.md"), &report_path).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();
    // Worker chooses state fixture per Anchor #22:
    //   - Reuse state-3ids.json if its seen_advisories overlap with report row IDs.
    //   - Else build inline StateFile with controlled overlap (1 ID matching report).
    // Recommended: inline build for predictable assertions.
    let pre_state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        seen_advisories: vec![
            // Worker self-decides: pick 1 ID from agent-report-1.md to force 1 skip.
            // Falls back to no-overlap if Worker can't determine IDs at write time → all kept.
            "CVE-2026-PLACEHOLDER".to_string(),
        ],
        agent_version: "advisory-watch@0.1.0".to_string(),
    };
    let pre_json = serde_json::to_string_pretty(&pre_state).unwrap() + "\n";
    std::fs::write(&state_path, &pre_json).unwrap();

    let pre_snapshot = read_state(&state_path);

    let assert = Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert
        .stdout(contains("\"appended\":"))
        .stdout(contains("\"skipped_dedup\":"))
        .stdout(contains("\"total_open\":"));

    let post = read_state(&state_path);

    // Sub-mech C: every pre ID present in post.
    for id in &pre_snapshot.seen_advisories {
        assert!(
            post.seen_advisories.contains(id),
            "Sub-mech C violation: pre ID {} lost",
            id
        );
    }
    // Monotonic non-shrink.
    assert!(post.seen_advisories.len() >= pre_snapshot.seen_advisories.len());
    // last_scan_at UPDATED (Utc::now() > pre 2026-05-23 fixture timestamp).
    assert!(
        post.last_scan_at > pre_snapshot.last_scan_at,
        "last_scan_at must be bumped (scan event)"
    );
    // agent_version + schema_version PRESERVED.
    assert_eq!(post.agent_version, pre_snapshot.agent_version);
    assert_eq!(post.schema_version, 1);

    // Inbox post-condition: `## Rows` heading still present, non-empty content.
    let inbox_post = std::fs::read_to_string(&inbox_path).expect("read inbox post");
    assert!(inbox_post.contains("## Rows"));
}

/// Test B — All skipped: state pre-populated with ALL advisory_ids from report → 0 appended.
///
/// Setup requires knowing the exact advisory_ids in agent-report-1.md. Worker self-decides:
/// - Option B1: extract IDs at test runtime via `parse-report` invocation, then build state.
/// - Option B2: inline known-list of IDs (brittle if fixture changes).
/// - Option B3: create new combo fixture (e.g., state-all-overlap.json).
///
/// Recommended: Option B1 (chain parse-report to extract IDs first).
#[test]
fn all_skipped_full_overlap_with_state() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    std::fs::copy(fixtures_dir().join("agent-report-1.md"), &report_path).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();

    // Extract advisory_ids from report via parse-report subcommand.
    let parse_output = Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args(["parse-report", "--input", report_path.to_str().unwrap()])
        .output()
        .expect("parse-report invocation");
    assert!(parse_output.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&parse_output.stdout)
        .expect("parse-report JSON");
    let report_ids: Vec<String> = parsed["rows"]
        .as_array()
        .expect("rows array")
        .iter()
        .map(|r| r["advisory_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !report_ids.is_empty(),
        "agent-report-1.md should yield ≥1 row"
    );

    // Pre-populate state with ALL report IDs.
    let pre_state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        seen_advisories: report_ids.clone(),
        agent_version: "advisory-watch@0.1.0".to_string(),
    };
    let pre_json = serde_json::to_string_pretty(&pre_state).unwrap() + "\n";
    std::fs::write(&state_path, &pre_json).unwrap();

    let pre_snapshot = read_state(&state_path);
    let pre_inbox = std::fs::read_to_string(&inbox_path).expect("pre inbox");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"appended\":0"))
        .stdout(contains(&format!(
            "\"skipped_dedup\":{}",
            report_ids.len()
        )));

    let post = read_state(&state_path);
    // No new IDs added (already had all).
    assert_eq!(post.seen_advisories.len(), pre_snapshot.seen_advisories.len());
    // last_scan_at still BUMPED (scan happened, even if nothing kept).
    assert!(post.last_scan_at > pre_snapshot.last_scan_at);

    // Inbox should be structurally unchanged in row count (no new rows inserted).
    // Loose assertion: file still parseable + has `## Rows`.
    let post_inbox = std::fs::read_to_string(&inbox_path).expect("post inbox");
    assert!(post_inbox.contains("## Rows"));
    // Stronger assertion: row count unchanged. Worker self-decide if checking exact equality
    // (insert_rows on empty kept slice may still produce byte-different output due to
    // total_open recount or trailing whitespace normalization).
    let _ = pre_inbox; // suppress unused warning if not asserted
}

/// Test C — Empty sentinel block: markers present but block is empty → 0 appended, 0 skipped, state still bumps last_scan_at.
#[test]
fn empty_sentinel_block_zero_counts_state_still_bumps() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let report_path = tmp.path().join("report.md");
    let inbox_path = tmp.path().join("inbox.md");
    let state_path = tmp.path().join("state.json");

    // Craft minimal report with empty sentinel block.
    let empty_report = "\
# Agent Report

No advisories found this run.

<!-- INBOX_APPEND_START -->
<!-- INBOX_APPEND_END -->
";
    std::fs::write(&report_path, empty_report).unwrap();
    std::fs::copy(fixtures_dir().join("inbox-baseline.md"), &inbox_path).unwrap();

    let pre_state = StateFile {
        schema_version: 1,
        last_scan_at: chrono::DateTime::parse_from_rfc3339("2026-05-23T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        seen_advisories: vec!["CVE-2026-7777".to_string()],
        agent_version: "advisory-watch@0.1.0".to_string(),
    };
    let pre_json = serde_json::to_string_pretty(&pre_state).unwrap() + "\n";
    std::fs::write(&state_path, &pre_json).unwrap();

    let pre_snapshot = read_state(&state_path);

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .args([
            "scan-and-append",
            "--report",
            report_path.to_str().unwrap(),
            "--inbox",
            inbox_path.to_str().unwrap(),
            "--state",
            state_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("\"appended\":0"))
        .stdout(contains("\"skipped_dedup\":0"));

    let post = read_state(&state_path);
    // No new IDs.
    assert_eq!(post.seen_advisories, pre_snapshot.seen_advisories);
    // last_scan_at STILL bumped (scan event regardless of empty block).
    assert!(
        post.last_scan_at > pre_snapshot.last_scan_at,
        "empty block still counts as scan event"
    );
}
```

**Lưu ý:**
- Test crate name `advisory_inbox` — Worker verifies via `Cargo.toml [package].name` + existing test files (`tests/state_backfill_cli.rs` from P008 precedent). If lib path doesn't expose `StateFile` → Worker uses `serde_json::Value` fallback (P008 same caveat).
- `tempfile::tempdir()` already in dev-deps per P006/P007/P008 precedent.
- `predicates::str::contains` — single-string substring (no `.or()` combinator needed; avoids P007 SD-1 trait-import gotcha).
- Test A's `CVE-2026-PLACEHOLDER` literal: Worker MUST replace with a real ID present in `agent-report-1.md` OR leave non-overlapping (in which case ALL rows kept; assertions adjust accordingly). Tầng 2 self-decide — phiếu allows either as long as Sub-mech C assertions hold.
- Test B uses parse-report subcommand to extract IDs at runtime — robust to fixture changes. Worker may switch to inline known-IDs if `parse-report --input <file>` doesn't accept file arg (verify Anchor #3 of P004 if confused).
- Test C empty-block report uses inline string — does not require new fixture file.
- Optional **Test D (atomic write smoke)** and **Test E (missing sentinel → exit 1)** are Tầng 2 self-decide; nice-to-have but not required.
- Worker MAY create new combo fixture `tests/fixtures/state-scan-pre.json` if inline `StateFile` literals feel awkward. NOT required.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/cli/scan_and_append.rs` | Task 1: stub → real impl (compose lib fns parse → dedup → append + state update) |
| `src/main.rs` | Task 2: update `Commands::ScanAndAppend` dispatch arm with 5-family error → exit-code map |
| `tests/scan_and_append_cli.rs` | Task 3: NEW integration test file (≥3 tests: happy / all-skipped / empty-block) |
| `docs/ARCHITECTURE.md` | Docs Gate: §5 P009 scaffold-status entry (`scan-and-append` wired, NO new lib module) |
| `docs/CHANGELOG.md` | Docs Gate: P009 entry — note `scan-and-append` ships composite + atomicity caveat (not cross-file atomic; inbox-first write order; recovery = state-backfill) |
| `README.md` | Docs Gate: `scan-and-append` quick-start section (conditional per Anchor #14; add after `state-backfill` quick-start) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/sentinel.rs` | `sentinel::extract_block` + `SentinelError` — call sites only; no signature change (P003 lock) |
| `src/row.rs` | `row::parse_row` + `AdvisoryRow` + `RowParseError` + `Status`/`Severity` enums — call sites only (P004 lock) |
| `src/state.rs` | `state::read` + `state::write_atomic` + `StateFile` + `StateReadError` + `StateWriteError` — call sites only (P002/P005/P007 lock) |
| `src/inbox.rs` | `inbox::read_inbox` + `inbox::insert_rows` + `inbox::write_atomic` + `InboxError` 3 variants — call sites only (P006/P008 lock) |
| `src/cli/parse_report.rs` | Unchanged from P004 |
| `src/cli/dedup.rs` | Unchanged from P005 |
| `src/cli/append.rs` | Unchanged from P006 |
| `src/cli/migrate_state.rs` | Unchanged from P007 |
| `src/cli/state_backfill.rs` | Unchanged from P008 |
| `src/cli/mod.rs` | `pub mod scan_and_append;` already registered (P001 ship) — verify Anchor #15 |
| `Cargo.toml` | NO new dep added — verify Anchor #11 |
| `docs/security/INVARIANTS.md` | INV-LOCAL-002 unchanged unless Worker decides to append "P009 fourth caller via inbox::write_atomic + state::write_atomic" (Tầng 2 self-decide; recommendation: skip per P008 precedent) |
| `tests/fixtures/agent-report-1.md` | Unchanged from P004 — used as composite report input |
| `tests/fixtures/inbox-baseline.md` | Unchanged from P006 — used as composite inbox input |

---

## Luật chơi (Constraints)

1. **No new Cargo deps.** Cargo.toml `[dependencies]` + `[dev-dependencies]` MUST remain identical post-P009. All needed crates (serde_json, chrono, anyhow, BTreeSet/HashSet from std, assert_cmd, predicates, tempfile) already present per P002-P008 chain.

2. **Composite uses underlying lib fns, NOT subcmd `run()`.** Worker MUST call `sentinel::extract_block`, `row::parse_row`, `state::read`, `inbox::read_inbox`, `inbox::insert_rows`, `inbox::write_atomic`, `state::write_atomic` directly. FORBIDDEN: calling `cli::parse_report::run`, `cli::dedup::run`, `cli::append::run` (each prints its own stdout JSON → 3 interleaved blobs).

3. **Write order: inbox FIRST, then state.** Worker MUST call `inbox::write_atomic` BEFORE `state::write_atomic`. Rationale per Architecture Decision #3 (locked): inbox-first is the safer failure mode (worst case = duplicate rows visible to Sếp; state-first worst case = silently-skipped advisories — security gap). Constraint enforced by code-inspection in Sub-mech F trace; Worker tests can also assert via mock failure injection (Tầng 2, not required).

4. **NOT cross-file atomic — document caveat in CHANGELOG.** P009 writes 2 files via 2 separate atomic writes. If state write fails after inbox write succeeded, state lacks IDs that inbox now contains. Recovery: `advisory-inbox state-backfill`. Worker MUST add caveat note to CHANGELOG entry. NO 2-phase commit / WAL / sentinel-file lock — out-of-scope (over-engineering).

5. **INV-LOCAL-002 atomic write enforced via existing lib fns.** Both writes go through `inbox::write_atomic` (P006) and `state::write_atomic` (P007). FORBIDDEN: `std::fs::write(inbox_path, ...)`, `OpenOptions::append(true)`, direct `std::fs::rename` in P009 code. Worker verifies Sub-mech F grep clean.

6. **Sub-mech C invariant — `seen_advisories` monotonic non-shrink.** `post.seen_advisories ⊇ pre.seen_advisories`. Test A asserts every pre-ID survives. Logic enforced by `BTreeSet::extend` semantics (union, never subtract).

7. **`last_scan_at` UPDATED to `Utc::now()`.** Composite IS a scan event. Worker MUST set `last_scan_at = chrono::Utc::now()` in the updated StateFile. Test A asserts `post.last_scan_at > pre.last_scan_at`. (Contrast with P008 state-backfill which preserves last_scan_at.)

8. **`agent_version` PRESERVED — no flag to update.** Worker MUST preserve `pre_state.agent_version`. No `--agent-version <STR>` CLI flag (Architectural Decision #5 — out-of-scope for MVP).

9. **`schema_version` PRESERVED == 1.** Worker MUST preserve `pre_state.schema_version` (which must be 1; `state::read` validates this). NEVER bump schema_version in P009 (schema migration is `migrate-state`'s domain).

10. **Empty sentinel block → exit 0 + `appended: 0`.** Treat as "no advisories found" — valid non-error outcome. State still bumps last_scan_at. Test C verifies.

11. **Missing sentinel markers → exit 1.** Per `sentinel::extract_block` contract (P003). Maps to ARCHITECTURE §1 spec "1..3 per subcmd error mapping".

12. **No `--dry-run` / `--force` / `--agent-version` / `--no-state-update` flags.** Out-of-scope. Hard Stop per RULES.md §12 (CLI surface change ngoài scope).

13. **No `unsafe { }` blocks.** INV-LOCAL-001 enforce. Escalate if tempted.

14. **Match-arm uniformity in main.rs.** Dispatch arm ends with `Ok(())` per P004 Turn 1 O1.1 precedent. All sibling arms have identical shape.

15. **`InboxError` exhaustive match MUST cover all 3 variants.** Per P008 Code Reality Map: `InboxError` now has `{ MissingRowsHeading, Io, ParseRow }`. P009 main.rs dispatch arm exhaustive match MUST include `ParseRow` arm (compile-time enforcement; runtime never triggers since scan-and-append doesn't call `parse_rows`).

16. **Voice: Vietnamese in phiếu body, English in code comments, English in CLI stdout/stderr.** Per CLAUDE.md language convention.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (≥62 tests total post-P009; baseline 59 from P008 + ≥3 new integration scan_and_append_cli)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (watch for `if_same_then_else` on dispatch arm per P008 SD-2 precedent)
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] Acceptance scenario: copy `tests/fixtures/agent-report-1.md` + `tests/fixtures/inbox-baseline.md` to tempdir + write minimal state JSON with 1 overlap ID → `cargo run --quiet -- scan-and-append --report <tmp-report> --inbox <tmp-inbox> --state <tmp-state>` → exit 0, stdout JSON `{"appended":K,"skipped_dedup":S,"total_open":M}` where K+S equals row count from agent-report-1.md. State post-condition: `seen_advisories` ⊇ pre + all observed_ids (sorted); `last_scan_at` BUMPED to Utc::now(); `agent_version` UNCHANGED; `schema_version == 1`. Inbox post-condition: K new rows at top of `## Rows`; baseline rows preserved.
- [ ] All-skipped: pre-populate state with ALL report IDs → `appended:0`, `skipped_dedup:<N>`, state count unchanged, last_scan_at bumped.
- [ ] Empty sentinel block: report markdown with `<!-- INBOX_APPEND_START -->\n<!-- INBOX_APPEND_END -->` → `appended:0`, `skipped_dedup:0`, exit 0, state last_scan_at bumped.
- [ ] Missing sentinel: report markdown without `INBOX_APPEND_START` marker → exit 1, stderr contains "sentinel" or "marker" or "missing", inbox + state UNCHANGED.
- [ ] Bad row format: report with malformed pipe-row (e.g., 5 cols instead of 8) inside sentinel → exit 2, stderr contains "row" or "parse" or "column", inbox + state UNCHANGED (failure happens before any write).
- [ ] State read fail: nonexistent state path → exit 1.
- [ ] Inbox missing `## Rows`: inbox file without heading → exit 1, state UNCHANGED (failure before write step 8).
- [ ] Stdin mode: `cat tests/fixtures/agent-report-1.md | cargo run --quiet -- scan-and-append --inbox <tmp-inbox> --state <tmp-state>` (no `--report` flag) → behaves identically to file mode.

### Regression
- [ ] `cargo test --test parse_report_cli` — 3 tests pass (P004 ship unchanged)
- [ ] `cargo test --test dedup_cli` — 4 tests pass (P005 ship unchanged)
- [ ] `cargo test --test append_cli` — 4 tests pass (P006 ship unchanged)
- [ ] `cargo test --test migrate_state_cli` — 5 tests pass (P007 ship unchanged)
- [ ] `cargo test --test state_backfill_cli` — 4 tests pass (P008 ship unchanged)
- [ ] `cargo run -- parse-report < tests/fixtures/agent-report-1.md` — unchanged behavior
- [ ] `cargo run -- state-backfill --state <fresh-path> --inbox <baseline> --dry-run` — unchanged behavior (no regression on `inbox::parse_rows` path)

### Docs Gate
- [ ] `docs/CHANGELOG.md` — entry P009 at top:
  - `scan-and-append` composite wired (compose sentinel → row::parse_row → dedup partition → inbox::insert_rows + state::write_atomic).
  - Atomicity caveat: NOT cross-file atomic; inbox-first write order; recovery = `advisory-inbox state-backfill`.
  - `state::write_atomic` is fourth caller (P006 inbox, P007 migrate, P008 backfill, P009 composite).
- [ ] `docs/ARCHITECTURE.md` §5 — P009 scaffold-status entry added (after P008 entry): `scan-and-append` wired; NO new lib module (composite reuses sentinel/row/state/inbox).
- [ ] `README.md` — `scan-and-append` quick-start section (conditional per Anchor #14; if section missing, add after `state-backfill` quick-start). Include 1 example invocation + JSON output shape + exit code table.
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P009.md` — full report written (anchors verified, Sub-mech B/C/F checks fired, sai lệch documented if any, Code Reality Map updated for P010+ Architect).
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `- 2026-MM-DD P009: scan-and-append composite shipped, 4th INV-LOCAL-002 caller, NOT cross-file atomic → see docs/discoveries/P009.md`
- [ ] Sub-mechanism B/C/D/E/F Verification Trace filled (table above)

### Lane assignment
- Classifier output: **Guarded** (composite of 3 file-write surfaces — writes BOTH inbox markdown AND state JSON; cross-file non-atomicity caveat; Sub-mech C invariant on seen_advisories; write-order discipline enforced).
- Reason files: `src/cli/scan_and_append.rs` (NEW composite write site), `src/main.rs` (5-family error exit-code map).
- Override: N/A (no override requested).
