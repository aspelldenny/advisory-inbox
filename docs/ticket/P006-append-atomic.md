# PHIẾU P006: append subcmd (atomic write)

> **ID format:** `P006` — counter `.phieu-counter` = 6 sau P005 ship.
> **Filename:** `docs/ticket/P006-append-atomic.md`
> **Branch:** `feat/P006-append-atomic`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P009 scan-and-append composite + P011 MCP `append` tool; cũng establishes atomic-write protocol that P007 migrate-state + P008 state-backfill + P009 sẽ tái sử dụng)
> **Ảnh hưởng:** `src/inbox.rs` (new module — `read_inbox` + `insert_rows` + `write_atomic` + `InboxError` + tests), `src/main.rs` (add `mod inbox;` + update `Commands::Append` dispatch arm exit-code map), `src/cli/append.rs` (stub → real impl), `src/cli/mod.rs` (verify `pub mod append;` registered — P001 ship), `src/row.rs` (add `impl Display for AdvisoryRow` rendering pipe-delim line per ARCHITECTURE §3), `tests/fixtures/inbox-baseline.md` (new), `tests/fixtures/rows-2.json` (new), `tests/append_cli.rs` (new integration test), `docs/ARCHITECTURE.md` §5 (mark `inbox.rs` module shipped + P006 scaffold-status entry), `docs/CHANGELOG.md` (entry P006), `docs/security/INVARIANTS.md` (review INV-LOCAL-002 — first concrete user; note inbox path is user-supplied), `README.md` (`append` quick-start nếu chưa cover — Worker check Anchor #14)
> **Dependency:** P001 (CLI scaffold + `Commands::Append` variant), P002 (`AdvisoryRow` 8 fields + `Status` + `Severity`), P004 (`RowsEnvelope` precedent + `serde_json::json!` pattern), P005 (`state::read` error-downcast → exit-code map pattern, anyhow downcast idiom) — all shipped 2026-05-28
> **Lane:** **Guarded** (filesystem persistence — write to user-supplied path outside `target/`; INV-LOCAL-002 atomic-write protocol applies; Sub-mech F runtime-state check required — no token leak in log/error wording)
> **Sub-mech áp dụng:** **B** (capability — `cargo check` + `cargo test inbox` + `cargo test --test append_cli`), **D** (persistence — ARCHITECTURE §5 + INVARIANTS §3 INV-LOCAL-002 + README cập nhật), **F** (runtime state — verify error messages KHÔNG echo file content / path content that could leak secrets; `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_'` clean across new code)

---

## Context

### Vấn đề hiện tại

P005 ship `dedup` → emits JSON `{ "kept": [...], "skipped": [...], "observed_ids": [...] }`. Bây giờ phải **wire `append` subcmd** để take `kept` rows + ghi vào `docs/security/advisory-inbox.md` (user-supplied) AT TOP của `## Rows` section.

Pipeline (ARCHITECTURE §1):
```
advisory-inbox append --inbox <FILE> --rows-json <FILE>
→ output: { "appended_count": N, "total_open": M }
→ exit:   0 success, 1 inbox missing `## Rows` heading, 2 write error
```

Behavior (ARCHITECTURE §3):
- Inbox markdown PHẢI có `## Rows` heading. Append inserts AFTER this line.
- Newest rows go to TOP (each new row immediately after heading).
- Pipe-delimited 8 col: `| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |`.
- Write MUST be atomic (INV-LOCAL-002: temp+fsync+rename pattern; partial-write corruption broken Sếp's review loop).

Stub hiện tại của `cli/append.rs` (P001) printf TODO. Sau P006:
- `cargo run -- append --inbox tests/fixtures/inbox-baseline.md --rows-json tests/fixtures/rows-2.json` → inbox modified with 2 new rows at top of `## Rows`, original rows preserved, JSON stdout `{ "appended_count": 2, "total_open": M }`, exit 0.
- Inbox missing `## Rows` heading → stderr, exit 1.
- Write error (parent dir not writable, disk full simulate) → stderr, exit 2.

Reference BACKLOG.md P006:
- Lane: Guarded (filesystem write).
- Scope: Wire `cli/append.rs` → read inbox markdown → insert rows after `## Rows` heading → atomic write temp+rename.
- Acceptance: Fixture inbox + 2 rows → write OK, rows at top of `## Rows` section, original rows preserved. Atomic write verified.
- Sub-mech checks: B, D, F (no token leak to logs).

INV-LOCAL-002 mandates atomic-write protocol — P006 is the FIRST concrete user; pattern must establish reference shape for P007/P008/P009/P011.

### Giải pháp

**5 unit công việc chính:**

1. **`src/inbox.rs` new module:**
   - `pub fn read_inbox(path: &Path) -> Result<String, InboxError>` — read inbox markdown to String. Io error → `InboxError::Io`.
   - `pub fn insert_rows(content: &str, rows: &[AdvisoryRow]) -> Result<(String, usize), InboxError>` — search for line `## Rows` (heading), insert rows IMMEDIATELY after heading line (newest at top). Returns `(new_content, total_open_count)`. Missing heading → `InboxError::MissingRowsHeading`. Empty rows slice → no-op clone (return original content + recount total_open).
   - `pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError>` — temp+fsync+persist per INV-LOCAL-002 (atomic-write protocol). `NamedTempFile::new_in(parent_dir)` (same filesystem → atomic rename), write all, `temp.as_file().sync_all()` (fsync data+metadata), `temp.persist(target)`. Io error → `InboxError::Io`.
   - `InboxError` enum (thiserror, 2 variant):
     - `MissingRowsHeading { path: PathBuf }` — exit code 1 contract.
     - `Io { path: PathBuf, source: std::io::Error }` — exit code 2 contract.
   - Unit tests (≥4):
     - `insert_rows` happy: baseline content with `## Rows` heading + 2 existing rows + insert 2 new rows → result content has 4 rows, new rows immediately after heading.
     - `insert_rows` missing heading → `MissingRowsHeading`.
     - `insert_rows` empty rows slice → no-op (content unchanged structurally; total_open count returned).
     - `write_atomic` round-trip: write content to temp path, read back, content equal (smoke test atomicity / file existence post-persist).
     - (optional 5th) `total_open` count correctness: insert 1 open + 1 dismissed → total_open count grows by 1 only.

2. **`src/row.rs` add `impl Display for AdvisoryRow`:**
   - Renders pipe-delim 8-col line per ARCHITECTURE §3: `| {date} | {advisory_id} | {source_url} | {package} | {file_line} | {severity} | {status} | {note} |`.
   - `date` format: `NaiveDate` Display = ISO `YYYY-MM-DD` (chrono default Display for NaiveDate matches ISO-8601 calendar date) — verify in Anchor.
   - `severity` Display = PascalCase variant name (per P002 serde `rename_all = "PascalCase"`; Display must match output of inbox markdown which uses PascalCase per ARCHITECTURE §3 Severity enum: `Critical/High/Medium/Low`).
   - `status` Display = lowercase variant name (`open`/`processed`/`dismissed` per P002 serde + ARCHITECTURE §3).
   - **KHÔNG implement `FromStr` reverse** — out-of-scope P006. (Reverse parsing covered by P008 state-backfill if needed.)
   - Unit test for `Display`: format one `AdvisoryRow` → expected string match.

3. **`src/cli/append.rs` real impl:**
   - `pub fn run(inbox: PathBuf, rows_json: PathBuf) -> anyhow::Result<()>`:
     1. Read rows JSON via `std::fs::read_to_string(&rows_json)` then `serde_json::from_str::<RowsEnvelope>(&txt)` where `RowsEnvelope { rows: Vec<AdvisoryRow> }` (same shape as P005 — Tầng 1 contract).
     2. Read inbox markdown via `inbox::read_inbox(&inbox)?`.
     3. Insert rows via `inbox::insert_rows(&content, &envelope.rows)?` → `(new_content, total_open)`.
     4. Atomic write via `inbox::write_atomic(&inbox, &new_content)?`.
     5. Emit stdout JSON `{ "appended_count": envelope.rows.len(), "total_open": total_open }` + trailing newline.
   - Errors propagate via anyhow; main.rs downcasts `InboxError` → exit code per variant.
   - **KHÔNG add `--dry-run`** flag. Out-of-scope.
   - **KHÔNG add `--backup`** flag. Out-of-scope (atomic write protocol provides crash-safety; backup is separate concern).

4. **`src/main.rs` dispatch arm — error → exit code map:**
   - Add `mod inbox;` declaration if absent (Anchor #6 verify).
   - Update `Commands::Append { inbox, rows_json }` dispatch arm:
     ```rust
     Commands::Append { inbox, rows_json } => {
         if let Err(e) = cli::append::run(inbox, rows_json) {
             let code = if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
                 match ie {
                     crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                     crate::inbox::InboxError::Io { .. } => 2,
                 }
             } else {
                 2  // rows JSON malformed / unreadable / serde err → exit 2 (per ARCHITECTURE §1 dedup precedent extended)
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```
   - Tail `Ok(())` REQUIRED (P004 Turn 1 O1.1 precedent — match-arm uniformity).

5. **Fixtures + integration test:**
   - `tests/fixtures/inbox-baseline.md` — markdown với 2 existing rows under `## Rows`. Includes the HTML-comment placeholder per ARCHITECTURE §3 example to ensure parser/insert skips it correctly (placeholder block stays intact, NOT modified by insert).
   - `tests/fixtures/rows-2.json` — `{ "rows": [...] }` envelope with 2 new rows (distinct advisory_id from baseline so test asserts both old + new survive).
   - `tests/append_cli.rs`:
     - Happy: copy baseline to tempdir, run append, assert: new rows present at top of `## Rows` section, old 2 rows preserved, exit 0, stdout `appended_count: 2` + `total_open: 4`.
     - Missing heading: tempfile without `## Rows` → exit 1, stderr contains "rows" or "heading".
     - Rows JSON malformed (inline tempfile without `rows` key) → exit 2, stderr contains "rows" or "JSON".
     - Atomic write smoke: write OK, file exists at expected path, content not empty, no `.tmp` leftover (NamedTempFile cleanup on drop after persist).

#### Why new module `src/inbox.rs` (vs inline in `cli/append.rs`)?

ARCHITECTURE §5 declares `src/inbox.rs` as the home for inbox markdown parser+writer. P006 first concretely needs that module. Future P008 state-backfill needs `inbox` module to read inbox + extract IDs (reverse parse). P009 scan-and-append composes it. P011 MCP tool `append` re-uses it. Module boundary set NOW prevents repeat refactor later.

Trade-off: small upfront cost (new file, new pub API) vs future amortization. Decision: ship module per architecture plan.

#### Why `InboxError` enum (vs raw `anyhow`)?

Same reasoning as P005 `StateReadError`: distinct exit codes (`MissingRowsHeading` → 1, `Io` → 2) require concrete type for `e.downcast_ref::<InboxError>()` in main.rs. Pattern matches P004 `SentinelError` + P005 `StateReadError`. Establishes consistent shape for P007 `MigrateError`, P008 `BackfillError`, etc.

```rust
#[derive(Error, Debug)]
pub enum InboxError {
    #[error("inbox `{path}` is missing `## Rows` heading — cannot determine insert position")]
    MissingRowsHeading { path: PathBuf },
    #[error("inbox `{path}` I/O failure: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
```

2 variant only (no `Json` variant — rows JSON parse happens in `cli/append.rs` not inbox.rs; `anyhow` bubbles serde error, falls through to else-branch exit 2). This keeps `InboxError` strictly about *inbox file* errors.

#### Why `Vec<AdvisoryRow>` + `Display` (vs hand-format pipe line)?

INV-LOCAL-003: "JSON serialization via serde_json (no manual)". Same spirit applies to inbox row format: hand `format!("| {date} |...")` lines scattered = drift risk if `AdvisoryRow` adds/renames field. Centralize in `impl Display for AdvisoryRow` → one source of truth per ARCHITECTURE §3 column order.

Note: This is NOT JSON; it's the pipe-delimited markdown row. INV-LOCAL-003 is JSON-specific. But the principle (single render path) applies. Worker writes `impl Display` exactly per ARCHITECTURE §3 col order.

#### Insert algorithm — line-scan, not regex

Trade-off:
- **A — line-scan for first line that equals `## Rows` (after trim_end):** simple, O(N), no dep. **Chọn.**
- **B — regex `(?m)^## Rows$`:** correct but overkill; `regex` crate (Cargo.toml line 22) already in deps, but trivial substring match doesn't need it.
- **C — markdown AST parse (pulldown-cmark):** new dep, massive overkill for one heading lookup. Reject.

Algorithm:
```rust
fn insert_rows(content: &str, rows: &[AdvisoryRow]) -> Result<(String, usize), InboxError> {
    // 1. Split into lines (preserve line endings via iter, but join with '\n' after — assume Unix LF).
    let lines: Vec<&str> = content.lines().collect();
    // 2. Find index of first line equal to "## Rows" after trim_end (tolerate trailing whitespace).
    let heading_idx = lines.iter().position(|l| l.trim_end() == "## Rows")
        .ok_or_else(|| InboxError::MissingRowsHeading { path: PathBuf::new() /* caller fills */ })?;
    // 3. Build new content: lines[0..=heading_idx] + (optional blank?) + new rows + lines[heading_idx+1..].
    // 4. Newest at top: rows in `rows` slice — insert in order? Spec says newest at top — for `rows: [A, B]`, output order is A then B (or B then A?).
    //    Decision: insert rows IN ORDER given (caller controls). Each row immediately after heading; iter rows.rev() so final order matches input slice order (first input row ends up TOPMOST).
    //    Wait — re-read spec: "Each new row goes immediately after heading" — so iter.rev() puts last input row at top, first input row below. Use iter() forward instead → first input row at top.
    //    Phiếu spec: rows iter forward; first `rows[0]` ends up topmost. Existing rows shifted down. Document this in inbox.rs.
    // 5. Count total_open: scan ALL rows in final content (existing parsed + new), count `status == "open"`.
    //    Simpler: count `| open |` substring occurrences in pipe-delim line. Worker Display impl renders `open` lowercase per ARCHITECTURE §3 status enum.
    //    Edge: HTML comment block `<!-- ... -->` may contain placeholder row with `| open |` — must skip. Implementation: only count lines OUTSIDE HTML comment blocks AND inside `## Rows` section (after heading, before next `##` heading or EOF).
    //    Simplest: count lines matching pattern `^\| .* \| open \|` outside comment blocks. Worker self-tests against fixture.
}
```

**Insert order decision (locked):** `rows[0]` ends up TOPMOST in output. Existing rows shifted down (preserved in their original relative order). This matches the convention "newest goes to top" when caller emits rows newest-first.

**`total_open` count algorithm:** scan inbox content AFTER insert, count rows where status column = `open`. Skip HTML comment blocks (`<!-- ... -->`) — per ARCHITECTURE §3 parser rules. Heuristic OK at MVP: substring match `| open |` outside `<!--` ... `-->` ranges. Worker may use simple state-machine line scanner.

#### Atomic write protocol — INV-LOCAL-002 reference shape

Per INVARIANTS §3 INV-LOCAL-002 (re-read by Architect during DRAFT load context):
```rust
let parent = target.parent().context("target has no parent dir")?;
let mut temp = NamedTempFile::new_in(parent)?;  // SAME filesystem → atomic rename
temp.write_all(content.as_bytes())?;
temp.as_file().sync_all()?;  // fsync data + metadata
temp.persist(target)?;  // atomic rename
Ok(())
```

**Key constraints (INV-LOCAL-002):**
- `NamedTempFile::new_in(parent)` REQUIRED (NOT `NamedTempFile::new()` — default `$TMPDIR` may be on different filesystem → cross-fs rename = copy+delete, loses atomicity).
- `sync_all()` REQUIRED before `persist` (fsync data + metadata; without it, kernel may reorder write+rename → partial visibility on crash).
- `persist(target)` atomic-replaces target.
- FORBIDDEN: `OpenOptions::append(true)` direct, `std::fs::write` direct (no temp), `std::fs::rename` outside `tempfile::persist`.

`NamedTempFile::as_file()` returns `&File` — `std::fs::File::sync_all(&self)` available. Anchor #9 marks `[needs Worker verify via cargo doc tempfile]` for the exact `as_file()` method name; if name differs (e.g., `file()` / deref auto-deref), Worker adjusts (Tầng 2 self-decide).

**Error mapping:** `NamedTempFile::new_in` Io error → `InboxError::Io`. `write_all` Io error → `InboxError::Io`. `sync_all` Io error → `InboxError::Io`. `persist` returns `Result<File, PersistError>` where `PersistError` is `tempfile::PersistError { error: std::io::Error, file: NamedTempFile }` — extract `.error` into `InboxError::Io`.

#### Sub-mech F — no token leak in logs

Concern: error messages echo file path / line content into stderr. If user runs `append --inbox /path/with-ghp_TOKEN-in-filename.md`, the error wording would leak. **Decision:** error messages reference the path supplied (path itself may contain a token but Sếp typed it knowingly), do NOT echo file CONTENT into stderr/log. Worker verifies during EXECUTE:
```bash
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/inbox.rs src/cli/append.rs
# expect: 0 hits (no hardcoded token literal anywhere in code)
```
Plus runtime check (`scripts/session-start-banner.sh` already covers env keys + git config).

#### Why no concurrency lock (multi-process safety)?

ARCHITECTURE §10 explicitly defers concurrency lock to future. P006 atomic write protects against PARTIAL write corruption (crash mid-write). It does NOT protect against TWO processes concurrently appending (last-writer-wins, may lose rows). MVP scope: single user, single agent process, no concurrent append. Deferred per BACKLOG (Phase 2 future surface).

### Scope

- CHỈ tạo: `src/inbox.rs` (new module file).
- CHỈ sửa: `src/cli/append.rs` (stub → real impl), `src/main.rs` (add `mod inbox;` + update `Commands::Append` dispatch arm), `src/row.rs` (add `impl Display for AdvisoryRow` + 1 unit test).
- CHỈ tạo fixtures: `tests/fixtures/inbox-baseline.md`, `tests/fixtures/rows-2.json`.
- CHỈ tạo integration: `tests/append_cli.rs`.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (mark `inbox.rs` shipped, P006 scaffold-status entry), `docs/CHANGELOG.md` (P006 entry), `docs/security/INVARIANTS.md` (INV-LOCAL-002 — note "first concrete user: P006; inbox path is user-supplied"), `README.md` (`append` quick-start nếu chưa cover; Worker check Anchor #14 conditional).
- KHÔNG sửa: `src/state.rs` (P002/P005 lock, P006 không touch state file), `src/sentinel.rs` (P003 lock), `src/cli/parse_report.rs` (P004 lock), `src/cli/dedup.rs` (P005 lock), `Cargo.toml` (no new dep — `tempfile = "3"` đã có dòng 21, `thiserror = "2"` dòng 20, `serde_json = "1"` dòng 16, `chrono` dòng 17, `anyhow = "1"` dòng 19; Worker verify Anchor #7/#8/#9).
- KHÔNG tạo: `src/error.rs` (ARCHITECTURE §5 pending — P006 không scope).
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 append: 0 success, 1 missing heading, 2 write error).
- KHÔNG đổi inbox markdown format (sentinel markers, row column order) — INV-LOCAL-005 protected.
- KHÔNG đổi `AdvisoryRow` shape (P002 lock) — chỉ ADD `impl Display`.
- KHÔNG đổi `RowsEnvelope` shape (P005 lock, defined inline in `cli/dedup.rs`) — P006 redefines envelope INLINE in `cli/append.rs` (duplicate definition acceptable per P005 Discovery Follow-up: "Nếu future P009 cần re-use → move lên `cli/mod.rs` hoặc `row.rs` — out-of-scope P005"). P006 honors this scope discipline: do NOT move/share `RowsEnvelope` in P006 either — Worker defines local `RowsEnvelope` in `cli/append.rs` same shape `{ rows: Vec<AdvisoryRow> }`.
- KHÔNG add `--dry-run` / `--backup` / `--force` flags.
- KHÔNG xoá HTML comment block `<!--` ... `-->` từ baseline fixture (placeholder example per ARCHITECTURE §3 — must remain unchanged after insert).
- KHÔNG implement `FromStr` for `AdvisoryRow` (reverse parse out-of-scope; P008 state-backfill domain).
- KHÔNG xoá `#![allow(dead_code)]` từ `sentinel.rs` (P004 Discovery follow-up; cross-phiếu housekeeping; NOT P006 scope).

### Skills consulted

Architect Read `docs/ticket/P005-dedup.md` để tham khảo:
- Pattern `RowsEnvelope` wrapper struct + `serde_json::from_str` inline deserialize.
- anyhow downcast → exit code map idiom (main.rs dispatch arm shape).
- Integration test idiom: `assert_cmd::Command::cargo_bin` + `predicates::str::contains` substring match.
- `serde_json::json!` macro emits keys alphabetical (`appended_count` → `total_open` order in actual stdout — test must use substring assert).

Architect Read `docs/discoveries/P004.md` + `docs/discoveries/P005.md` để học:
- Test count baseline post-P005: 31 tests (24 unit + 7 integration). P006 target: ≥31 + ≥5 new unit (inbox 4 + row Display 1) + ≥4 new integration = 40+ tests.
- `#![allow(dead_code)]` removal pattern only when wire-in complete (P006 does NOT introduce new `#![allow(dead_code)]`; new `inbox.rs` module has immediate consumer `cli/append.rs` so no need).
- `serde_json::json!` key alphabetical ordering carries forward.

Architect Read `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 in full — atomic write protocol spec re-confirmed. P006 IS first concrete user → reference shape locked here.

Architect verified `tempfile` 3 API via context7 query (`/stebalien/tempfile`): `NamedTempFile::new_in(directory)` confirmed (creates temp in specified dir); `NamedTempFile::persist(target)` confirmed (atomic move replacing target). Documentation does NOT explicitly show `as_file().sync_all()` chain — Anchor #9 marks `[needs Worker verify via cargo doc tempfile]` for that specific method chain. Fallback: if `as_file()` not available, try `temp.flush()?` then call `tempfile`'s into-inner pattern; Worker self-decides Tầng 2 if exact method signature differs.

Architect did NOT use context7 for `serde_json`, `chrono`, `anyhow`, `thiserror` (well-known APIs already exercised in P002-P005).

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/append.rs` hiện là stub printf TODO (P001 ship). Signature có thể đã là `pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()>` per P001 scaffold pattern (analogous P004/P005 stub shape). | P004 Anchor #1 + P005 Anchor #1 (both confirmed stub signature pattern) | `[needs Worker verify]` | ✅ Confirmed: stub `pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()>` with printf TODO. |
| 2 | `src/main.rs` có `Commands::Append { inbox: PathBuf, rows_json: PathBuf }` clap variant + existing dispatch arm `cli::append::run(inbox, rows_json)`. P001 ship + P004/P005 confirmed all 8 dispatch arms present at lines ~25-65. | P005 Anchor #6 confirmed all dispatch arms; P004 same. | `[needs Worker verify]` | ✅ Confirmed: variant at lines 40-47; dispatch arm at line 124. |
| 3 | `Commands::Append` clap variant declares `--inbox <FILE>` + `--rows-json <FILE>` REQUIRED (kebab-case `--rows-json` matches ARCHITECTURE §1). | ARCHITECTURE §1 append subcmd block dòng 46-53 + P001 scaffold ship | `[needs Worker verify]` | ✅ Confirmed: both flags REQUIRED per `cargo run -- append --help` output. |
| 4 | `src/row.rs` exports `pub struct AdvisoryRow` 8 fields: `date: NaiveDate`, `advisory_id: String`, `source_url: String`, `package: String`, `file_line: String`, `severity: Severity`, `status: Status`, `note: String`. + `Status` enum (Open/Processed/Dismissed serde lowercase) + `Severity` enum (Critical/High/Medium/Low serde PascalCase). | P002 phiếu + P004 Anchor #2 + P005 Anchor #5 + P004 Discovery Anchor #2 ("row.rs:14-54") | `[verified]` | ✅ Pre-verified transitively (P002→P004→P005 chain unchanged). |
| 5 | `src/row.rs` đã có `Serialize + Deserialize` derive trên `AdvisoryRow` (P002 ship). `Status` + `Severity` cũng derive Serialize+Deserialize. | P004 Discovery Anchor #2 | `[verified]` | ✅ Pre-verified. |
| 6 | `src/row.rs` chưa có `impl Display for AdvisoryRow` (P002/P004 không scope). | P002 phiếu Scope + P004 Discovery "row.rs unchanged post-P004" | `[unverified]` | ✅ Confirmed: 0 hits — no Display impl exists yet. |
| 7 | `Cargo.toml` `[dependencies]` có `tempfile = "3"` (line 21 — Architect Read directly), `thiserror = "2"` (line 20), `serde_json = "1"` (line 16), `chrono` (line 17), `anyhow = "1"` (line 19), `serde` derive (line 15). | Architect Read Cargo.toml dòng 13-23 trong load context | `[verified]` | ✅ Confirmed all 6 deps present at expected lines. |
| 8 | `Cargo.toml` `[dev-dependencies]` có `assert_cmd = "2"` (line 27) + `predicates = "3"` (line 28). | Architect Read Cargo.toml | `[verified]` | ✅ Confirmed lines 27-28. |
| 9 | `tempfile::NamedTempFile::new_in(dir: &Path)` + `temp.write_all(&[u8])` + `temp.as_file()` returning `&std::fs::File` + `file.sync_all()` + `temp.persist(target)` returning `Result<File, PersistError>` API exists in `tempfile = "3"`. | Architect context7 `/stebalien/tempfile` query: `new_in` confirmed; `persist` confirmed; `as_file().sync_all()` chain NOT explicit in surfaced docs → flag `[needs Worker verify via cargo doc tempfile]`. | `[needs Worker verify]` | ✅ Confirmed: `pub fn as_file(&self) -> &F` exists in tempfile-3.27.0/src/file/mod.rs. Returns `&std::fs::File`. Chain `temp.as_file().sync_all()` valid. |
| 10 | `chrono::NaiveDate` Display impl = ISO calendar date `YYYY-MM-DD` (e.g., `2026-05-28`). | P004 Anchor #6 (chrono NaiveDate::parse_from_str confirmed via context7); Display is round-trip with parse `%Y-%m-%d`. | `[unverified]` | ✅ Confirmed: `NaiveDate::Display` delegates to `Debug` which formats `YYYY-MM-DD` using `write_hundreds` — verified in chrono 0.4 source. |
| 11 | `tests/fixtures/` directory tồn tại sau P004/P005 ship (`tests/fixtures/agent-report-1.md`, `state-3ids.json`, `rows-5.json` present). | P005 Anchor #11 + P004 Discovery Anchor #11 | `[verified]` | ✅ Pre-verified. |
| 12 | `tests/` directory tồn tại sau P004/P005 ship. | P005 Anchor #12 | `[verified]` | ✅ Pre-verified. |
| 13 | `src/main.rs` đã import `use std::path::PathBuf;` (P004 + P005 confirmed). | P005 Task 3 note ("already imported sau P004") | `[verified]` | ✅ Transitive. |
| 14 | `README.md` chưa có `append` quick-start (P004 covered parse-report, P005 covered dedup; append untouched). | P005 Discovery — README updated for dedup only, append likely still stub | `[unverified]` | ✅ Confirmed partial: `append` in binary description only (line 3). No dedicated `append` quick-start section exists. Task 6.5.4 MUST add. |
| 15 | `docs/ARCHITECTURE.md` §1 append subcmd block document I/O contract đúng (input `--inbox` + `--rows-json`, output JSON 2 key `appended_count`/`total_open`, exit 0/1/2). | Architect Read ARCHITECTURE.md dòng 44-53 trong load context | `[verified]` | ✅ Dòng 44-53: subcmd spec exact match. No drift. |
| 16 | `docs/ARCHITECTURE.md` §3 inbox format spec: `## Rows` heading mandatory, pipe-delim 8 col `Date / Advisory ID / Source / Package / File:Line / Severity / Status / Note`, HTML comment placeholder skipped, status `open/processed/dismissed`, severity `Critical/High/Medium/Low`. | Architect Read ARCHITECTURE.md dòng 154-180 | `[verified]` | ✅ Confirmed §3 dòng 162-180. |
| 17 | `docs/ARCHITECTURE.md` §7 Atomic Write Pattern dòng 280-298 mô tả `NamedTempFile::new_in(parent)` + `write_all` + `flush` + `persist`. (Note: §7 omits `sync_all()` — INVARIANTS §3 INV-LOCAL-002 ADDS `sync_all` explicit. INV is stricter — Worker follows INV not §7. Architect flags doc drift for follow-up.) | Architect Read ARCHITECTURE.md §7 + INVARIANTS §3 INV-LOCAL-002 | `[verified]` | ✅ Both read. Drift noted: §7 says `flush`, INV-LOCAL-002 says `sync_all`. Worker follows INV (stricter). Discovery Report tracks `ARCHITECTURE.md §7 should align with INV-LOCAL-002` as follow-up. |
| 18 | `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 mandates `NamedTempFile::new_in(target.parent()?)` + `temp.write_all(content)` + `temp.as_file().sync_all()` + `temp.persist(target)`. FORBIDDEN: `OpenOptions::append(true)` + `std::fs::write` direct + `std::fs::rename` outside `tempfile::persist`. | Architect Read INVARIANTS.md dòng 98-115 | `[verified]` | ✅ Confirmed dòng 100-115. P006 inbox.rs `write_atomic` MUST follow exactly. |
| 19 | `src/cli/mod.rs` đã có `pub mod append;` (P001 scaffold ship 8 subcmd modules). | P001 Discovery + P004/P005 implicit verification (their dispatch arm compiled = mod registered) | `[unverified]` | ✅ Confirmed: `pub mod append;` at cli/mod.rs:7. |
| 20 | `src/main.rs` chưa có `mod inbox;` declaration (P006 is first to introduce inbox module). | ARCHITECTURE §5 "Pending Phase 1+ phiếu: inbox.rs" | `[unverified]` | ✅ Confirmed: 0 hits — `mod inbox;` not yet declared. Worker adds after `mod cli;` in alphabetical order. |

**Note:** Worker EXECUTE Task 0 chạy ~10 grep/check verify (Anchors #1/#2/#3/#6/#9/#10/#14/#19/#20 → 9 anchors `[needs Worker verify]` + `[unverified]`). Nếu BẤT KỲ anchor nào lệch giả định → STOP escalate Discovery Report.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. 1 doc drift đã document tại Anchor #17 (ARCHITECTURE §7 mềm hơn INV-LOCAL-002 — Worker theo INV).

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND).
> Sếp đọc lúc nghiệm thu. Cap = 3 turns.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no Tầng 1 challenges.** All anchors verified against real code 2026-05-28.

**Anchor verification:**

| # | Result | Notes |
|---|--------|-------|
| 1 | ✅ | `src/cli/append.rs` is stub `pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()>` with printf TODO — exact P004/P005 pattern. |
| 2 | ✅ | `Commands::Append { inbox, rows_json }` at main.rs:40-47. Dispatch arm at line 124 is flat passthrough `cli::append::run(inbox, rows_json)` — ready to replace with error-aware arm per Task 4. |
| 3 | ✅ | `--inbox <INBOX>` + `--rows-json <ROWS_JSON>` both REQUIRED, confirmed via `cargo run -- append --help`. |
| 4 | ✅ | Pre-verified (transitive P002→P004→P005 chain). |
| 5 | ✅ | Pre-verified (transitive). |
| 6 | ✅ | 0 hits for `impl.*Display\|impl Display` in `src/row.rs` — no existing Display impl. |
| 7 | ✅ | All 6 deps confirmed in Cargo.toml: `tempfile = "3"`, `thiserror = "2"`, `serde_json = "1"`, `chrono = "0.4"`, `anyhow = "1"`, `serde = "1"`. |
| 8 | ✅ | `assert_cmd = "2"` + `predicates = "3"` in `[dev-dependencies]`. |
| 9 | ✅ | `tempfile` 3.27.0 installed. `as_file()` confirmed in source: `pub fn as_file(&self) -> &F` at `tempfile-3.27.0/src/file/mod.rs`. Returns `&std::fs::File` → `sync_all()` available. Full chain `temp.as_file().sync_all()` compiles. |
| 10 | ✅ | `NaiveDate::Display` delegates to `Debug` which uses `write_hundreds` pattern producing `YYYY-MM-DD`. Confirmed from chrono source. |
| 11 | ✅ | Pre-verified (P005 confirmed). |
| 12 | ✅ | Pre-verified (P005 confirmed). |
| 13 | ✅ | `use std::path::PathBuf;` at main.rs:8. |
| 14 | ✅ (partial) | `append` appears at README line 3 (binary description) but NO dedicated `append` quick-start section exists. Task 6.5.4 MUST add it. |
| 15 | ✅ | Pre-verified (ARCHITECTURE §1 append block confirmed). |
| 16 | ✅ | Pre-verified (ARCHITECTURE §3 inbox format confirmed). |
| 17 | ✅ | Drift acknowledged: ARCHITECTURE §7 says `flush`; INVARIANTS §3 INV-LOCAL-002 says `sync_all`. Worker follows INV (stricter). |
| 18 | ✅ | Pre-verified (INVARIANTS §3 INV-LOCAL-002 confirmed exact protocol). |
| 19 | ✅ | `pub mod append;` at `src/cli/mod.rs:7`. |
| 20 | ✅ | 0 hits for `^mod inbox\|mod inbox;` in `src/main.rs` — `mod inbox;` not yet declared. Worker adds in Task 4. |

**Baseline test count [mechanical]:** Phiếu states 31 tests (P005 baseline). Actual count = 31 (24 unit + 4 dedup integration + 3 parse_report integration). `wc -l` on `--list` output gave 37 because it counts blank lines and summary rows. No discrepancy — phiếu is correct.

**Sub-mech F preflight:** `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml` → 0 hits (clean).

**No Tầng 1 objections.** Phiếu V1 assumptions all match code reality. Ready for EXECUTE.

**Status:** ✅ WORKER CHALLENGE ACCEPTED — V1 approved, proceed to EXECUTE

### Turn 1 — Architect Response
*(Architect fill khi invoked RESPOND mode.)*

- [O1.1] → ACCEPT / DEFEND / REFRAME (Tầng 2) / DEFER TO SẾP → action taken
- [O1.2] → …

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V<N>
- Total turns: <count>
- Approved (autonomous narrate or Sếp gate): [date] — code execution may begin

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
| B (capability) | `cargo test inbox` | ≥4 unit tests pass (insert happy / missing heading / empty rows / write_atomic round-trip) | | |
| B (capability) | `cargo test row::tests::display` (or similar) | ≥1 unit test for `AdvisoryRow` Display impl | | |
| B (capability) | `cargo test --test append_cli` | ≥3 integration tests pass (happy / missing heading / rows malformed) | | |
| B (capability) | `cargo run --quiet -- append --inbox tests/fixtures/inbox-baseline.md --rows-json tests/fixtures/rows-2.json` (after copying baseline to tempdir) | stdout JSON `appended_count: 2`, `total_open: M`, exit 0; tempdir inbox modified with 2 new rows at top of `## Rows`, original 2 rows preserved | | |
| C (state schema) | (no state schema change in P006) | N/A | | N/A |
| D (persistence) | `grep -l "append" docs/ARCHITECTURE.md` | ≥1 hit (§1 append subcmd + §5 scaffold-status entry post-P006) | | |
| D (persistence) | `grep -l "inbox.rs\|InboxError\|InboxAtomicWrite" docs/ARCHITECTURE.md` | ≥1 hit (§5 module table) | | |
| D (persistence) | `grep -n "P006\|inbox.rs" docs/security/INVARIANTS.md` | ≥1 hit (INV-LOCAL-002 cross-ref note added) | | |
| D (persistence) | `grep -n "append" README.md` | ≥1 hit (quick-start section per Anchor #14 conditional) | | |
| E (env drift) | `cargo update --dry-run` | no surprise bump | | |
| E (env drift) | `cargo build --release` from clean target | exit 0, 0 warnings | | |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/inbox.rs src/cli/append.rs src/row.rs` | 0 hits | | |
| F (runtime state) | `bash scripts/session-start-banner.sh` | no forbidden key detected | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + D + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code.

**Lệnh chạy (verify Anchor #1, #2, #3, #6, #9, #10, #14, #19, #20):**

```bash
# Anchor #1 — append stub state
cat src/cli/append.rs

# Anchor #2 — main.rs Append variant + dispatch
grep -n "Append" src/main.rs

# Anchor #3 — append clap help
cargo run --quiet -- append --help 2>&1 | head -20

# Anchor #6 — no existing Display impl on AdvisoryRow
grep -n "impl.*Display\|impl Display" src/row.rs

# Anchor #9 — tempfile API verify (cargo doc preferred; fallback: write test stub + cargo check)
# Option A: open docs locally (Worker manual scan)
# cargo doc --no-deps -p tempfile  # generates docs/, Worker opens NamedTempFile page
# Option B: compile-test the chain in a throwaway file
cat > /tmp/p006-tempfile-probe.rs <<'EOF'
use std::io::Write;
use tempfile::NamedTempFile;
fn _probe() -> std::io::Result<()> {
    let mut t = NamedTempFile::new_in(".")?;
    t.write_all(b"x")?;
    t.as_file().sync_all()?;
    let _p = t.persist("/tmp/x")?;
    Ok(())
}
EOF
# (Don't compile — just verify the chain shape; if Worker doubt → cargo doc inspect)

# Anchor #10 — chrono NaiveDate Display = "YYYY-MM-DD"
# Inline in row.rs Display impl unit test (Task 2)

# Anchor #14 — README append coverage
grep -n "append" README.md

# Anchor #19 — cli/mod.rs registers append
grep -n "pub mod append" src/cli/mod.rs

# Anchor #20 — main.rs no existing mod inbox
grep -n "^mod inbox\|mod inbox;" src/main.rs

# Sub-mech F preflight
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml || echo "clean"

# Baseline test count
cargo test --all -- --list 2>/dev/null | wc -l
```

**Output:** Worker fill kết quả vào Debate Log Turn 1.

**Hard Stop triggers:**
- Anchor #2 — nếu `Commands::Append` không tồn tại HOẶC field naming khác (`inbox: PathBuf` + `rows_json: PathBuf`) → STOP escalate (P001 drift).
- Anchor #3 — nếu `--inbox` hoặc `--rows-json` không phải REQUIRED → STOP escalate (drift ARCHITECTURE §1).
- Anchor #4/#5 — nếu `AdvisoryRow` shape / `Status` / `Severity` variants/serde khác P002 spec → STOP escalate (P002 drift; rất unlikely vì pre-verified).
- Anchor #9 — nếu `as_file().sync_all()` chain không compile → Worker tries fallback `temp.flush()` only + log Discovery (Tầng 2 self-decide, OK; vẫn atomic via persist nhưng less crash-safe).
- Anchor #19 — nếu `pub mod append;` MISSING từ `cli/mod.rs` → STOP escalate (P001 drift; P004/P005 cũng đã verify implicitly).
- Anchor #20 — nếu `mod inbox;` ALREADY trong main.rs → STOP escalate (unexpected — P006 first to introduce).

### Task 1: Tạo `src/inbox.rs` — module mới

**File:** `src/inbox.rs` (NEW)

**Mục tiêu:**
1. Define `InboxError` enum (2 variant: `MissingRowsHeading`, `Io`).
2. Implement `read_inbox(path)` → `Result<String, InboxError>`.
3. Implement `insert_rows(content, rows)` → `Result<(String, usize), InboxError>`.
4. Implement `write_atomic(path, content)` → `Result<(), InboxError>` per INV-LOCAL-002.
5. Add ≥4 unit tests.

**Skeleton:**

```rust
//! Inbox markdown parser + writer for advisory-inbox.
//!
//! See `docs/ARCHITECTURE.md` §3 for the inbox markdown format,
//! and §7 + `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 for the
//! atomic-write protocol (this module is the first concrete user).

use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use thiserror::Error;

use crate::row::AdvisoryRow;

/// Errors raised by inbox read/parse/write operations.
///
/// Exit-code mapping (caller's responsibility in `main.rs`):
/// - [`InboxError::MissingRowsHeading`] → exit code 1 (per ARCHITECTURE §1 append).
/// - [`InboxError::Io`] → exit code 2.
#[derive(Error, Debug)]
pub enum InboxError {
    #[error("inbox `{path}` is missing `## Rows` heading — cannot determine insert position")]
    MissingRowsHeading { path: PathBuf },
    #[error("inbox `{path}` I/O failure: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Read the inbox markdown file at `path` into a String.
pub fn read_inbox(path: &Path) -> Result<String, InboxError> {
    std::fs::read_to_string(path).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Insert `rows` after the `## Rows` heading line. Returns the new content
/// and the total count of `status == open` rows in the resulting inbox.
///
/// Order: `rows[0]` ends up TOPMOST. Existing rows are preserved in their
/// original relative order (shifted down). Caller emits rows newest-first
/// to maintain the inbox's "newest at top" invariant.
///
/// Errors with [`InboxError::MissingRowsHeading`] if no line equal to
/// `## Rows` (after `trim_end`) is found in `content`.
///
/// Empty `rows` slice → no-op: returns original content + recounts total_open.
pub fn insert_rows(
    content: &str,
    rows: &[AdvisoryRow],
) -> Result<(String, usize), InboxError> {
    // 1. Split content into lines.
    let lines: Vec<&str> = content.lines().collect();

    // 2. Find `## Rows` heading.
    let heading_idx = lines
        .iter()
        .position(|l| l.trim_end() == "## Rows")
        .ok_or_else(|| InboxError::MissingRowsHeading {
            path: PathBuf::new(), // caller (cli/append.rs) replaces with real path before bubble-up if needed; OK to leave empty for unit tests
        })?;

    // 3. Build output: lines[0..=heading_idx] + new rows (forward order — rows[0] topmost) + lines[heading_idx+1..]
    let mut out = String::with_capacity(content.len() + rows.len() * 200);
    for line in &lines[..=heading_idx] {
        out.push_str(line);
        out.push('\n');
    }
    for row in rows {
        // `impl Display for AdvisoryRow` emits the pipe-delim 8-col line (per ARCHITECTURE §3).
        out.push_str(&row.to_string());
        out.push('\n');
    }
    for line in &lines[heading_idx + 1..] {
        out.push_str(line);
        out.push('\n');
    }

    // Preserve trailing newline if original had one; `lines()` strips it.
    // If content didn't end with newline, the loop above added one extra — trim it.
    if !content.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    // 4. Count total_open: scan output, count pipe-delim rows with status=open outside HTML comments.
    let total_open = count_open_rows(&out);

    Ok((out, total_open))
}

/// Count rows with `status == open` in the inbox content. Skips HTML comment
/// blocks (`<!-- ... -->`) per ARCHITECTURE §3 parser rules.
///
/// Implementation: line-by-line scan with a simple `in_comment_block` flag.
/// A line counts as an "open row" if (a) we are NOT in a comment block,
/// and (b) the line matches the heuristic `| ... | open | ... |` (pipe-delim
/// with `open` as the 7th column).
fn count_open_rows(content: &str) -> usize {
    let mut in_comment = false;
    let mut count = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<!--") && !trimmed.contains("-->") {
            in_comment = true;
            continue;
        }
        if in_comment {
            if trimmed.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        // Substring match: pipe-delim line with ` | open | ` as 7th column.
        // Worker may tighten via split('|') + nth(7) trim if false-positives observed.
        if line.contains("| open |") {
            count += 1;
        }
    }
    count
}

/// Atomically write `content` to `path` per INV-LOCAL-002 protocol:
/// temp file in SAME parent directory → fsync data+metadata → atomic rename.
///
/// Forbidden alternatives (INV-LOCAL-002): `OpenOptions::append`,
/// `std::fs::write` direct, `std::fs::rename` outside `tempfile::persist`.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError> {
    let parent = path.parent().ok_or_else(|| InboxError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        ),
    })?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.write_all(content.as_bytes()).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.as_file().sync_all().map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.persist(path).map_err(|e| InboxError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use crate::row::{Severity, Status};

    fn sample_row(advisory_id: &str, status: Status) -> AdvisoryRow {
        AdvisoryRow {
            date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
            advisory_id: advisory_id.to_string(),
            source_url: format!("https://example.test/{}", advisory_id),
            package: "pkg@<1.0".to_string(),
            file_line: "src/foo.rs:1".to_string(),
            severity: Severity::High,
            status,
            note: "-".to_string(),
        }
    }

    const BASELINE: &str = "# Advisory Inbox\n\n## Rows\n\n| 2026-05-20 | CVE-OLD-1 | https://example.test/CVE-OLD-1 | pkg@<0.9 | src/old.rs:1 | High | open | - |\n| 2026-05-19 | CVE-OLD-2 | https://example.test/CVE-OLD-2 | pkg@<0.8 | src/old.rs:2 | Medium | processed | - |\n";

    #[test]
    fn insert_rows_happy_path() {
        let rows = vec![
            sample_row("CVE-NEW-1", Status::Open),
            sample_row("CVE-NEW-2", Status::Open),
        ];
        let (out, total_open) = insert_rows(BASELINE, &rows).expect("insert ok");

        // Both new rows present.
        assert!(out.contains("CVE-NEW-1"));
        assert!(out.contains("CVE-NEW-2"));
        // Old rows still present.
        assert!(out.contains("CVE-OLD-1"));
        assert!(out.contains("CVE-OLD-2"));
        // Order: CVE-NEW-1 (rows[0]) appears BEFORE CVE-OLD-1 in output.
        let pos_new1 = out.find("CVE-NEW-1").unwrap();
        let pos_old1 = out.find("CVE-OLD-1").unwrap();
        assert!(pos_new1 < pos_old1, "rows[0] should be topmost");
        // total_open count: CVE-OLD-1 open + CVE-NEW-1 open + CVE-NEW-2 open = 3.
        // (CVE-OLD-2 is processed, not counted.)
        assert_eq!(total_open, 3);
    }

    #[test]
    fn insert_rows_missing_heading_errors() {
        let no_heading = "# Advisory Inbox\n\nSome text but no Rows heading.\n";
        let rows = vec![sample_row("CVE-X", Status::Open)];
        let err = insert_rows(no_heading, &rows).unwrap_err();
        assert!(matches!(err, InboxError::MissingRowsHeading { .. }));
    }

    #[test]
    fn insert_rows_empty_rows_noop() {
        let (out, total_open) = insert_rows(BASELINE, &[]).expect("insert ok empty");
        // Content structurally preserved (line-for-line).
        for old_line in BASELINE.lines() {
            assert!(out.contains(old_line), "old line missing: {old_line}");
        }
        // total_open = 1 (only CVE-OLD-1 is open).
        assert_eq!(total_open, 1);
    }

    #[test]
    fn write_atomic_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("inbox.md");
        let content = "# Advisory Inbox\n\n## Rows\n\n| 2026-05-28 | CVE-X | u | p | f:1 | High | open | - |\n";
        write_atomic(&target, content).expect("write atomic ok");
        // File exists with expected content.
        let read_back = std::fs::read_to_string(&target).expect("read back");
        assert_eq!(read_back, content);
        // No `.tmp` leftover in parent dir.
        let leftover_count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with(".tmp"))
            .count();
        assert_eq!(leftover_count, 0);
    }

    #[test]
    fn count_open_skips_html_comment_block() {
        // Placeholder example in HTML comment must NOT be counted.
        let with_comment = "## Rows\n\n| 2026-05-28 | CVE-A | u | p | f:1 | High | open | - |\n\n<!--\n| 2026-05-23 | CVE-PLACEHOLDER | u | p | f:1 | Medium | open | - |\n-->\n";
        // Only CVE-A counted.
        assert_eq!(count_open_rows(with_comment), 1);
    }
}
```

**Lưu ý:**
- Module-level docstring cites ARCHITECTURE §3 + INVARIANTS §3 explicit — anchor for future maintainers.
- `insert_rows` uses `lines()` which strips line endings; rebuild with `\n`. Preserves trailing-newline behavior of original content.
- `count_open_rows` is heuristic substring match `| open |`. Worker may tighten if false-positives surface in fixture testing (Tầng 2 self-decide).
- `write_atomic` uses `as_file().sync_all()` per INV-LOCAL-002. Anchor #9 flagged — Worker verifies via Task 0 compile probe; fallback `temp.flush()?` (no `sync_all`) if `as_file()` API differs.
- `PathBuf::new()` placeholder in `insert_rows`'s `MissingRowsHeading` error — caller (`cli/append.rs`) does NOT replace it before bubble (anyhow wraps, Display still shows `inbox '' is missing ## Rows heading`). Acceptable Tầng 2 — Worker may improve by accepting `path: &Path` parameter on `insert_rows` (then propagate into error). Self-decide; thông báo Discovery.
  - **Architect recommendation:** add `path: &Path` parameter to `insert_rows(content, rows, path)` for cleaner error messages. Tradeoff: tightens API, but path-aware error is meaningful (Sếp sees actual filename in error). Worker pick.
