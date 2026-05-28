# PHIẾU P005: dedup subcmd

> **ID format:** `P005` — counter `.phieu-counter` = 5 sau P004 ship.
> **Filename:** `docs/ticket/P005-dedup.md`
> **Branch:** `feat/P005-dedup`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P006 append + P009 scan-and-append; dedup là filter middle stage giữa parse-report → append)
> **Ảnh hưởng:** `src/cli/dedup.rs` (stub → real impl), `src/state.rs` (add `pub fn read(&Path) -> anyhow::Result<StateFile>` + tests; xoá `#![allow(dead_code)]`), `src/main.rs` (dispatch `Commands::Dedup { state, rows_json }` error → exit code map), `tests/fixtures/state-3ids.json` (new fixture), `tests/fixtures/rows-5.json` (new fixture), `tests/dedup_cli.rs` (new integration test), `docs/ARCHITECTURE.md` §5, `docs/CHANGELOG.md`, `README.md` (quick-start nếu chưa cover dedup)
> **Dependency:** P001 (CLI scaffold), P002 (`StateFile` + `AdvisoryRow`), P004 (parse-report rows JSON output shape) — tất cả đã ship 2026-05-28
> **Lane:** Normal (CLI subcmd wire-in + public API surface mới trong `state.rs` `read()` + new test files — Normal per RULES.md §1; KHÔNG filesystem-write/process-spawn/secret-handling → không phải Guarded)
> **Sub-mech áp dụng:** B (capability — `cargo check` + `cargo test`), C (state schema check — `state::read` enforces `schema_version == 1`), D (persistence — ARCHITECTURE §5 mark P005 shipped, README quick-start sync)

---

## Context

### Vấn đề hiện tại

P004 ship `parse-report` → emits JSON `{ "rows": [...], "stack_scanned": {}, "advisories_found": N }`. P002 ship `StateFile` struct + `SCHEMA_VERSION = 1`. Bây giờ **wire `dedup` subcmd** để filter rows mới qua `state.seen_advisories[]`.

Pipeline yêu cầu:
1. Read state file JSON → `StateFile` (validate `schema_version == 1`).
2. Read rows JSON file → unwrap `{ "rows": [...] }` envelope → `Vec<AdvisoryRow>`.
3. For each row: if `advisory_id ∈ state.seen_advisories` → push `skipped`; else → push `kept`.
4. `observed_ids[]` = ALL input row advisory_ids (regardless of kept/skipped) — downstream consumers (future P009 scan-and-append) dùng để update state.
5. Output JSON `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` → stdout.
6. Map error → exit 1 (state unreadable / schema mismatch / file missing) hoặc 2 (rows malformed).

Stub hiện tại của `cli/dedup.rs` (P001) printf TODO. Sau P005:
- `cargo run -- dedup --state tests/fixtures/state-3ids.json --rows-json tests/fixtures/rows-5.json` → JSON stdout với `kept` 3 row + `skipped` 2 row + `observed_ids` 5 IDs, exit 0.
- State file missing / schema_version != 1 → stderr, exit 1.
- Rows JSON malformed (missing `rows` key / not an array / bad row shape) → stderr, exit 2.

Reference BACKLOG.md P005:
- Scope: Wire `cli/dedup.rs` → read state JSON → filter rows → JSON stdout. Preserve `observed_ids[]` for state update.
- Acceptance: Fixture state với 3 IDs + report với 5 rows (2 match) → output `kept: 3, skipped: 2`.
- Sub-mech checks: B (cargo check), C (state schema check).

### Giải pháp

**4 unit công việc chính:**

1. **`src/state.rs` mở rộng:**
   - Thêm `pub fn read(path: &Path) -> anyhow::Result<StateFile>`: đọc file → `serde_json::from_str` → `StateFile`. Sau parse, validate `state.schema_version == SCHEMA_VERSION` (else error gợi ý chạy `migrate-state`).
   - **Xoá `#![allow(dead_code)]`** ở đầu file (per P002 Discovery — xoá khi consumer wire-in). Sau P005, `StateFile` + `SCHEMA_VERSION` đều có public consumer (`state::read` + `cli::dedup::run`). Nếu Worker thấy clippy `dead_code` warning sau khi xoá → STOP escalate (signals incomplete wire-in).
   - Thêm ≥3 unit test cho `read()`:
     - Happy path: write fixture tempfile, read back, fields match.
     - Missing file: `read("/nonexistent/path")` → `Err`.
     - Schema mismatch: write JSON với `schema_version: 99` → `Err` containing hint `migrate-state`.

2. **`src/cli/dedup.rs` real impl:**
   - Replace stub printf bằng `pub fn run(state: PathBuf, rows_json: PathBuf) -> anyhow::Result<()>`.
   - Read state qua `state::read(&state)?` (state-side errors propagate qua anyhow; `main.rs` downcast `StateReadError` → exit 1).
   - Read rows JSON: `std::fs::read_to_string(&rows_json).with_context(...)` rồi `serde_json::from_str::<RowsEnvelope>(&txt)` với `#[derive(Deserialize)] struct RowsEnvelope { rows: Vec<AdvisoryRow> }`. Error propagate qua anyhow; downcast to map exit code 2.
   - Loop rows, build 3 vector: `kept: Vec<AdvisoryRow>`, `skipped: Vec<AdvisoryRow>`, `observed_ids: Vec<String>`. Use `state.seen_advisories.iter().any(|id| id == &row.advisory_id)` or `state.seen_advisories.contains(&row.advisory_id)` (cleaner; `Vec<String>::contains(&String)` works).
   - Output JSON với `serde_json::json!` macro + trailing newline (cùng pattern P004 parse-report).
   - **KHÔNG** dùng `process::exit` trong `run()` — bubble qua `anyhow::Result`. main.rs map.