- `#[cfg(test)] mod tests` — Worker may need to add `use crate::row::AdvisoryRow;` if not already in scope from sibling module (it's in scope via `super::*` + the parent-module `use crate::row::AdvisoryRow;`).
- 5 unit tests minimum (4 listed + `count_open_skips_html_comment_block`). Worker may add more edge cases if discovered.
- NO `unsafe { }` block — INV-LOCAL-001.

### Task 2: `src/row.rs` — add `impl Display for AdvisoryRow`

**File:** `src/row.rs`

**Tìm** (Worker grep verify post-Task 0):
```rust
// existing AdvisoryRow + Status + Severity definitions (P002 ship)
// — KHÔNG đổi struct shape, KHÔNG đổi derive, KHÔNG đổi serde rename
```

**Thêm** (after the existing `AdvisoryRow` struct + impl blocks, BEFORE `#[cfg(test)] mod tests`):

```rust
use std::fmt;

impl fmt::Display for AdvisoryRow {
    /// Render as pipe-delimited 8-col line per ARCHITECTURE.md §3.
    ///
    /// Format: `| {date} | {advisory_id} | {source_url} | {package} | {file_line} | {severity} | {status} | {note} |`
    ///
    /// - `date`: ISO calendar date `YYYY-MM-DD` (chrono NaiveDate default Display).
    /// - `severity`: PascalCase variant name (`Critical`/`High`/`Medium`/`Low`).
    /// - `status`: lowercase variant name (`open`/`processed`/`dismissed`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "| {date} | {advisory_id} | {source_url} | {package} | {file_line} | {severity} | {status} | {note} |",
            date = self.date,
            advisory_id = self.advisory_id,
            source_url = self.source_url,
            package = self.package,
            file_line = self.file_line,
            severity = self.severity,
            status = self.status,
            note = self.note,
        )
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Open => f.write_str("open"),
            Status::Processed => f.write_str("processed"),
            Status::Dismissed => f.write_str("dismissed"),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Critical => f.write_str("Critical"),
            Severity::High => f.write_str("High"),
            Severity::Medium => f.write_str("Medium"),
            Severity::Low => f.write_str("Low"),
        }
    }
}
```

**Thêm test** (inside existing `#[cfg(test)] mod tests`):

```rust
#[test]
fn advisory_row_display_pipe_delim() {
    use chrono::NaiveDate;
    let row = AdvisoryRow {
        date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
        advisory_id: "CVE-2026-9999".to_string(),
        source_url: "https://nvd.nist.gov/vuln/detail/CVE-2026-9999".to_string(),
        package: "next@<15.5.17".to_string(),
        file_line: "src/middleware.ts:42".to_string(),
        severity: Severity::High,
        status: Status::Open,
        note: "-".to_string(),
    };
    let rendered = format!("{row}");
    assert_eq!(
        rendered,
        "| 2026-05-28 | CVE-2026-9999 | https://nvd.nist.gov/vuln/detail/CVE-2026-9999 | next@<15.5.17 | src/middleware.ts:42 | High | open | - |"
    );
}
```

**Lưu ý:**
- 3 `impl Display` blocks: `AdvisoryRow` (composite), `Status` (lowercase), `Severity` (PascalCase). Status/Severity Display must EXACTLY match serde rename_all output (lowercase / PascalCase respectively) so inbox round-trip (Display → parse via future `FromStr`) is consistent with JSON round-trip (serialize → deserialize). Worker verify P002 serde rename rules match.
- If P002 used `#[serde(rename_all = "lowercase")]` on Status and `#[serde(rename_all = "PascalCase")]` on Severity — Display impl above matches. If either differs → STOP escalate (P002 spec drift).
- `chrono::NaiveDate` Display = `YYYY-MM-DD` (verified P004 indirect; Worker test #10 confirms compile + runtime).
- KHÔNG add `impl FromStr` (out-of-scope per Scope).
- `use std::fmt;` at top of file — add if not present (P002 shipped row.rs without it).

### Task 3: `src/cli/append.rs` — wire-in

**File:** `src/cli/append.rs`

**Tìm** (P001 stub — Worker verify Task 0; expect signature `pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()>` body printf TODO, analogous to P004/P005 stubs):

```rust
pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!("TODO: append (inbox={:?} rows_json={:?}) — wired in P006", inbox, rows_json);
    Ok(())
}
```

(Worker confirm exact wording; phiếu signature based on P004/P005 stub precedent.)

**Thay bằng (full file content):**

```rust
//! `advisory-inbox append` — insert filtered rows into the inbox markdown at
//! the top of `## Rows`, atomic-write.
//!
//! See `docs/ARCHITECTURE.md` §1 subcmd `append` for the I/O contract,
//! §3 for the inbox markdown format, and `docs/security/INVARIANTS.md` §3
//! INV-LOCAL-002 for the atomic-write protocol (delegated to `inbox::write_atomic`).

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::inbox;
use crate::row::AdvisoryRow;

/// JSON envelope shape accepted by `append`. Same shape as `parse-report`
/// and `dedup` output (per ARCHITECTURE §1 — Tầng 1 contract: ONE input shape).
#[derive(Deserialize)]
struct RowsEnvelope {
    rows: Vec<AdvisoryRow>,
}

/// Read rows JSON + inbox, insert rows after `## Rows` heading, atomic-write,
/// emit `{ "appended_count": N, "total_open": M }` to stdout.
///
/// # Errors
/// - [`inbox::InboxError::MissingRowsHeading`] (exit 1 in `main.rs`).
/// - [`inbox::InboxError::Io`] (exit 2 in `main.rs`).
/// - I/O + serde errors on rows JSON file → anyhow bubble (exit 2 in `main.rs`).
pub fn run(inbox_path: PathBuf, rows_json: PathBuf) -> Result<()> {
    // 1. Read rows envelope.
    let rows_text = std::fs::read_to_string(&rows_json)
        .with_context(|| format!("read rows file {}", rows_json.display()))?;
    let envelope: RowsEnvelope = serde_json::from_str(&rows_text)
        .with_context(|| format!("parse rows JSON from {}", rows_json.display()))?;

    // 2. Read inbox.
    let content = inbox::read_inbox(&inbox_path)?;

    // 3. Insert (newest at top — rows[0] topmost).
    let (new_content, total_open) = inbox::insert_rows(&content, &envelope.rows)?;

    // 4. Atomic write.
    inbox::write_atomic(&inbox_path, &new_content)?;

    // 5. Emit JSON stdout + trailing newline.
    let appended_count = envelope.rows.len();
    let out = serde_json::json!({
        "appended_count": appended_count,
        "total_open": total_open,
    });
    serde_json::to_writer(std::io::stdout().lock(), &out).context("write stdout JSON")?;
    println!();
    Ok(())
}
```

**Lưu ý:**
- Function arg `inbox_path: PathBuf` (rename từ `inbox` để tránh shadow `inbox` module import).
- `RowsEnvelope` defined INLINE — same shape as P005 `cli/dedup.rs`'s local definition. Duplicate accepted per P005 Discovery scope discipline (move-to-shared deferred to future).
- `inbox::read_inbox` + `inbox::insert_rows` + `inbox::write_atomic` propagate `InboxError` via anyhow auto-wrap (`From<InboxError> for anyhow::Error` blanket impl since `InboxError: std::error::Error + Send + Sync + 'static`).
- Rows JSON file errors (`std::io::Error` + `serde_json::Error`) bubble via `with_context` anyhow chain; main.rs else-branch maps → exit 2 (per ARCHITECTURE §1 contract).
- `serde_json::json!` macro alphabetizes keys — actual stdout key order will be `"appended_count"` → `"total_open"` (already alpha). Test assertions use substring `predicate::str::contains`.
- KHÔNG `process::exit` trong `run()` — bubble via `Result`.
- KHÔNG add flag `--dry-run`/`--backup`/`--force`.

### Task 4: `src/main.rs` — `mod inbox;` + dispatch arm

**File:** `src/main.rs`

**4.1. Add `mod inbox;` declaration:**

**Tìm** (existing `mod` declarations — Worker grep, e.g., `mod row;`, `mod state;`, `mod sentinel;`, `mod cli;`):

```rust
mod cli;
mod row;
mod sentinel;
mod state;
```

(Worker confirm exact order + presence — P001/P002/P003 ship sequence.)

**Thay bằng (add `inbox` in alphabetical order):**

```rust
mod cli;
mod inbox;
mod row;
mod sentinel;
mod state;
```

Worker preserves existing order if not alphabetical; key requirement is `mod inbox;` exists somewhere in main.rs at module level.

**4.2. Verify `Commands::Append` clap variant intact:**

**Tìm** (Worker `grep -n "Append" src/main.rs` Task 0; expect existing P001 shape):

```rust
Append {
    #[arg(long)]
    inbox: PathBuf,
    #[arg(long = "rows-json")]
    rows_json: PathBuf,
},
```

→ Match → KHÔNG đổi struct.
→ Thiếu/khác → STOP escalate (P001 drift — ARCHITECTURE §1 contract).

**4.3. Update dispatch arm — error → exit code map:**

**Tìm** (current dispatch — Worker grep verify):
```rust
Commands::Append { inbox, rows_json } => cli::append::run(inbox, rows_json),
```

**Thay bằng:**
```rust
Commands::Append { inbox, rows_json } => {
    if let Err(e) = cli::append::run(inbox, rows_json) {
        let code = if let Some(ie) = e.downcast_ref::<crate::inbox::InboxError>() {
            match ie {
                crate::inbox::InboxError::MissingRowsHeading { .. } => 1,
                crate::inbox::InboxError::Io { .. } => 2,
            }
        } else {
            2  // rows JSON malformed / unreadable / other serde err → exit 2
        };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

**Lưu ý:**
- **Tail `Ok(())` REQUIRED** (P004 Turn 1 O1.1 precedent — match-arm uniformity).
- `e.downcast_ref::<InboxError>()` returns `Option<&InboxError>`; pattern-match the variant to assign exit code. This is RICHER than P005's `is::<StateReadError>()` boolean check because P006 has 2 sub-variants of `InboxError` needing distinct codes.
- Alternative: `e.is::<InboxError>()` + custom logic — REJECT because we need variant discrimination.
- KHÔNG đổi other dispatch arms (ParseReport / Dedup / MigrateState / StateBackfill / ScanAndAppend / Serve / Init — all preserved).
- Verify `use std::path::PathBuf;` already imported (P004/P005 confirmed — Anchor #13).

### Task 5: Fixtures

**File 5.1:** `tests/fixtures/inbox-baseline.md` (new)

```markdown
# Advisory Inbox

> Sếp gạt row "open" → "processed" hoặc "dismissed" + ghi note.

## Rows

| 2026-05-20 | CVE-OLD-1 | https://example.test/CVE-OLD-1 | pkg@<0.9 | src/old.rs:1 | High | open | - |
| 2026-05-19 | CVE-OLD-2 | https://example.test/CVE-OLD-2 | pkg@<0.8 | src/old.rs:2 | Medium | processed | - |

<!-- Placeholder example (in HTML comment — append skips this) -->
<!--
| 2026-05-15 | GHSA-placeholder | https://example.test/placeholder | example@<1.0 | indirect | Medium | open | - |
-->
```

**File 5.2:** `tests/fixtures/rows-2.json` (new)

```json
{
  "rows": [
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-NEW-1",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-NEW-1",
      "package": "next@<15.5.17",
      "file_line": "src/middleware.ts:42",
      "severity": "High",
      "status": "open",
      "note": "-"
    },
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-NEW-2",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-NEW-2",
      "package": "flask@<2.3.5",
      "file_line": "app.py:8",
      "severity": "Medium",
      "status": "open",
      "note": "-"
    }
  ]
}
```

**Lưu ý:**
- `inbox-baseline.md` mimics ARCHITECTURE §3 example structure: title + Sếp note + `## Rows` heading + 2 rows (1 open + 1 processed) + HTML comment placeholder.
- Acceptance accounting:
  - Baseline open count: 1 (CVE-OLD-1) — CVE-OLD-2 is processed; placeholder in HTML comment NOT counted.
  - After append 2 new (both open): `total_open = 1 + 2 = 3`.
  - `appended_count = 2`.