3. **`src/main.rs` error mapping:**
   - Verify `Commands::Dedup` clap variant tồn tại + có `state: PathBuf` + `rows_json: PathBuf` (Anchor #10). Nếu thiếu/khác → STOP escalate.
   - Update dispatch arm:
     ```rust
     Commands::Dedup { state, rows_json } => {
         if let Err(e) = cli::dedup::run(state, rows_json) {
             let code = if e.is::<crate::state::StateReadError>() {
                 1
             } else {
                 2
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```
   - Tail `Ok(())` REQUIRED (per P004 Turn 1 O1.1 precedent — match-arm uniformity với `fn main() -> anyhow::Result<()>`).

4. **Fixture + integration test:**
   - `tests/fixtures/state-3ids.json` — state file với 3 IDs.
   - `tests/fixtures/rows-5.json` — rows envelope với 5 row, 2 advisory_id match state IDs.
   - `tests/dedup_cli.rs` — integration test cover acceptance:
     - Happy: 3 kept + 2 skipped + 5 observed_ids, exit 0.
     - State missing: `--state /nonexistent.json` → exit 1, stderr contains "state".
     - Schema mismatch: fixture với `schema_version: 99` → exit 1, stderr contains "schema_version" hoặc "migrate-state".
     - Rows malformed: fixture không có key `rows` → exit 2, stderr contains "rows".

#### Why typed `StateReadError` (vs raw `anyhow`)?

Spawn-prompt + ARCHITECTURE §1 exit code contract requires "state file unreadable" → exit 1 (distinct từ "rows malformed" → exit 2). main.rs cần downcast để map. Options đã consider:

- **A (typed `StateReadError` enum trong `state.rs`):** clear concrete error type, easy `e.is::<StateReadError>()` downcast. **Chọn.** Pattern khớp P004 `SentinelError` / `RowParseError`.
- **B (anyhow context string match):** fragile (depends on Display wording). Reject.
- **C (return `Result<StateFile, std::io::Error>`):** không cover schema mismatch case (which is a parsed-JSON failure, not IO). Reject.

Spec:
```rust
#[derive(Error, Debug)]
pub enum StateReadError {
    #[error("state file `{path}` unreadable: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("state file `{path}` malformed JSON: {source}")]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("state file `{path}` schema_version {found} != expected {expected} — run `advisory-inbox migrate-state --state {path}`")]
    SchemaMismatch { path: PathBuf, found: u32, expected: u32 },
}
```

3 variant cover: file missing/permission (Io), bad JSON (Json), wrong schema (SchemaMismatch). `migrate-state` hint trong SchemaMismatch — phiếu P007 wire-up. Worker không cần wait P007: hint là pure text, KHÔNG invoke migrate-state binary.

#### Why `RowsEnvelope` wrapper struct (vs accept flat array)?

Spawn-prompt: "Architect picks: accept wrapped object `{ "rows": [...] }` (consistent với parse-report output), error if not."

Rationale:
- P004 emits `{ "rows": [...], "stack_scanned": {}, "advisories_found": N }`. Pipeline: `parse-report | dedup` (future P009 composite). Dedup phải accept exact shape parse-report emits.
- Flat array `[ {...}, {...} ]` cũng có người dùng trực tiếp gọi dedup không qua parse-report — nhưng add support tăng surface (`#[serde(untagged)]` enum hoặc fallback parse), bloat phiếu. Tầng 1 contract: ONE shape, fail-fast on others.
- `#[derive(Deserialize)] struct RowsEnvelope { rows: Vec<AdvisoryRow> }` ignores `stack_scanned` + `advisories_found` (serde default behavior: extra fields silently dropped). Worker không cần allow-unknown-fields config.

#### Why `observed_ids[]` populated regardless of kept/skipped?

Spawn-prompt explicit: "ALL input row advisory_ids (regardless of kept/skipped)". Rationale: P009 scan-and-append sẽ union `observed_ids` vào `state.seen_advisories[]` để bump state forward — even skipped ones still "observed this scan". State's `seen_advisories` grows on EVERY observation, not only kept rows. This guarantees idempotency: re-running dedup with same input always produces same `kept`/`skipped` split (since state grows monotonically).

#### Why `Vec<String>::contains` (vs `BTreeSet` lookup)?

ARCHITECTURE §2 says "Dedup via `BTreeSet` internal" — but that's the storage shape recommendation for state mutation in P009. For P005 read-only filter, linear `Vec::contains` is O(N×M) where N=5 rows, M=3..50 seen IDs. Acceptance fixture: 5×3 = 15 comparisons. Performance non-issue at MVP scale.

Future optimization (P009 or later): convert `seen_advisories` to `HashSet<String>` internally for O(1) lookup. Out-of-scope P005 — tracking via Discovery Report follow-up if Worker observes.

#### `[cli::dedup::run]` signature — `PathBuf` direct, not `Option<PathBuf>`

`Commands::Dedup` clap variant declares `--state <FILE>` + `--rows-json <FILE>` as REQUIRED (ARCHITECTURE §1 documents no default). Anchor #10 must verify P001 scaffold matches. If clap variant has `Option<PathBuf>` → STOP escalate (drift).

### Scope

- CHỈ sửa: `src/cli/dedup.rs` (stub → real impl), `src/state.rs` (add `read()` + `StateReadError` + tests + xoá `#![allow(dead_code)]`), `src/main.rs` (verify dispatch arm; update error → exit code mapping).
- CHỈ tạo: `tests/fixtures/state-3ids.json`, `tests/fixtures/rows-5.json`, `tests/dedup_cli.rs`.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (mark `cli/dedup.rs` + `state::read` wired), `docs/CHANGELOG.md` (entry P005), `README.md` (quick-start `dedup` nếu chưa cover — Worker check Anchor #15).
- KHÔNG sửa: `src/row.rs` (P002/P004 ship, treat read-only), `src/sentinel.rs` (P003), `src/cli/parse_report.rs` (P004), `Cargo.toml` (no new dep — `tempfile` đã có cho P004 atomic-write pattern → Worker verify via Anchor #16, fallback `tempfile` exists per P002 Cargo.toml read).
- KHÔNG tạo: `src/error.rs` (ARCHITECTURE §5 pending — P005 not scope).
- KHÔNG đổi `StateFile` schema (fields, `schema_version: u32`, `seen_advisories: Vec<String>`) — P002 lock; P005 chỉ ADD `read()` helper.
- KHÔNG đổi `AdvisoryRow` (P002 lock).
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 dedup: 0 success, 1 state unreadable, 2 rows malformed).
- KHÔNG touch `state.rs` xa hơn việc add `read()`/`StateReadError`/test + remove allow(dead_code). KHÔNG add `write()` (P009 scope), KHÔNG add `migrate()` (P007 scope).

### Skills consulted

Architect Read `docs/ticket/P004-parse-report.md` để tham khảo pattern wire-in CLI subcmd + anyhow downcast exit-code mapping + integration test idiom (assert_cmd + predicates). P005 đi theo cùng shape — giảm cognitive load cho Worker.

Architect Read `docs/discoveries/P002.md` để biết `#![allow(dead_code)]` của `state.rs` chưa được xoá (Anchor #14 P002 Discovery item — "cần xóa khi P005 import StateFile"). P005 thực hiện xóa.

Architect Read `docs/discoveries/P004.md` để học `serde_json::json!` macro alphabetizes keys ở stdout output — test assertion phải dùng `predicate::str::contains` substring match, KHÔNG exact equality.

Architect KHÔNG dùng context7 cho `serde_json` (well-known API) hay `tempfile` (Worker verify direct trong test if cần). Cargo.toml dep list đã được Architect Read trực tiếp khi check P004 Anchors #4/#5/#7/#8/#9 — `tempfile = "3"` confirmed line 23 — marked `[verified]` for Anchor #16.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/dedup.rs` hiện là stub printf TODO (P001 ship). Signature có thể đã là `pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()>` per P001 scaffold pattern. | P001 Discovery Report + analogy with P004 Anchor #1 (parse_report stub had correct signature already) | `[needs Worker verify]` | ⏳ TO VERIFY — Worker grep `src/cli/dedup.rs` Task 0. |
| 2 | `src/state.rs` exports `pub struct StateFile` (4 field: `schema_version: u32`, `last_scan_at: DateTime<Utc>`, `seen_advisories: Vec<String>`, `agent_version: String`) + `pub const SCHEMA_VERSION: u32 = 1`. P002 shipped. | P002 phiếu spec + P002 Discovery Anchor #12 + ARCHITECTURE §2 dòng 130-146 | `[unverified]` | ⏳ TO VERIFY — Worker grep `pub struct StateFile\|pub const SCHEMA_VERSION` `src/state.rs` Task 0. |
| 3 | `src/state.rs` KHÔNG có `pub fn read` hoặc `pub fn write` (P002 chỉ ship types). | P002 phiếu Scope section + P002 Discovery "types only, not yet wired into subcmd logic" | `[unverified]` | ⏳ TO VERIFY — Worker grep `pub fn ` `src/state.rs` expect 0 hits ngoài trong `#[cfg(test)]`. |
| 4 | `src/state.rs` có `#![allow(dead_code)]` ở module-level (per P002 Discovery dòng 49 + Anchor #14 P002). | P002 Discovery Report dòng 49 + P004 Discovery Follow-up #1 ("state.rs keeps the attribute until P005 dedup wire-in") | `[unverified]` | ⏳ TO VERIFY — Worker `head -5 src/state.rs` expect `#![allow(dead_code)]` at line ~1-3. |
| 5 | `src/row.rs` exports `pub struct AdvisoryRow` (8 field) + `Status` + `Severity`. `AdvisoryRow` derives `Serialize + Deserialize` (P002). | P002 phiếu + P004 Anchor #2 ("Confirmed at row.rs:14-54") | `[verified]` | ✅ Confirmed via P004 Discovery Anchor #2 transitive check. |
| 6 | `src/main.rs` có `Commands::Dedup { state: PathBuf, rows_json: PathBuf }` clap variant + dispatch arm `cli::dedup::run(state, rows_json)`. P001 ship + P004 verified main.rs structure intact (8 dispatch arms). | P004 Anchor #10 confirmed all dispatch arms present | `[needs Worker verify]` | ⏳ TO VERIFY — Worker grep `Commands::Dedup\|Dedup {` `src/main.rs`. |
| 7 | `Cargo.toml` `[dev-dependencies]` có `assert_cmd = "2"` + `predicates = "3"` (Architect Read line 27/28 trước đây cho P004 Anchor #4/#5). | P004 Discovery Anchor #4/#5 re-confirmed | `[verified]` | ✅ Cargo.toml dev-deps unchanged sau P004 ship (no Cargo.toml diff in P004 per scope). |
| 8 | `Cargo.toml` `[dependencies]` có `anyhow = "1"` + `serde_json = "1"` + `thiserror = "2"` + `serde` (with derive) + `chrono` (with serde feature). | P004 Discovery Anchor #7/#8/#9 + P002 Discovery Anchor #1 | `[verified]` | ✅ Cargo.toml unchanged sau P004. |
| 9 | `Cargo.toml` `[dependencies]` có `tempfile = "3"` (cho atomic-write pattern future P006, available now cho unit test trong P005 state::read tests). | P002 phiếu Cargo.toml dep list spec | `[unverified]` | ⏳ TO VERIFY — Worker `grep -n "^tempfile" Cargo.toml`. Nếu missing → unit test có thể dùng `std::env::temp_dir()` fallback (less clean nhưng OK Tầng 2). |
| 10 | `Commands::Dedup` clap variant declares `--state <FILE>` + `--rows-json <FILE>` REQUIRED (kebab-case `--rows-json` matches ARCHITECTURE §1). | ARCHITECTURE §1 dedup subcmd block + P001 scaffold ship | `[needs Worker verify]` | ⏳ TO VERIFY — Worker `cargo run -- dedup --help` expect `--state <STATE>` `--rows-json <ROWS_JSON>` present. |
| 11 | `tests/fixtures/` directory EXISTS sau P004 ship (`tests/fixtures/agent-report-1.md` shipped). | P004 Task 4 + P004 Discovery Anchor #11 | `[verified]` | ✅ Confirmed via P004 Discovery — `mkdir -p tests/fixtures` ran, `agent-report-1.md` present. |
| 12 | `tests/` directory EXISTS sau P004 ship (`tests/parse_report_cli.rs` present). | P004 Task 5 + P004 Discovery | `[verified]` | ✅ Confirmed via P004 Discovery. |
| 13 | `state.rs` test module name = `tests` (P002 convention). Worker phải append vào existing module, KHÔNG tạo `state_read_tests` mod riêng (style consistency). | P002 phiếu spec test pattern + P004 Task 1 note ("nếu trùng → dùng `tests` module và append test") | `[needs Worker verify]` | ⏳ TO VERIFY — Worker `grep -n "mod tests" src/state.rs`. Nếu khác → tự match existing name. |
| 14 | `README.md` chưa có `dedup` subcmd quick-start (P004 Task 6.3 chỉ thêm `parse-report`). | P004 Discovery Anchor #15 + P004 Task 6.3 wording (chỉ parse-report block) | `[unverified]` | ⏳ TO VERIFY — Worker `grep -n "dedup" README.md`. Expect 0 hits (CLI mention) hoặc chỉ trong subcmd list block. |
| 15 | `docs/ARCHITECTURE.md` §1 dedup subcmd block đã document I/O contract đúng (input `--state` + `--rows-json`, output JSON 3 key `kept`/`skipped`/`observed_ids`, exit 0/1/2). | Architect Read ARCHITECTURE.md dòng 34-43 trong load context | `[verified]` | ✅ Dòng 34-43: subcmd spec exact match. No drift. |
| 16 | `Cargo.toml` `[dependencies]` có `tempfile = "3"`. (Anchor #9 already covers; Anchor #16 redundant — REMOVED in V1 final.) | (consolidated into #9) | — | — |

**Note:** Worker EXECUTE Task 0 chạy 8 grep/ls verify (Anchors #1/#2/#3/#4/#6/#9/#10/#13/#14 → 9 anchors needing Worker verify). Nếu BẤT KỲ anchor nào lệch giả định → STOP escalate Discovery Report.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE) và Architect (RESPOND).
> Sếp đọc lúc nghiệm thu. Cap = 3 turns.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no challenges.**

**Anchor verification results:**
- Anchor #1: ✅ `src/cli/dedup.rs` is stub with `pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()>` — matches phiếu expectation exactly.
- Anchor #2: ✅ `StateFile` has all 4 fields (`schema_version: u32`, `last_scan_at: DateTime<Utc>`, `seen_advisories: Vec<String>`, `agent_version: String`) + `pub const SCHEMA_VERSION: u32 = 1` at state.rs:18/26.
- Anchor #3: ✅ `grep -n "^pub fn " src/state.rs` → 0 hits outside cfg(test). No existing `read()` or `write()`.
- Anchor #4: ✅ `#![allow(dead_code)]` is at state.rs line 9.
- Anchor #5: ✅ (pre-verified by Architect via P004 transitive check).
- Anchor #6: ✅ `Commands::Dedup { state: PathBuf, rows_json: PathBuf }` present in main.rs lines 31-38. Dispatch arm at line 112.
- Anchor #7/#8: ✅ (pre-verified by Architect — Cargo.toml unchanged post-P004).
- Anchor #9: ✅ `tempfile = "3"` confirmed at Cargo.toml line 21 (in [dependencies], available to dev tests).
- Anchor #10: ✅ `cargo run -- dedup --help` shows `--state <STATE>` + `--rows-json <ROWS_JSON>` both REQUIRED, no optional wrappers.
- Anchor #11/#12: ✅ (pre-verified via P004 Discovery).
- Anchor #13: ✅ `mod tests` at state.rs line 38. Worker will append into existing module.
- Anchor #14: ✅ `dedup` already in README line 3 (description) + line 37 (quick-start block `advisory-inbox dedup --state ... --rows-json ...`). Per phiếu Task 6.3 conditional: ≥1 hit in real subcmd doc → Worker reviews existing section and updates if wording drifts from ARCHITECTURE §1. Current README line 37 wording matches spec — no update needed.
- Anchor #15: ✅ (pre-verified by Architect — ARCHITECTURE §1 dòng 34-43 exact match).

**No Tầng 1 objections.** All anchors ✅. Baseline test count: 26 lines (≈ 23 tests per phiếu assertion). Code reality matches phiếu spec on all verified anchors.

Ready for Chủ nhà approval gate.

**Status:** ✅ WORKER ACCEPTED V1 — proceeding to EXECUTE

### Final consensus
- Phiếu version: V1
- Total turns: 1 (accepted, no architect response needed)
- Approved: 2026-05-28 — code execution may begin

---

## Debug Log (advisory-inbox specific)

> Worker emit observability records during EXECUTE.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (Sub-mechanism A-F checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | (no hook/cron in this phiếu) | N/A | N/A | N/A |
| B (capability) | `cargo check` | exit 0 | exit 0, 0 warnings | ✅ |
| B (capability) | `cargo test state` | ≥7 tests pass (4 P002 + ≥3 new read()) | 8 tests pass (4 original + 4 new read() cases) | ✅ |
| B (capability) | `cargo test --test dedup_cli` | ≥4 integration tests pass | 4 tests pass | ✅ |
| B (capability) | `cargo run -- dedup --state tests/fixtures/state-3ids.json --rows-json tests/fixtures/rows-5.json` | stdout JSON, `"kept"` length 3, `"skipped"` length 2, `"observed_ids"` length 5, exit 0 | kept=3, skipped=2, observed_ids=5, exit 0 | ✅ |
| C (state schema) | `grep -n "schema_version" src/state.rs` | ≥3 hits (struct field + SCHEMA_VERSION const + validate in read()) | 5+ hits (struct field, const, read() validation, test fixtures) | ✅ |
| C (state schema) | Run dedup against fixture với `schema_version: 99` | exit 1, stderr contains "schema_version" hoặc "migrate-state" | exit 1, stderr: "schema_version 99 != expected 1 — run migrate-state" | ✅ |
| D (persistence) | `grep -l "dedup" docs/ARCHITECTURE.md` | ≥1 hit (§1 + §5) | ≥2 hits (§1 subcmd block + §5 scaffold status P005 line) | ✅ |
| D (persistence) | `grep -l "dedup\|state::read" README.md` | ≥1 hit nếu Anchor #14 yêu cầu add | Full quick-start section added with exit code table | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise bump | no packages updated | ✅ |
| E (env drift) | `cargo build --release` from clean target | exit 0, 0 warnings | Finished release profile, 0 warnings | ✅ |
| F (runtime state) | (no env var read, no token surface) | N/A | N/A | N/A |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + C + D)

**Mục tiêu:** Worker grep + read trạng thái thật TRƯỚC khi viết code.

**Lệnh chạy (verify Anchor #1, #2, #3, #4, #6, #9, #10, #13, #14):**

```bash
# Anchor #1 — dedup stub state
cat src/cli/dedup.rs

# Anchor #2 — StateFile shape
grep -n "pub struct StateFile\|pub const SCHEMA_VERSION\|seen_advisories\|schema_version" src/state.rs

# Anchor #3 — no existing read/write in state.rs (outside #[cfg(test)])
grep -n "^pub fn " src/state.rs

# Anchor #4 — #![allow(dead_code)] location
head -5 src/state.rs

# Anchor #6 — main.rs Dedup variant + dispatch
grep -n "Dedup" src/main.rs

# Anchor #9 — tempfile dep
grep -n "^tempfile" Cargo.toml

# Anchor #10 — dedup clap help
cargo run --quiet -- dedup --help 2>&1 | head -20

# Anchor #13 — state test module name
grep -n "^mod tests\|#\[cfg(test)\]" src/state.rs

# Anchor #14 — README dedup coverage
grep -n "dedup" README.md

# Baseline
cargo check
cargo test --all -- --list | wc -l
```

**Output:** Worker fill kết quả vào Debate Log Turn 1.

**Hard Stop triggers:**
- Anchor #2 — nếu `StateFile` field naming khác P002 spec (`schema_version` / `last_scan_at` / `seen_advisories` / `agent_version`) → STOP escalate.
- Anchor #3 — nếu `state.rs` đã có `pub fn read` từ trước → STOP escalate (P002 drift; Architect didn't expect).
- Anchor #6 — nếu `Commands::Dedup` không tồn tại hoặc field naming khác → STOP escalate (P001 drift).
- Anchor #10 — nếu `--state` hoặc `--rows-json` không phải REQUIRED flag → STOP escalate (drift ARCHITECTURE §1).
- Anchor #9 — nếu `tempfile` missing → Worker may use `std::env::temp_dir()` + manual cleanup trong unit test (Tầng 2 self-decide, OK).

### Task 1: Mở rộng `src/state.rs` — `read()` + `StateReadError` + tests + xoá `#![allow(dead_code)]`

**File:** `src/state.rs`

**Mục tiêu:**
1. Add `pub enum StateReadError` (3 variant via `thiserror`).
2. Add `pub fn read(path: &Path) -> Result<StateFile, StateReadError>`.
3. Xoá `#![allow(dead_code)]` ở đầu file (Anchor #4 confirms present).
4. Add ≥3 unit test cho `read()` (happy / missing / schema mismatch).

**Skeleton:**

```rust
// (Xoá #![allow(dead_code)] dòng đầu — Task 1.3)

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;

// ... (giữ StateFile struct từ P002 — KHÔNG đổi field naming/typing)

/// Errors returned by [`read`] when the state file cannot be loaded.
#[derive(Error, Debug)]
pub enum StateReadError {
    #[error("state file `{path}` unreadable: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("state file `{path}` malformed JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("state file `{path}` schema_version {found} != expected {expected} — run `advisory-inbox migrate-state --state {path}`")]
    SchemaMismatch {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
}

/// Read + validate a state file from disk.
///
/// Returns [`StateReadError::Io`] if the file is missing/unreadable,
/// [`StateReadError::Json`] if the contents are not valid JSON for [`StateFile`],
/// and [`StateReadError::SchemaMismatch`] if `schema_version != SCHEMA_VERSION`.
///
/// The schema-mismatch error includes a `migrate-state` hint pointing at the
/// same path so the operator can run the migration subcommand directly.
pub fn read(path: &Path) -> Result<StateFile, StateReadError> {
    let bytes = std::fs::read(path).map_err(|source| StateReadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed: StateFile = serde_json::from_slice(&bytes).map_err(|source| StateReadError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    if parsed.schema_version != SCHEMA_VERSION {
        return Err(StateReadError::SchemaMismatch {
            path: path.to_path_buf(),
            found: parsed.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    // ... (P002 existing tests — KHÔNG đổi)

    fn write_tempfile(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write fixture");
        f
    }

    #[test]
    fn read_happy_path() {
        let json = r#"{
            "schema_version": 1,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": ["CVE-2026-9256", "GHSA-xxxx-yyyy"],
            "agent_version": "advisory-watch@0.1.0"
        }"#;
        let f = write_tempfile(json);
        let state = read(f.path()).expect("read ok");
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.seen_advisories.len(), 2);
        assert_eq!(state.agent_version, "advisory-watch@0.1.0");
    }

    #[test]
    fn read_missing_file_errors() {
        let err = read(Path::new("/nonexistent/advisory-state.json")).unwrap_err();
        assert!(matches!(err, StateReadError::Io { .. }));
    }

    #[test]
    fn read_schema_mismatch_errors() {
        let json = r#"{
            "schema_version": 99,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": [],
            "agent_version": "x"
        }"#;
        let f = write_tempfile(json);
        let err = read(f.path()).unwrap_err();
        match err {
            StateReadError::SchemaMismatch { found, expected, .. } => {
                assert_eq!(found, 99);
                assert_eq!(expected, 1);
            }
            other => panic!("expected SchemaMismatch, got {other:?}"),
        }
        // Error Display must hint migrate-state per ARCHITECTURE §1 future flow
        let msg = format!("{}", read(f.path()).unwrap_err());
        assert!(msg.contains("migrate-state"));
    }

    #[test]
    fn read_malformed_json_errors() {
        let json = r#"{"schema_version": 1, "broken-json"#;
        let f = write_tempfile(json);
        let err = read(f.path()).unwrap_err();
        assert!(matches!(err, StateReadError::Json { .. }));
    }
}
```

**Lưu ý:**
- **Xoá `#![allow(dead_code)]`** ở đầu file. Sau xóa, `StateFile` + `SCHEMA_VERSION` được consumed bởi `read()` (new pub fn) + `cli::dedup::run()` (P005) → clippy `dead_code` clean.
- `read()` trả `Result<StateFile, StateReadError>` — concrete type, KHÔNG `anyhow::Result`. `cli::dedup::run()` sẽ `?`-propagate; anyhow auto-wraps qua `From<StateReadError>` blanket impl. main.rs `e.is::<StateReadError>()` để downcast → exit 1.
- `StateReadError` derives `Error + Debug` only (KHÔNG `PartialEq + Eq` vì `std::io::Error` không impl `PartialEq`). Test dùng `matches!()` macro thay `assert_eq!()`.
- Test module name = `tests` (Anchor #13 — Worker confirm match P002 existing). Nếu P002 dùng tên khác → match existing tên, append vào.
- `tempfile::NamedTempFile::new()` từ `tempfile = "3"` (Anchor #9). Nếu missing → fallback `std::env::temp_dir().join(format!("dedup-test-{}.json", uuid))` — Tầng 2 self-decide; thông báo Discovery Report.
- KHÔNG add `pub fn write()` / `pub fn migrate()` — P005 scope ONLY read().

### Task 2: Wire-in `src/cli/dedup.rs`

**File:** `src/cli/dedup.rs`

**Tìm** (P001 stub — Worker verify exact wording qua Anchor #1; expect signature có thể đã là `pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()>` body printf TODO):

```rust
pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!("TODO: dedup (state={:?} rows_json={:?}) — wired in P005", state, rows_json);
    Ok(())
}
```

(Worker confirm exact wording; phiếu signature based on P004 stub precedent.)

**Thay bằng (full file content):**

```rust
//! `advisory-inbox dedup` — filter parse-report rows against state.seen_advisories.
//!
//! See ARCHITECTURE.md §1 subcmd `dedup` for the I/O contract.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::row::AdvisoryRow;
use crate::state;

/// JSON envelope emitted by `parse-report` (subset — extra fields ignored).
///
/// Accepts shape `{ "rows": [...], ... }`. Flat-array input is NOT supported
/// (per Constraint #4 — Tầng 1 contract: ONE input shape).
#[derive(Deserialize)]
struct RowsEnvelope {
    rows: Vec<AdvisoryRow>,
}

/// Read state + rows JSON, partition rows by `state.seen_advisories` membership,
/// emit `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` to stdout.
///
/// `observed_ids` carries every input row's `advisory_id` regardless of kept/skipped —
/// downstream consumers (e.g., `scan-and-append`) union this into state for the
/// next scan's seen set.
///
/// # Errors
/// - [`state::StateReadError`] (mapped to exit 1 in `main.rs`) — file unreadable,
///   malformed JSON, or schema_version mismatch.
/// - I/O + JSON errors on `rows_json` → anyhow bubble (mapped to exit 2 in main.rs).
pub fn run(state_path: PathBuf, rows_json: PathBuf) -> Result<()> {
    // 1. Read + validate state.
    let state = state::read(&state_path)?;

    // 2. Read rows envelope.
    let rows_text = std::fs::read_to_string(&rows_json)
        .with_context(|| format!("read rows file {}", rows_json.display()))?;
    let envelope: RowsEnvelope = serde_json::from_str(&rows_text)
        .with_context(|| format!("parse rows JSON from {}", rows_json.display()))?;

    // 3. Partition.
    let mut kept: Vec<AdvisoryRow> = Vec::new();
    let mut skipped: Vec<AdvisoryRow> = Vec::new();
    let mut observed_ids: Vec<String> = Vec::with_capacity(envelope.rows.len());
    for row in envelope.rows {
        observed_ids.push(row.advisory_id.clone());
        if state.seen_advisories.contains(&row.advisory_id) {
            skipped.push(row);
        } else {
            kept.push(row);
        }
    }

    // 4. Emit output JSON + trailing newline.
    let out = serde_json::json!({
        "kept": kept,
        "skipped": skipped,
        "observed_ids": observed_ids,
    });
    serde_json::to_writer(std::io::stdout().lock(), &out)
        .context("write stdout JSON")?;
    println!();
    Ok(())
}
```

**Lưu ý:**
- Function arg `state_path: PathBuf` (rename từ `state` để tránh shadow `state` module import). Verify clap binding trong main.rs Task 3 pass `state` field value vào `state_path` arg.
- `state::read(&state_path)?` propagates `StateReadError` qua `anyhow::Error` (thiserror types implement `std::error::Error + Send + Sync + 'static` → `anyhow::Error::from()` accepts).
- `RowsEnvelope` defined inline trong dedup.rs vì chỉ dùng tại đây. Nếu future P009 cần re-use → move lên `cli/mod.rs` hoặc `row.rs` — out-of-scope P005.
- `state.seen_advisories.contains(&row.advisory_id)` — `Vec<String>::contains(&String)` works via `PartialEq<String> for String`. O(N×M) acceptable tại MVP scale (see Giải pháp).
- `observed_ids.push(row.advisory_id.clone())` — clone vì sau đó `row` consumed bởi `kept`/`skipped` push. Performance: 5 row × 1 clone = trivial.
- `serde_json::json!` macro emits keys alphabetical (per P004 Discovery — `kept` → `observed_ids` → `skipped` order in actual stdout). Test assertion dùng `predicate::str::contains` substring match.
- KHÔNG `process::exit` trong `run()` — bubble qua `Result`.
- KHÔNG add `--dry-run` flag / `--quiet` flag — out-of-scope.

### Task 3: Update `src/main.rs` — dispatch error → exit code map

**File:** `src/main.rs`

**Mục tiêu 2 phần:**

**3.1. Verify `Commands::Dedup` clap variant intact:**

**Tìm** (Worker `grep -n "Dedup" src/main.rs` ngay Task 0; expect existing):

```rust
Dedup {
    #[arg(long)]
    state: PathBuf,
    #[arg(long = "rows-json")]
    rows_json: PathBuf,
},
```

→ Nếu match → KHÔNG đổi struct.
→ Nếu thiếu hoặc field naming khác → STOP escalate (P001 drift — ARCHITECTURE §1 contract).

**3.2. Cập nhật dispatch arm — error → exit code map:**

**Tìm** (real existing line — Worker grep verify):
```rust
Commands::Dedup { state, rows_json } => cli::dedup::run(state, rows_json),
```

**Thay bằng:**
```rust
Commands::Dedup { state, rows_json } => {
    if let Err(e) = cli::dedup::run(state, rows_json) {
        let code = if e.is::<crate::state::StateReadError>() {
            1
        } else {
            2
        };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

**Lưu ý:**
- **Tail `Ok(())` REQUIRED** (per P004 Turn 1 O1.1 precedent — match-arm uniformity với `fn main() -> anyhow::Result<()>`). `std::process::exit` diverging (`!`) → `Ok(())` unreachable on err path; present for type checker on happy path.
- Anyhow downcast 2 branches collapsed (avoid clippy `if_same_then_else` per P004 Discovery): `StateReadError` → 1, else → 2. Sentinel/Row errors KHÔNG xuất hiện trong dedup pipeline (parse đã done upstream), nên check 1 concrete error đủ.
- `serde_json::Error` (rows JSON malformed) + `std::io::Error` (rows file missing) fallthrough → exit 2 ("rows malformed/unreadable" — ARCHITECTURE §1 dedup exit 2).
- KHÔNG đổi other dispatch arm. ParseReport (P004) dispatch giữ nguyên.
- `use std::path::PathBuf` — already imported sau P004 (Anchor #6 + P004 confirmed). Verify; nếu missing → add.

### Task 4: Tạo fixtures

**File 4.1:** `tests/fixtures/state-3ids.json` (new)

```json
{
  "schema_version": 1,
  "last_scan_at": "2026-05-28T09:51:35Z",
  "seen_advisories": [
    "CVE-2026-9256",
    "GHSA-aaaa-bbbb",
    "CVE-2026-27205"
  ],
  "agent_version": "advisory-watch@0.1.0"
}
```

**File 4.2:** `tests/fixtures/rows-5.json` (new)

```json
{
  "rows": [
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-2026-NEW1",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-NEW1",
      "package": "next@<15.5.17",
      "file_line": "src/middleware.ts:42",
      "severity": "High",
      "status": "open",
      "note": "-"
    },
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-2026-9256",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-9256",
      "package": "lodash@<4.17.22",
      "file_line": "package.json:14",
      "severity": "Medium",
      "status": "open",
      "note": "-"
    },
    {
      "date": "2026-05-28",
      "advisory_id": "GHSA-aaaa-bbbb",
      "source_url": "https://github.com/advisories/GHSA-aaaa-bbbb",
      "package": "flask@<2.3.5",
      "file_line": "astro-service/app.py:8",
      "severity": "Medium",
      "status": "open",
      "note": "-"
    },
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-2026-NEW2",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-NEW2",
      "package": "django@<4.2.7",
      "file_line": "requirements.txt:3",
      "severity": "Critical",
      "status": "open",
      "note": "-"
    },
    {
      "date": "2026-05-28",
      "advisory_id": "CVE-2026-NEW3",
      "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-NEW3",
      "package": "ruby@<3.2.5",
      "file_line": "Gemfile:21",
      "severity": "Low",
      "status": "open",
      "note": "-"
    }
  ],
  "stack_scanned": {},
  "advisories_found": 5
}
```

**File 4.3 (optional — Worker can inline-build trong integration test instead):** `tests/fixtures/state-schema-mismatch.json` (new — only nếu Worker decide tách file thay vì inline-write trong test).

**Lưu ý:**
- 5 row trong `rows-5.json`: 2 match state (`CVE-2026-9256` + `GHSA-aaaa-bbbb`) → skipped; 3 không match (`CVE-2026-NEW1/NEW2/NEW3`) → kept. Acceptance: `kept: 3, skipped: 2`.
- `observed_ids` từ dedup output sẽ chứa cả 5 IDs (theo input order).
- `stack_scanned: {}` + `advisories_found: 5` trong rows-5.json — serde_json sẽ silently ignore (RowsEnvelope chỉ deserialize `rows` key).
- Field naming trong row JSON khớp `AdvisoryRow` serde derive (P002): `date`, `advisory_id`, `source_url`, `package`, `file_line`, `severity`, `status`, `note`. `severity` PascalCase value, `status` lowercase value (per P002 serde rename_all).
- KHÔNG add row với severity sai / status sai trong rows-5.json — happy-path fixture. Malformed scenarios inline-built trong dedup_cli.rs Task 5.
- Worker preserve trailing newline cuối file (POSIX convention).

### Task 5: Tạo integration test `tests/dedup_cli.rs`

**File:** `tests/dedup_cli.rs` (new)

**Nội dung (Worker write skeleton — 4 test case):**

```rust
//! Integration tests for `advisory-inbox dedup` subcmd.
//!
//! Covers: happy path (3 kept + 2 skipped + 5 observed_ids), state file missing
//! (exit 1), schema_version mismatch (exit 1), rows JSON malformed (exit 2).

use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;

const STATE_3IDS: &str = "tests/fixtures/state-3ids.json";
const ROWS_5: &str = "tests/fixtures/rows-5.json";

#[test]
fn dedup_happy_path_3_kept_2_skipped() {
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state").arg(STATE_3IDS)
        .arg("--rows-json").arg(ROWS_5)
        .assert()
        .success()
        // 3 kept: CVE-2026-NEW1, CVE-2026-NEW2, CVE-2026-NEW3
        .stdout(predicate::str::contains("CVE-2026-NEW1"))
        .stdout(predicate::str::contains("CVE-2026-NEW2"))
        .stdout(predicate::str::contains("CVE-2026-NEW3"))
        // 2 skipped: CVE-2026-9256, GHSA-aaaa-bbbb
        .stdout(predicate::str::contains("CVE-2026-9256"))
        .stdout(predicate::str::contains("GHSA-aaaa-bbbb"))
        // observed_ids includes all 5 (JSON key match)
        .stdout(predicate::str::contains(r#""observed_ids""#))
        .stdout(predicate::str::contains(r#""kept""#))
        .stdout(predicate::str::contains(r#""skipped""#));
}

#[test]
fn dedup_state_missing_exit_1() {
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state").arg("/nonexistent/advisory-state.json")
        .arg("--rows-json").arg(ROWS_5)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("state file"));
}

#[test]
fn dedup_state_schema_mismatch_exit_1() {
    // Inline-build a schema-99 state file via tempfile.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    let bad_state = r#"{
        "schema_version": 99,
        "last_scan_at": "2026-05-28T09:51:35Z",
        "seen_advisories": [],
        "agent_version": "x"
    }"#;
    tmp.write_all(bad_state.as_bytes()).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state").arg(tmp.path())
        .arg("--rows-json").arg(ROWS_5)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("schema_version").or(predicate::str::contains("migrate-state")));
}

#[test]
fn dedup_rows_malformed_exit_2() {
    // Inline-build a rows file without the "rows" key.
    let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
    tmp.write_all(br#"{"not_rows": []}"#).expect("write tmp");

    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("dedup")
        .arg("--state").arg(STATE_3IDS)
        .arg("--rows-json").arg(tmp.path())
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("rows"));
}
```

**Lưu ý:**
- 4 integration test cover acceptance + 3 error path.
- `tempfile = "3"` dev-dep (Anchor #9). Nếu missing → fallback `std::env::temp_dir()` (Tầng 2 self-decide; thông báo Discovery).
- `predicate::str::contains(...).or(predicate::str::contains(...))` — predicate logical OR (predicates crate API). Worker verify exact predicates `or` method idiom via `cargo doc predicates`.
- Fixture path constants `STATE_3IDS` + `ROWS_5` — relative tới crate root khi `assert_cmd` invoke binary (cwd = crate root by default).
- `--state` `--rows-json` flag passing qua `.arg("--state").arg(STATE_3IDS)` 2-step (clap handles space-separated). Nếu Worker prefer `=` syntax (`--state=path`) → OK, equivalent.
- stderr assertion dùng substring match. Worker verify thực tế wording `StateReadError::Io` Display = `"state file `<path>` unreadable: ..."` → "state file" substring match.
- KHÔNG add test cho `state.rs` `read()` ở đây (đã test trong Task 1 unit). Integration test focus subcmd end-to-end behavior.

### Task 6: Docs Gate updates

**File 6.1:** `docs/ARCHITECTURE.md` — §5 Scaffold status

**Tìm** (block sau P004 Discovery cập nhật):
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic, not yet wired into `cli/parse_report.rs`.
- P004: `cli/parse_report.rs` wired (stdin/`--input` → sentinel → row → JSON stdout); `row::parse_row` + `RowParseError` + `FromStr` for `Status`/`Severity` shipped.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Thay bằng:**
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic, not yet wired into `cli/parse_report.rs`.
- P004: `cli/parse_report.rs` wired (stdin/`--input` → sentinel → row → JSON stdout); `row::parse_row` + `RowParseError` + `FromStr` for `Status`/`Severity` shipped.
- P005: `cli/dedup.rs` wired (state + rows JSON → kept/skipped/observed_ids JSON stdout); `state::read` + `StateReadError` (Io/Json/SchemaMismatch) shipped.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Lưu ý:**
- Date heading `2026-05-28` giữ nguyên (snapshot baseline).
- KHÔNG đổi §1 CLI surface (dedup contract đã document đúng từ trước — không drift).
- KHÔNG đổi §2 State schema (P005 không touch `StateFile` shape, chỉ add `read()` helper).

**File 6.2:** `docs/CHANGELOG.md` — entry P005

**Thêm entry (newest at top, theo convention P001-P004):**

```markdown
## P005 — dedup subcmd (2026-MM-DD)

**Type:** feat | **Tầng:** 1 | **Lane:** Normal

- Wire `src/cli/dedup.rs` — `run(state: PathBuf, rows_json: PathBuf) -> anyhow::Result<()>` reads state via `state::read`, deserializes rows envelope `{ "rows": [...] }`, partitions rows into `kept`/`skipped` against `state.seen_advisories`, emits JSON `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` to stdout. `observed_ids` carries every input row's `advisory_id` regardless of kept/skipped.
- Extend `src/state.rs`: add `pub fn read(&Path) -> Result<StateFile, StateReadError>` (read + parse + schema_version validate). Add `pub enum StateReadError` (Io/Json/SchemaMismatch, all via `thiserror`). SchemaMismatch Display hints `advisory-inbox migrate-state` (P007 wire-up).
- Remove `#![allow(dead_code)]` from `src/state.rs` (consumer wire-in complete: `read()` + `cli::dedup`).
- `src/main.rs`: dispatch `Commands::Dedup { state, rows_json }` maps `StateReadError` → exit 1, other → exit 2; anyhow cause chain printed to stderr.
- New fixtures `tests/fixtures/state-3ids.json` + `tests/fixtures/rows-5.json` (5 row, 2 match).
- New integration test `tests/dedup_cli.rs` (4 cases: happy 3-kept/2-skipped, state missing → 1, schema mismatch → 1, rows malformed → 2).
- New unit tests in `src/state.rs`: `read_happy_path`, `read_missing_file_errors`, `read_schema_mismatch_errors`, `read_malformed_json_errors`.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)
```

**Lưu ý:**
- `2026-MM-DD` → Worker thay ngày ship thực.
- Newest at top — append above P004 entry.

**File 6.3:** `README.md` quick-start (CONDITIONAL — depending Anchor #14)

**If** `grep -n "dedup" README.md` returns 0 hits → add quick-start section sau `parse-report` block:

**Thêm (new section):**
````markdown
### Quick start — dedup against state

Filter parsed rows against `seen_advisories[]` in a state file:

```bash
advisory-inbox dedup --state .advisory-scan-state --rows-json rows.json
# → { "kept": [...], "skipped": [...], "observed_ids": [...] }
```

- `kept` — rows whose `advisory_id` is NOT yet in state (new advisories).
- `skipped` — rows whose `advisory_id` is already in state (re-observed).
- `observed_ids` — every input row's `advisory_id` (downstream uses this to extend state).

Exit codes:
- `0` — partition succeeded (any number of kept/skipped, including zero)
- `1` — state file missing, malformed JSON, or `schema_version != 1` (run `advisory-inbox migrate-state` to upgrade)
- `2` — rows JSON missing or malformed (expected envelope `{ "rows": [...] }`)
````

**If** `dedup` already mentioned (≥1 hit in real subcmd doc, not just subcmd list) → Worker review existing section + update wording. Otherwise skip.

**Lưu ý:**
- README cập nhật là Tầng 1 (CLI subcmd activated). RULES.md §11 matrix: "CLI subcommand added/removed/renamed → README.md quick-start" — P005 đổi behavior thực từ TODO → real → treated as "subcmd activated".
- Wording phải khớp ARCHITECTURE §1 exit codes (0/1/2) — không drift.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/state.rs` | Task 1: thêm `read()` + `StateReadError` + ≥4 unit test; **xoá `#![allow(dead_code)]`** |
| `src/cli/dedup.rs` | Task 2: stub → real impl (`run(PathBuf, PathBuf) -> anyhow::Result<()>` pipeline) |
| `src/main.rs` | Task 3: dispatch `Commands::Dedup` error → exit code map (StateReadError → 1, else → 2); tail `Ok(())` |
| `tests/fixtures/state-3ids.json` | Task 4.1: new — state with 3 seen IDs |
| `tests/fixtures/rows-5.json` | Task 4.2: new — rows envelope with 5 row (2 match) |
| `tests/dedup_cli.rs` | Task 5: new — 4 integration test |
| `docs/ARCHITECTURE.md` | Task 6.1: §5 Scaffold status thêm P005 line |
| `docs/CHANGELOG.md` | Task 6.2: prepend P005 entry |
| `README.md` | Task 6.3: add `dedup` quick-start nếu chưa có (conditional per Anchor #14) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/row.rs` | Treat read-only; KHÔNG đổi `AdvisoryRow`/`Status`/`Severity`/`parse_row`/`RowParseError`. P005 chỉ consume. |
| `src/sentinel.rs` | KHÔNG touch. |
| `src/cli/parse_report.rs` | P004 ship, KHÔNG đổi. P005 chỉ accept output JSON shape. |
| `src/cli/append.rs` | Vẫn stub TODO — P006 wire-in. |
| `src/cli/{migrate_state,state_backfill,scan_and_append,serve,init}.rs` | Vẫn stub TODO. |
| `Cargo.toml` | `tempfile` (Anchor #9) / `anyhow` / `serde_json` / `thiserror` / `assert_cmd` / `predicates` đã có. KHÔNG thêm dep mới. |
| `CLAUDE.md` | KHÔNG đổi doctrine. |
| `docs/RULES.md` | KHÔNG đổi. |
| `docs/PROJECT.md` | Phase status không đổi (Phase 1 in-progress). |
| `docs/BACKLOG.md` | Worker không tự strikethrough P005 — orchestrator/Sếp xử lý "Recently shipped" section post-merge. |

---

## Luật chơi (Constraints)

1. **No new deps.** `tempfile = "3"` (Anchor #9) + `anyhow`/`serde_json`/`thiserror`/`assert_cmd`/`predicates` đã có. KHÔNG add `serde_yaml`, `clap_complete`, hoặc HashSet alternative crate.
2. **No `unsafe { ... }` block** — pure safe Rust.
3. **No `process::exit` ngoài `src/main.rs`** — `cli/dedup.rs` bubble `anyhow::Result`. Reason: testability + separation.
4. **Input JSON shape cố định** — `{ "rows": [...] }` envelope ONLY. KHÔNG accept flat array `[ ... ]` (Tầng 1 contract). Extra fields (`stack_scanned`/`advisories_found`) silently ignored bởi serde. Future flat-array support → new phiếu.
5. **Output JSON shape cố định** — `{ "kept": ..., "skipped": ..., "observed_ids": ... }` đúng 3 field, đúng tên (ARCHITECTURE §1 contract). KHÔNG add `kept_count` / `skipped_count` / `state_path` echo — Tầng 1 contract.
6. **Exit code semantics cố định** — 0 success, 1 state unreadable (Io/Json/SchemaMismatch), 2 rows malformed. KHÔNG add code 3+.
7. **`StateFile` schema locked** (P002 ship). P005 chỉ ADD `read()` helper + `StateReadError`. KHÔNG đổi field naming/typing.
8. **`observed_ids` includes ALL input row advisory_ids** (kept + skipped + duplicates if any). KHÔNG dedup observed_ids — downstream consumer (P009) handle dedup khi union vào state.
9. **`Vec::contains` for membership check** (O(N×M) acceptable at MVP scale). KHÔNG promote `seen_advisories` to `HashSet` trong P005 — out-of-scope refactor (Discovery Report can flag for future).
10. **`#![allow(dead_code)]` chỉ xoá khỏi `state.rs`.** KHÔNG touch nơi khác (P004 đã xoá khỏi `row.rs`; `sentinel.rs` có 1 attribute stale — P004 Discovery follow-up #1 — xử lý ở phiếu housekeeping riêng, KHÔNG trong P005).
11. **`anyhow::Error::is::<T>()` downcast — KHÔNG chain extra error types.** Chỉ check 1 concrete error: `StateReadError`. Other error (IO trên rows file, JSON parse rows envelope, write stdout) fallthrough to exit 2 — đúng spec ARCHITECTURE §1.
12. **`migrate-state` hint trong SchemaMismatch là pure text.** KHÔNG invoke binary, KHÔNG block dedup pipeline, KHÔNG depend P007 ship trước.
13. **Conventional commits** — `feat(P005): wire dedup subcmd` (match P001-P004 pattern; Worker grep `git log --oneline -5` confirm).
14. **Docs Gate Tầng 1 mandatory** — CLI subcmd activated. ARCHITECTURE §5 + CHANGELOG + README (conditional) bắt buộc.
15. **`tempfile = "3"` test idiom — Worker may fallback** `std::env::temp_dir()` nếu Anchor #9 fail (Tầng 2 self-decide). Thông báo Discovery.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (≥23 baseline P004 + 4 new state + 4 new dedup_cli = ≥31 tests total)
- [ ] `cargo test state` — ≥8 tests (4 P002 + ≥4 mới `read()` cases)
- [ ] `cargo test --test dedup_cli` — 4 integration tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (sau khi xoá `#![allow(dead_code)]` state.rs, không có dead_code warning vì consumer wire-in)
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] Happy path: `cargo run -- dedup --state tests/fixtures/state-3ids.json --rows-json tests/fixtures/rows-5.json` → stdout JSON; pipe vào `jq`: `.kept | length` = 3, `.skipped | length` = 2, `.observed_ids | length` = 5, exit 0.
- [ ] All kept (no overlap): state với 0 IDs + same rows-5 → `.kept | length` = 5, `.skipped | length` = 0.
- [ ] All skipped (full overlap): state với 5 IDs match rows-5 → `.kept | length` = 0, `.skipped | length` = 5.
- [ ] State missing: `cargo run -- dedup --state /nonexistent.json --rows-json tests/fixtures/rows-5.json` → stderr `error: state file ...`, exit 1.
- [ ] Schema mismatch: state với `schema_version: 99` → exit 1, stderr contains `schema_version` + `migrate-state` hint.
- [ ] Rows malformed: rows file thiếu key `rows` → exit 2, stderr contains "rows".
- [ ] Help: `cargo run -- dedup --help` → clap usage hiển thị `--state <STATE>` + `--rows-json <ROWS_JSON>` flag.

### Regression
- [ ] `cargo run -- --help` vẫn show 8 subcmd.
- [ ] Other stub subcmd vẫn exit 0 với TODO message: `cargo run -- append`, `cargo run -- migrate-state`, etc.
- [ ] `cargo run -- parse-report < tests/fixtures/agent-report-1.md` (P004) — JSON output unchanged, exit 0.
- [ ] `cargo test row` — 10 P004 row tests vẫn pass.
- [ ] `cargo test sentinel` — 6 P003 test vẫn pass.
- [ ] `cargo test --test parse_report_cli` — 3 P004 integration tests vẫn pass.

### Docs Gate
- [ ] `docs/ARCHITECTURE.md` §5 — Scaffold status thêm P005 line (Task 6.1).
- [ ] `docs/CHANGELOG.md` — P005 entry prepended (Task 6.2), date filled.
- [ ] `README.md` — quick-start `dedup` section present + exit code table đúng (Task 6.3 conditional).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P005.md` — full report written. **MUST include:**
  - Anchor verification results (table — 15 anchor).
  - Confirmation `Commands::Dedup` clap variant signature thực tế (Anchor #6 outcome — match phiếu spec or drift).
  - Confirmation `#![allow(dead_code)]` đã xoá khỏi `state.rs` + `cargo clippy` không phát sinh `dead_code` warning.
  - Confirmation `tempfile = "3"` available (Anchor #9 result; nếu fallback used, document why + how).
  - Test count: 23 baseline (P004) + ≥8 new = ≥31 total.
  - `serde_json::json!` output key ordering (alphabetical per P004 Discovery — actual stdout order is `kept` → `observed_ids` → `skipped`).
  - Any drift / unexpected behavior phát hiện trong `state::read` schema validation hoặc clap arg binding.
  - Sub-mech B + C + D Verification Trace fill đầy đủ.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `- 2026-MM-DD P005: dedup wired (state + rows JSON → kept/skipped/observed_ids), state::read enforces schema_version==1, #![allow(dead_code)] removed from state.rs, anyhow downcast maps exit codes (StateReadError→1, else→2) → see docs/discoveries/P005.md`.
- [ ] Sub-mechanism B + C + D Verification Trace filled (table above).