- `rows-2.json` uses field naming exactly matching P002 `AdvisoryRow` serde derive: `date`, `advisory_id`, `source_url`, `package`, `file_line`, `severity` (PascalCase value), `status` (lowercase value), `note`.
- KHÔNG add rows with `Severity::Critical` or `Status::Dismissed` in fixture — keeps acceptance arithmetic simple (test_open_count = 3 deterministic).
- Worker preserve trailing newline at EOF of both fixtures (POSIX convention).
- HTML comment block MUST remain unchanged after append (insert positions ABOVE comment block since comment is below the existing 2 rows). Worker verifies in integration test (Task 6).

### Task 6: Integration test `tests/append_cli.rs`

**File:** `tests/append_cli.rs` (new)

```rust
//! Integration tests for `advisory-inbox append` subcmd.
//!
//! Covers: happy path (2 new rows inserted at top, old rows preserved,
//! placeholder comment intact, exit 0), missing `## Rows` heading (exit 1),
//! rows JSON malformed (exit 2), atomic-write smoke test (file exists post-persist).

use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;

const BASELINE_FIXTURE: &str = "tests/fixtures/inbox-baseline.md";
const ROWS_2: &str = "tests/fixtures/rows-2.json";

#[test]
fn append_happy_path_2_new_rows() {
    // Copy baseline to a tempdir so the test mutates a throwaway file.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox").arg(&target)
        .arg("--rows-json").arg(ROWS_2)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""appended_count":2"#))
        .stdout(predicate::str::contains(r#""total_open":3"#));

    // Verify file content post-append.
    let after = std::fs::read_to_string(&target).expect("read after");

    // New rows present.
    assert!(after.contains("CVE-NEW-1"), "CVE-NEW-1 missing");
    assert!(after.contains("CVE-NEW-2"), "CVE-NEW-2 missing");
    // Old rows preserved.
    assert!(after.contains("CVE-OLD-1"), "CVE-OLD-1 missing");
    assert!(after.contains("CVE-OLD-2"), "CVE-OLD-2 missing");
    // Placeholder HTML comment block intact.
    assert!(after.contains("GHSA-placeholder"), "placeholder comment block damaged");
    assert!(after.contains("<!-- Placeholder example"), "placeholder header comment damaged");

    // Order: new rows appear BEFORE old rows in the inbox text.
    let pos_new1 = after.find("CVE-NEW-1").expect("new1 pos");
    let pos_old1 = after.find("CVE-OLD-1").expect("old1 pos");
    assert!(pos_new1 < pos_old1, "newest rows must be at top of ## Rows");

    // `## Rows` heading still present and appears ONCE.
    assert_eq!(after.matches("## Rows").count(), 1, "## Rows heading should be unique");
}

#[test]
fn append_missing_heading_exit_1() {
    // Inline-build an inbox without `## Rows`.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox-noheading.md");
    std::fs::write(&target, "# Advisory Inbox\n\nNo heading here.\n").expect("write");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox").arg(&target)
        .arg("--rows-json").arg(ROWS_2)
        .assert()
        .failure()
        .code(1)
        .stderr(
            predicate::str::contains("## Rows")
                .or(predicate::str::contains("heading"))
                .or(predicate::str::contains("missing")),
        );
}

#[test]
fn append_rows_malformed_exit_2() {
    // Inline-build a rows file without the "rows" key.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(br#"{"not_rows": []}"#).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox").arg(&target)
        .arg("--rows-json").arg(tmp.path())
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("rows").or(predicate::str::contains("JSON")));
}

#[test]
fn append_atomic_write_no_leftover_tmp() {
    // Smoke test: after successful append, no leftover .tmp file in parent dir.
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("inbox.md");
    std::fs::copy(BASELINE_FIXTURE, &target).expect("copy baseline");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("append")
        .arg("--inbox").arg(&target)
        .arg("--rows-json").arg(ROWS_2)
        .assert()
        .success();

    // NamedTempFile creates files with `.tmp` prefix or random suffix in parent;
    // after persist, the temp filename should be GONE (renamed to target).
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("readdir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // Expect exactly 1 entry: "inbox.md". No `.tmpXXXX` siblings.
    assert_eq!(entries.len(), 1, "leftover temp files: {entries:?}");
    assert_eq!(entries[0], "inbox.md");
}
```

**Lưu ý:**
- 4 integration tests. Acceptance criteria covered in `append_happy_path_2_new_rows`.
- `tempfile::tempdir()` + `std::fs::copy(BASELINE_FIXTURE, &target)` — keeps `tests/fixtures/inbox-baseline.md` immutable (committed file untouched by test run).
- `predicate::str::contains` substring match (NOT exact) — `serde_json::json!` key ordering already alphabetical for `appended_count` → `total_open` so substring assertions work.
- `predicate::str::contains(...).or(...)` predicates logical OR confirmed P005 working.
- Atomic-write smoke (`append_atomic_write_no_leftover_tmp`): NamedTempFile names contain `.tmp` prefix by default; after `persist`, temp file is RENAMED to target → no sibling. Worker may need to relax this assertion if `tempfile` 3 uses different prefix scheme — Tầng 2 self-decide (acceptable: just assert target exists + content non-empty).

### Task 6.5: Docs — Tầng 1 update

**6.5.1.** `docs/CHANGELOG.md` — add entry top of relevant section:

```markdown
## [Unreleased] — 2026-05-28

### Added (P006 — append subcmd atomic write)
- New module `src/inbox.rs` — `read_inbox`, `insert_rows`, `write_atomic` per INV-LOCAL-002 atomic-write protocol.
- `cli/append.rs` wired (stub → real impl): read rows JSON + inbox, insert at top of `## Rows`, atomic write, emit `{ "appended_count": N, "total_open": M }`.
- `InboxError` enum (2 variant): `MissingRowsHeading` → exit 1, `Io` → exit 2.
- `impl Display for AdvisoryRow / Status / Severity` — pipe-delim 8-col line per ARCHITECTURE §3.
- Fixtures: `tests/fixtures/inbox-baseline.md`, `tests/fixtures/rows-2.json`.
- Integration tests: `tests/append_cli.rs` (4 cases).
- Lane: Guarded (filesystem write). Sub-mech B + D + F.
```

(Worker preserve existing CHANGELOG entries; append new section at top of `## [Unreleased]` block.)

**6.5.2.** `docs/ARCHITECTURE.md` §5 — mark P006 shipped:

**Tìm** (existing P002-P005 scaffold-status block, dòng ~238-244):
```markdown
**Scaffold status (2026-05-28):**
- P001: ...
- P002: ...
- P003: ...
- P004: ...
- P005: ...
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Thay bằng:**
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic.
- P004: `cli/parse_report.rs` wired; `row::parse_row` + `RowParseError` + `FromStr` for `Status`/`Severity` shipped.
- P005: `cli/dedup.rs` wired; `state::read` + `StateReadError` shipped.
- P006: `cli/append.rs` wired; `inbox.rs` (`read_inbox` + `insert_rows` + `write_atomic` + `InboxError`) shipped — first concrete user of INV-LOCAL-002 atomic-write protocol. `impl Display for AdvisoryRow / Status / Severity` added to `row.rs`.
- Pending Phase 1+ phiếu (see BACKLOG.md): `mcp/`, `error.rs`.
```

**6.5.3.** `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 — add cross-ref note:

**Tìm** (end of INV-LOCAL-002 block, the `Status: Active. ...` line at ~115):
```markdown
**Status:** Active. Inherits INV-21 of advisory-cron pattern.
```

**Thay bằng:**
```markdown
**Status:** Active. Inherits INV-21 of advisory-cron pattern.

**First concrete user:** `src/inbox.rs::write_atomic` (P006, shipped 2026-05-28). Reference shape — future state-write subcmd (P007 migrate-state, P008 state-backfill, P009 scan-and-append, P011 MCP `append` tool) MUST follow same protocol.

**Note on user-supplied path:** `--inbox <FILE>` argument is user-controlled. Worker / Sếp PHẢI ensure path points to intended inbox markdown file (typo could write to wrong file). Atomic protocol ensures partial-write safety; it does NOT validate semantic intent. No file-content echoing into stderr/log (Sub-mech F clean).
```

**6.5.4.** `README.md` — add `append` quick-start (conditional per Anchor #14):

If Anchor #14 shows append already has full quick-start section → skip. If only stub line → replace with full block analogous to P005's dedup quick-start.

**Skeleton (Worker adapts to existing README structure):**

```markdown
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
```

**Lưu ý:**
- 4 sub-tasks (CHANGELOG / ARCHITECTURE / INVARIANTS / README). ALL Tầng 1 per RULES.md §11 (new module added + new MCP-callable surface eventually + INVARIANTS touched).
- Docs Gate must pass: `docs-gate --all --verbose` clean before commit.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/inbox.rs` | **NEW** — Task 1: `InboxError`, `read_inbox`, `insert_rows`, `write_atomic`, `count_open_rows` (private), 5 unit tests. |
| `src/row.rs` | Task 2: add `impl Display for AdvisoryRow / Status / Severity`, 1 unit test. |
| `src/cli/append.rs` | Task 3: stub → real impl (`RowsEnvelope` inline, `pub fn run`). |
| `src/main.rs` | Task 4: add `mod inbox;`, update `Commands::Append` dispatch arm with `InboxError` downcast + variant-aware exit code. |
| `tests/fixtures/inbox-baseline.md` | **NEW** — Task 5: 2 existing rows (1 open + 1 processed) + HTML comment placeholder. |
| `tests/fixtures/rows-2.json` | **NEW** — Task 5: `{ "rows": [...] }` envelope, 2 open rows. |
| `tests/append_cli.rs` | **NEW** — Task 6: 4 integration tests. |
| `docs/CHANGELOG.md` | Task 6.5.1: P006 entry. |
| `docs/ARCHITECTURE.md` | Task 6.5.2: §5 scaffold-status mark P006 shipped + remove `inbox.rs` from pending. |
| `docs/security/INVARIANTS.md` | Task 6.5.3: INV-LOCAL-002 "First concrete user" note + user-supplied path note. |
| `README.md` | Task 6.5.4: `append` quick-start (conditional Anchor #14). |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/state.rs` | P005 lock — append does NOT touch state file. |
| `src/sentinel.rs` | P003 lock — append does NOT parse sentinel markers. |
| `src/cli/parse_report.rs` | P004 lock — independent subcmd. |
| `src/cli/dedup.rs` | P005 lock — `RowsEnvelope` shape reused but defined inline in `cli/append.rs` (no shared module). |
| `src/cli/mod.rs` | Anchor #19: `pub mod append;` already present — verify, do NOT re-add. |
| `Cargo.toml` | Anchor #7/#8: no new dep — `tempfile`, `thiserror`, `serde_json`, `chrono`, `anyhow`, `serde` all already present. NO `cargo add`. |
| `.advisory-scan-state` (runtime) | NOT TOUCHED — P006 has no state file logic. |
| `src/main.rs` (other dispatch arms) | ParseReport/Dedup/MigrateState/StateBackfill/ScanAndAppend/Serve/Init arms preserved. |

---

## Luật chơi (Constraints)

1. **Stay within current `Cargo.toml` deps.** NO `cargo add`. Verify via Anchor #7/#8.
2. **Atomic write MUST follow INV-LOCAL-002 protocol exactly:** `NamedTempFile::new_in(parent)` (same filesystem) + `write_all` + `as_file().sync_all()` (fsync) + `persist(target)`. FORBIDDEN: `OpenOptions::append`, `std::fs::write` direct, `std::fs::rename` outside `tempfile::persist`. INV-LOCAL-002 mandate.
3. **No `unsafe { }` block.** INV-LOCAL-001 standing rejection.
4. **`RowsEnvelope { rows: Vec<AdvisoryRow> }` shape only.** Flat array `[ {...} ]` NOT supported (Tầng 1 contract — fail-fast on other shapes). Same as P005.
5. **Insert order:** `rows[0]` ends up TOPMOST in output. Existing rows preserved in their original relative order, shifted down.
6. **`## Rows` heading match:** line equals `## Rows` after `trim_end()` (tolerate trailing whitespace). NOT regex.
7. **`total_open` count:** scan post-insert content, count rows where status column = `open`, SKIP rows inside HTML comment blocks `<!-- ... -->` (per ARCHITECTURE §3 parser rules).
8. **Pipe-delim row format (8 col):** `| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |`. Column order locked per ARCHITECTURE §3 — DO NOT permute. Render via `impl Display for AdvisoryRow`.
9. **No token leak in stderr/log.** Sub-mech F: error messages reference user-supplied path but DO NOT echo file content. Verify via `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/` → 0 hits.
10. **Exit code semantics (ARCHITECTURE §1 append):** 0 success, 1 missing `## Rows` heading, 2 write error / rows malformed. KHÔNG drift.
11. **Inbox markdown format (sentinel markers, row column order)** KHÔNG đổi — INV-LOCAL-005 protected.
12. **HTML comment placeholder block preserved verbatim** after insert. Integration test `append_happy_path_2_new_rows` asserts.
13. **No `process::exit` in `run()` functions** — bubble via `Result`, main.rs handles exit code mapping (P004/P005 precedent).
14. **No `--dry-run` / `--backup` / `--force` flag.** Out-of-scope; future phiếu if needed.
15. **No `impl FromStr` for `AdvisoryRow` / `Status` / `Severity` from inbox markdown row.** P002 already shipped `FromStr` for Status/Severity via P004; `AdvisoryRow` FromStr from pipe-delim line is P008 state-backfill domain.
16. **Hard Stop on any anchor drift** (Anchor #2 / #3 / #4 / #5 / #19 / #20 explicit triggers). DO NOT silently fix — escalate.
17. **Match existing rustfmt + clippy convention.** No `dbg!()`, no `eprintln!()` debug, no commented-out code in final commit (DoD §5).
18. **Test count target:** baseline (P005) = 31 tests. P006 adds ≥5 unit (4 inbox + 1 row Display) + ≥4 integration = ≥9 new. Final ≥40 tests.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass, ≥40 tests total
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] `cp tests/fixtures/inbox-baseline.md /tmp/p006-test-inbox.md && cargo run --quiet -- append --inbox /tmp/p006-test-inbox.md --rows-json tests/fixtures/rows-2.json` → stdout JSON `appended_count: 2, total_open: 3`, exit 0; `/tmp/p006-test-inbox.md` has CVE-NEW-1 + CVE-NEW-2 at top of `## Rows`, CVE-OLD-1 + CVE-OLD-2 preserved below, HTML comment placeholder intact at bottom.
- [ ] Run again with same args (idempotency NOT a contract — re-running APPENDS again, so total_open grows by 2 each run). Verify exit 0, no corruption.
- [ ] `echo '' > /tmp/empty-inbox.md && cargo run --quiet -- append --inbox /tmp/empty-inbox.md --rows-json tests/fixtures/rows-2.json` → exit 1, stderr contains "Rows" or "heading".
- [ ] `echo '{"not_rows":[]}' > /tmp/bad-rows.json && cargo run --quiet -- append --inbox tests/fixtures/inbox-baseline.md --rows-json /tmp/bad-rows.json` → exit 2 (NOTE: don't run on committed baseline; copy first if you fear contamination).

### Regression
- [ ] `cargo run --quiet -- parse-report < tests/fixtures/agent-report-1.md` → still works per P004 contract, exit 0.
- [ ] `cargo run --quiet -- dedup --state tests/fixtures/state-3ids.json --rows-json tests/fixtures/rows-5.json` → still works per P005 contract (`kept: 3, skipped: 2, observed_ids: 5`), exit 0.
- [ ] `cargo run --quiet -- --help` → 8 subcmd still listed.

### Docs Gate (Tầng 1 — CỨNG)
- [ ] `docs/CHANGELOG.md` — P006 entry added (Task 6.5.1).
- [ ] `docs/ARCHITECTURE.md` §5 — P006 scaffold-status entry; `inbox.rs` removed from "Pending" list (Task 6.5.2).
- [ ] `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 — "First concrete user" note added (Task 6.5.3).
- [ ] `README.md` — `append` quick-start (conditional, Task 6.5.4 + Anchor #14).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P006.md` — full report written per RULES.md §13 format.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top).
- [ ] Sub-mechanism A-F Verification Trace filled (table above): A=N/A, B (cargo check + cargo test inbox + cargo test row + cargo test append_cli + happy-path run) ✅, C=N/A (no schema change), D (ARCHITECTURE + INVARIANTS + README grep) ✅, E (cargo update --dry-run + cargo build --release clean) ✅, F (grep token pattern + session-start-banner.sh) ✅.

### Lane + INV gate (Guarded specific)
- [ ] PR body `## Lane override` section: `original: guarded`, `requested: guarded`, no override.
- [ ] `/security-review <PR>` auto-invoked post-push (orchestrator) — Giám sát Verdict APPROVE (or self-acknowledged FLAG with rationale).
- [ ] INV-LOCAL-002 hand-check: `grep -n "NamedTempFile::new_in\|sync_all\|persist" src/inbox.rs` → ≥3 hits in `write_atomic`; `grep -E "OpenOptions::append|std::fs::write\(.*inbox|std::fs::rename" src/inbox.rs src/cli/append.rs` → 0 hits.
- [ ] INV-LOCAL-005 hand-check: no rename of sentinel markers (P006 does NOT touch sentinel — verify `git diff src/sentinel.rs` empty).
