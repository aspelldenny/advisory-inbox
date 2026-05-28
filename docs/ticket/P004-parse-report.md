# PHIẾU P004: parse-report subcmd

> **ID format:** `P004` — counter `.phieu-counter` = 4 sau P003 ship.
> **Filename:** `docs/ticket/P004-parse-report.md`
> **Branch:** `feat/P004-parse-report`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P005 dedup — dedup consumes `Vec<AdvisoryRow>` JSON output của parse-report; pipeline đi tiếp parse → dedup → append)
> **Ảnh hưởng:** `src/cli/parse_report.rs` (stub → real impl), `src/row.rs` (add `parse_row` + `RowParseError` + `FromStr` cho `Status`/`Severity`), `src/main.rs` (đảm bảo `Commands::ParseReport` clap variant có `--input`, error handling map exit code), `tests/fixtures/agent-report-1.md` (new fixture), `tests/parse_report_cli.rs` (new integration test), `docs/ARCHITECTURE.md` §5, `docs/CHANGELOG.md`, `README.md` (quick-start nếu chưa cover parse-report)
> **Dependency:** P001 (CLI scaffold), P002 (`AdvisoryRow`/`Status`/`Severity`), P003 (`sentinel::extract_block`) — tất cả đã ship 2026-05-28
> **Lane:** Normal (CLI subcmd wire-in + public API surface mới trong `row.rs` + new test file — Normal per RULES.md §1)
> **Sub-mech áp dụng:** B (capability — `cargo check` + `cargo test`), D (persistence — ARCHITECTURE §5 mark P004 shipped, README quick-start sync nếu chạm)

---

## Context

### Vấn đề hiện tại

P001 ship CLI stub. P002 ship `AdvisoryRow` + `Status` + `Severity`. P003 ship `sentinel::extract_block`. Bây giờ **wire 3 piece này vào subcmd `parse-report`** để emit real JSON output. Đây là first subcmd có logic thật trong advisory-inbox.

Pipeline yêu cầu trong phiếu:
1. Read input — stdin (default) hoặc `--input <FILE>`.
2. `sentinel::extract_block(&text)` → `Vec<String>` raw row lines.
3. Parse mỗi raw line thành `AdvisoryRow` qua function mới `row::parse_row(&str) -> Result<AdvisoryRow, RowParseError>`.
4. Build output JSON `{ "rows": [...], "stack_scanned": {}, "advisories_found": N }` → print stdout.
5. Map error → exit code 1 (missing sentinel) hoặc 2 (parse error) trong `main.rs`.

Stub hiện tại của `cli/parse_report.rs` (P001) chỉ printf TODO. Sau P004:
- `cargo run -- parse-report < fixtures/agent-report-1.md` → JSON stdout, exit 0
- Missing sentinel → stderr error, exit 1
- Bad row format → stderr error, exit 2

Reference BACKLOG.md item P004:
- Scope: Wire `cli/parse_report.rs` → sentinel parse → row parse → JSON stdout.
- Acceptance: `cargo run -- parse-report < fixtures/agent-report-1.md` outputs JSON matching expected.
- Sub-mech checks: B (cargo check), D (docs grep).

### Giải pháp

**4 unit công việc chính:**

1. **`src/row.rs` mở rộng:**
   - Thêm `pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError>` — split line bằng `|`, trim, validate 8 cell, parse từng cell theo type.
   - Thêm `pub enum RowParseError` derive `thiserror::Error + Debug + PartialEq + Eq` với variants: `EmptyLine`, `WrongCellCount { expected: usize, actual: usize }`, `InvalidDate(String)`, `InvalidSeverity(String)`, `InvalidStatus(String)`.
   - Thêm `impl FromStr for Status` + `impl FromStr for Severity` — map string canonical (lowercase cho Status, PascalCase cho Severity) sang enum variant. Trả `RowParseError::InvalidStatus`/`InvalidSeverity` khi không match.
   - **Xoá `#![allow(dead_code)]`** đầu file `src/row.rs` (per P002 Discovery Report — xoá khi consumer wire-in). Sau P004, `AdvisoryRow`/`Status`/`Severity` đều có consumer (parse_row + cli/parse_report). `state.rs` vẫn giữ `#![allow(dead_code)]` (P005 sẽ xoá khi dedup import).
   - Thêm ≥3 unit test cho `parse_row`: happy path, bad date, bad severity. Thêm ≥1 test mỗi FromStr impl (lowercase Status, PascalCase Severity, unknown variant).

2. **`src/cli/parse_report.rs` real impl:**
   - Replace stub printf bằng `pub fn run(input: Option<PathBuf>) -> anyhow::Result<()>`.
   - Đọc text: nếu `input.is_some()` → `std::fs::read_to_string(path)?`; else đọc toàn bộ stdin qua `std::io::read_to_string(std::io::stdin().lock())?`.
   - Gọi `crate::sentinel::extract_block(&text)?` — propagate `SentinelError` qua `anyhow::Error` (thiserror types tự convert qua `?` vào anyhow).
   - Loop mỗi raw line, gọi `crate::row::parse_row(&line)`. Bất kỳ `Err` → propagate `RowParseError` qua `anyhow::Error`.
   - Collect `Vec<AdvisoryRow>` → build output JSON với `serde_json::json!` macro:
     ```rust
     let out = serde_json::json!({
         "rows": rows,
         "stack_scanned": {},
         "advisories_found": rows.len(),
     });
     serde_json::to_writer(std::io::stdout().lock(), &out)?;
     println!(); // trailing newline cho POSIX line convention
     ```
   - Xoá `#![allow(dead_code)]` ở file này nếu P001 có (P001 stub có thể đã add — Worker verify).

3. **`src/main.rs` error mapping:**
   - Trong dispatch arm `Commands::ParseReport { input } => ...`, wrap `parse_report::run(input)` để map error → exit code. Vì `fn main() -> anyhow::Result<()>` và mọi match arm khác evaluate to `Result<()>`, arm này phải tail-return `Ok(())` để giữ uniformity (V2 fix per Turn 1 O1.1 — Worker correct, see Debate Log):
     ```rust
     Commands::ParseReport { input } => {
         if let Err(e) = parse_report::run(input) {
             let code = if e.is::<crate::sentinel::SentinelError>() {
                 1
             } else if e.is::<crate::row::RowParseError>() {
                 2
             } else {
                 2 // any other parse/IO failure → "processing error"
             };
             eprintln!("error: {:#}", e);
             std::process::exit(code);
         }
         Ok(())
     }
     ```
   - Đảm bảo `Commands::ParseReport` clap variant có field `input: Option<PathBuf>` với `#[arg(long)]`. Nếu P001 chưa add `--input` → add trong P004 (ARCHITECTURE §1 đã document `[--input <FILE>]`).
   - `use std::path::PathBuf;` import nếu chưa có.

4. **Fixture + integration test:**
   - `tests/fixtures/agent-report-1.md` — agent report mẫu với preamble + 2 row trong sentinel block + trailing prose. Worker viết theo template dưới.
   - `tests/parse_report_cli.rs` — integration test dùng `assert_cmd::Command::cargo_bin("advisory-inbox")` + `predicates`:
     - Happy: stdin pipe fixture content → stdout chứa `"advisories_found":2` + 2 CVE ID + exit 0.
     - Missing sentinel: stdin pipe "no markers" → exit 1, stderr contains "missing sentinel start".
     - Bad row: stdin pipe sentinel block với row sai format (e.g., bad severity "Critic") → exit 2, stderr contains "InvalidSeverity" hoặc related wording.

#### Why bubble qua `anyhow::Result` + downcast in main?

Spawn-prompt explicit: "bubble via anyhow::Result to main and have main map to exit code." Lý do design:
- `anyhow::Error::is::<T>()` cho phép check concrete error type ngay sau `?` propagation — không cần wrap each subcmd's errors trong 1 mega-enum.
- `SentinelError` + `RowParseError` đều là `thiserror::Error` → auto `Send + Sync + 'static` (enum không chứa raw pointer/non-Send field) → `anyhow::Error::from()` accepts.
- Future subcmd (dedup, append) sẽ dùng pattern y hệt — `is::<T>()` switch trong main.

Alternative đã reject:
- **B (typed `ParseReportError` enum):** cleaner type signature nhưng mỗi subcmd phải define enum riêng → boilerplate. Bỏ.
- **C (`process::exit` trong `run()`):** vi phạm separation — `cli/*` không nên direct exit (testability). Bỏ.

#### Why `parse_row` trong `row.rs` (không tách `row_parser.rs`)?

Spawn-prompt recommend keep scope tight. `row.rs` đã định nghĩa `AdvisoryRow` + 2 enum — đây là module owner cho row contract. `parse_row` là **inverse** của serde serialize, logically thuộc cùng module. P003 đã set precedent (sentinel.rs giữ `extract_block` + `SentinelError` cùng nhà).

Khi `row.rs` quá lớn (> 300 LOC sau P006/P008) → có thể tách `row/parse.rs`, `row/types.rs`. Tracked như refactor future, không phải P004 scope.

#### Why `FromStr` impl cho `Status` + `Severity`?

`parse_row` cần convert cell text → enum. Options đã consider:
- **A (`impl FromStr`):** standard Rust idiom, reusable cho future (e.g., `--min-severity High` CLI flag future feature). **Chọn.**
- **B (`serde_json::from_str(&format!(r#""{cell}""#))`):** hack, dùng serde infrastructure cho text parse. Reject — fragile escape.
- **C (manual match-on-string trong parse_row):** đơn giản nhưng không reusable. Reject.

Spec impl FromStr:
```rust
impl FromStr for Status {
    type Err = RowParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Status::Open),
            "processed" => Ok(Status::Processed),
            "dismissed" => Ok(Status::Dismissed),
            other => Err(RowParseError::InvalidStatus(other.to_string())),
        }
    }
}
// Severity tương tự với "Critical"/"High"/"Medium"/"Low".
```

#### Why split bằng `trim_start_matches('|') + trim_end_matches('|') + split('|')` (không regex)?

Markdown row format cố định: `| cell1 | cell2 | ... | cell8 |`. Naive `line.split('|')` cho 10 segment (leading empty + 8 cell + trailing empty). Robust approach:

```rust
let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
if cells.len() != 8 {
    return Err(RowParseError::WrongCellCount { expected: 8, actual: cells.len() });
}
```

Edge case: cell content chứa `|` literal? — Markdown table escape `\|` không support trong scope phiếu này. Agent report convention không emit pipe-in-cell (CVE ID / URL / package name không chứa `|`). Note ở Constraint #6: nếu phát hiện cell với `|` literal → Discovery Report + phiếu mới.

#### `stack_scanned: {}` placeholder

Spawn-prompt: "stack_scanned is `{}` for now (placeholder — populated from report `**Stack scanned:**` section later if needed; per BACKLOG MVP scope this can be empty)". Output JSON luôn emit empty object. ARCHITECTURE §1 đã document trường này trong output schema.

Future phiếu (out-of-scope P004): parse `**Stack scanned:**` markdown section → populate object như `{ "pnpm-lock.yaml": 42, "requirements.txt": 8 }`. Track như backlog enhancement, không block P004.

### Scope

- CHỈ sửa: `src/cli/parse_report.rs` (stub → real impl), `src/row.rs` (add `parse_row` + `RowParseError` + 2 FromStr impl + xoá `#![allow(dead_code)]`), `src/main.rs` (clap `--input` flag verify + error → exit code mapping).
- CHỈ tạo: `tests/fixtures/agent-report-1.md`, `tests/parse_report_cli.rs`.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (mark `cli/parse_report.rs` wired), `docs/CHANGELOG.md` (entry P004), `README.md` (quick-start section nếu chưa cover parse-report — Worker check).
- KHÔNG sửa: `src/sentinel.rs` (P003 ship, treat read-only), `src/state.rs` (P002, P005 sẽ touch), `Cargo.toml` (no new dep — `assert_cmd`/`predicates` đã có per Anchor #4 + #5), `src/cli/{dedup,append,migrate_state,state_backfill,scan_and_append,serve,init}.rs` (vẫn stub TODO).
- KHÔNG tạo: `src/error.rs` (ARCHITECTURE §5 list module này nhưng pending — KHÔNG ship trong P004).
- KHÔNG đổi `Status` / `Severity` enum variant naming (P002 lock — `Status` lowercase serde, `Severity` PascalCase serde). Chỉ ADD `FromStr` impl.
- KHÔNG đổi exit code semantics (ARCHITECTURE §1 đã document: 0 success, 1 input error, 2 processing error).

### Skills consulted

Architect ran `context7 query-docs /websites/rs_chrono_chrono` để verify `NaiveDate::parse_from_str("%Y-%m-%d")` API + ParseError type. Confirmed Anchor #6.

Architect đã đọc `Cargo.toml` để verify `assert_cmd = "2"` + `predicates = "3"` dev-deps + `anyhow = "1"` + `serde_json = "1"` + `thiserror = "2"` — Anchors #4/#5/#7/#8/#9 đều `[verified]` từ direct Read.

Architect KHÔNG dùng `assert_cmd` context7 lookup (library ID không tồn tại trong context7 cho `assert_cmd` Rust crate — Architect treat as `[needs Worker verify via cargo doc assert_cmd]` cho exact API surface). Worker EXECUTE Task 0 verify usage idiom qua `cargo doc --open` hoặc inline test trong P004 Task 4.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/cli/parse_report.rs` hiện là stub printf TODO (P001 ship). Chưa có logic. | P001 Discovery Report dòng 50-51 ("8 stub handlers (`parse_report`, `dedup`, ...). Each stub prints `TODO: <subcmd> — wired in P<NNN>` and exits 0") | `[needs Worker verify]` | ✅ Stub confirmed — but stub already has correct signature `pub fn run(input: Option<PathBuf>) -> Result<()>` at line 6; body is a single `println!("TODO: parse-report (input={:?}) — wired in P004", input); Ok(())`. P001 was slightly more sophisticated than Discovery Report described. Signature upgrade requirement is moot — same sig already present. |
| 2 | `src/row.rs` exports `pub struct AdvisoryRow` + `pub enum Status` (3 variant: `Open`/`Processed`/`Dismissed`) + `pub enum Severity` (4 variant: `Critical`/`High`/`Medium`/`Low`). P002 shipped. | P002 phiếu spec + P002 Discovery Report confirm | `[needs Worker verify]` | ✅ Confirmed at row.rs:14-30, 37-54. Status: Open/Processed/Dismissed (serde lowercase). Severity: Critical/High/Medium/Low (serde PascalCase). AdvisoryRow: 8 fields matching spec. All exact match. |
| 3 | `src/sentinel.rs` exports `pub fn extract_block(&str) -> Result<Vec<String>, SentinelError>` + `pub enum SentinelError` (variants `MissingStartMarker` / `MissingEndMarker`). P003 shipped. | P003 phiếu Task 1 + P003 Discovery Report | `[needs Worker verify]` | ✅ Confirmed: `pub fn extract_block(report_text: &str) -> Result<Vec<String>, SentinelError>` at sentinel.rs:41. `pub enum SentinelError` at line 23 with variants `MissingStartMarker` (line 25) and `MissingEndMarker` (line 27). Exact match. |
| 4 | `Cargo.toml` `[dev-dependencies]` có `assert_cmd = "2"`. KHÔNG cần add dep. | Architect Read `Cargo.toml` dòng 27 | `[verified]` | ✅ Dòng 27: `assert_cmd = "2"` confirmed. |
| 5 | `Cargo.toml` `[dev-dependencies]` có `predicates = "3"`. KHÔNG cần add dep. | Architect Read `Cargo.toml` dòng 28 | `[verified]` | ✅ Dòng 28: `predicates = "3"` confirmed. |
| 6 | `chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") -> ParseResult<NaiveDate>` (alias `Result<NaiveDate, chrono::format::ParseError>`). Example: `parse_from_str("2026-05-28", "%Y-%m-%d").is_ok()`. | context7 `/websites/rs_chrono_chrono` query Architect-side | `[verified]` | ✅ Docs example confirmed: `parse_from_str("2015-09-05", "%Y-%m-%d")` returns `Ok(NaiveDate)`. Out-of-range/format-mismatch returns `Err`. |
| 7 | `Cargo.toml` `[dependencies]` có `anyhow = "1"`. | Architect Read `Cargo.toml` dòng 19 | `[verified]` | ✅ Dòng 19: `anyhow = "1"` confirmed. |
| 8 | `Cargo.toml` `[dependencies]` có `serde_json = "1"`. | Architect Read `Cargo.toml` dòng 16 | `[verified]` | ✅ Dòng 16: `serde_json = "1"` confirmed. |
| 9 | `Cargo.toml` `[dependencies]` có `thiserror = "2"`. | Architect Read `Cargo.toml` dòng 20 | `[verified]` | ✅ Dòng 20: `thiserror = "2"` confirmed. |
| 10 | `src/main.rs` dispatch arm cho `Commands::ParseReport` hiện tồn tại + có field `input` (per ARCHITECTURE §1 documented `[--input <FILE>]`). Whether P001 đã wire `--input` clap arg = unknown. | P001 BACKLOG scope "8 subcmd registered" nhưng không nói flag detail | `[needs Worker verify]` | ✅ Case A confirmed: `Commands::ParseReport { #[arg(long)] input: Option<PathBuf> }` at main.rs:25-29. Dispatch at line 98: `Commands::ParseReport { input } => cli::parse_report::run(input)`. Clap variant fully wired. No change needed to struct. HOWEVER: dispatch returns `Result<()>` directly propagating via `fn main() -> Result<()>` — exit code mapping (phiếu Task 3.2) requires careful integration. See O1.1. |
| 11 | `tests/fixtures/` directory chưa tồn tại (P001-P003 không tạo). | Glob — Architect chỉ nhìn được path qua Glob. | `[needs Worker verify]` | ✅ `ls tests/fixtures/` → "No such file or directory". Both `tests/` and `tests/fixtures/` absent. Task 4 must `mkdir -p tests/fixtures`. |
| 12 | `tests/` directory: chưa có integration test file nào (mọi test inline `#[cfg(test)]` trong src/). P003 Discovery Report confirm "KHÔNG tạo `tests/sentinel_*.rs`". | P003 phiếu Scope dòng 79-80 | `[needs Worker verify]` | ✅ `tests/` directory does not exist. No existing integration tests to conflict with. |
| 13 | `Status` + `Severity` enum hiện CHƯA có `impl FromStr` (P002 chỉ derive `Serialize, Deserialize, Debug, Clone, PartialEq, Eq` — không có FromStr). | P002 phiếu Giải pháp dòng 39 list derive list explicit | `[needs Worker verify]` | ✅ `grep -n "impl FromStr" src/row.rs` → 0 hits. No FromStr implementation present. |
| 14 | `src/row.rs` có `#![allow(dead_code)]` ở module-level (P002 Discovery Report D1). Sẽ xoá trong P004 vì `AdvisoryRow`/`Status`/`Severity` được consume bởi `parse_row` (new) + `cli/parse_report.rs` (new). | P002 Discovery Report dòng 35-37, 104 ("Architect ghi nhận cho phiếu đó") | `[needs Worker verify]` | ✅ Confirmed at row.rs:8 — `#![allow(dead_code)]` present. Also: sentinel.rs:12 has same `#![allow(dead_code)]` — P004 wire-in of sentinel via parse_report.rs will eliminate need there too. Phiếu does not mention removing it from sentinel.rs; Worker will track in Discovery Report. |
| 15 | `README.md` tồn tại + có thể đã/chưa cover `parse-report` subcmd quick-start. | README chưa được verify. | `[needs Worker verify]` | ✅ `grep -n "parse-report" README.md` → 1 hit at line 15: `./target/release/advisory-inbox parse-report` (P001 quick-start scaffold, no output description). Per phiếu logic (≥1 hit → review + update wording), Task 6.3 must update README to add output JSON shape + exit code table. |

**Note Anchor #1, #2, #3, #10, #11, #12, #13, #14, #15:** Worker EXECUTE Task 0 chạy 9 grep/ls verify trên trước khi viết code. Nếu BẤT KỲ anchor nào lệch giả định → STOP, escalate qua Discovery Report (không tự fix vượt scope phiếu).

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.** Hiện không có ❌ — phụ thuộc Worker Task 0.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (V1 → V2 sau Turn 1 Architect Response — Task 3.2 dispatch body now tail-returns `Ok(())`)

### Turn 1 — Worker Challenge

**Anchor verification:**
- Anchor #1 ✅ — stub confirmed; signature already `pub fn run(input: Option<PathBuf>) -> Result<()>` (phiếu said `() -> ()` — moot, Task 2 body replacement still needed)
- Anchor #2 ✅ — Status/Severity/AdvisoryRow variants + field layout exact match
- Anchor #3 ✅ — `extract_block` signature + `SentinelError` variants exact match
- Anchor #4 ✅ — `assert_cmd = "2"` at Cargo.toml:27 (Architect-verified, re-confirmed)
- Anchor #5 ✅ — `predicates = "3"` at Cargo.toml:28 (Architect-verified, re-confirmed)
- Anchor #6 ✅ — chrono NaiveDate API confirmed (Architect skill-verified, accepted)
- Anchor #7 ✅ — `anyhow = "1"` at Cargo.toml:19 (Architect-verified, re-confirmed)
- Anchor #8 ✅ — `serde_json = "1"` at Cargo.toml:16 (Architect-verified, re-confirmed)
- Anchor #9 ✅ — `thiserror = "2"` at Cargo.toml:20 (Architect-verified, re-confirmed)
- Anchor #10 ✅ — `Commands::ParseReport { input: Option<PathBuf> }` Case A at main.rs:25-29; dispatch at line 98. See O1.1.
- Anchor #11 ✅ — `tests/` directory does not exist; `mkdir -p tests/fixtures` needed
- Anchor #12 ✅ — no integration test files present
- Anchor #13 ✅ — 0 `impl FromStr` hits in row.rs
- Anchor #14 ✅ — `#![allow(dead_code)]` at row.rs:8; also present at sentinel.rs:12 (out-of-scope for phiếu — will note in Discovery)
- Anchor #15 ✅ — README line 15 has `parse-report` mention (stub only, no output description); Task 6.3 update path: "≥1 hit → review + update wording"

**Baseline:** 14 tests (4 row + 6 sentinel + 4 state). Phiếu expects ≥23 after P004 (14 + ≥6 new row/FromStr + 3 integration) — count checks out.

---

**Objections:**

### O1.1 [shape] — Task 3.2 dispatch replacement produces type mismatch with `fn main() -> Result<()>`

**Reality** (`src/main.rs:94-117`):

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::ParseReport { input } => cli::parse_report::run(input),  // line 98 — returns Result<()>
        Commands::Dedup { ... } => cli::dedup::run(...),                   // returns Result<()>
        // ... all other arms return Result<()>
    }
}
```

**Phiếu's Task 3.2 replacement:**

```rust
Commands::ParseReport { input } => {
    if let Err(e) = parse_report::run(input) {
        let code = if e.is::<crate::sentinel::SentinelError>() { 1 }
                   else if e.is::<crate::row::RowParseError>() { 2 }
                   else { 2 };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
}
```

**Problem:** This arm evaluates to `()` (the `if let` block has no explicit tail expression). All other arms evaluate to `Result<()>`. Rust match arms must have identical types → **compile error**: `expected `Result<(), anyhow::Error>`, found `()``.

**Also mechanical note:** Phiếu's Task 3.2 "Find" text says to look for `Commands::ParseReport { .. } => parse_report::run()` but the real dispatch at main.rs:98 is `Commands::ParseReport { input } => cli::parse_report::run(input)`. The "Find" string won't match — Worker cannot use it verbatim as a search anchor.

**Proposed fix (Tầng 2 — Worker can self-decide; documenting as Tầng 1 shape issue because it is a function-signature / return-type matter affecting all dispatch arms):**

**Option A (Recommended):** Append `Ok(())` inside the arm block, making it return `Result<()>`:

```rust
Commands::ParseReport { input } => {
    if let Err(e) = parse_report::run(input) {
        let code = if e.is::<crate::sentinel::SentinelError>() { 1 }
                   else if e.is::<crate::row::RowParseError>() { 2 }
                   else { 2 };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

`std::process::exit` is diverging (`-> !`) so the `Ok(())` is unreachable when an error occurs, but satisfies the type checker for the happy path.

**Option B:** Change the Task 3.2 search anchor to match the real code at main.rs:98 exactly (fix the "Find" text), then apply Option A's body. Functionally the same as A; only difference is phiếu wording correction.

**Status:** ✅ RESOLVED by Turn 1 Architect Response (V2)

### Turn 1 — Architect Response (phiếu V2)

- **[O1.1] → ACCEPT (Option A).** Worker correct — `if let Err` block evaluates to `()` which fails match-arm uniformity against the other arms returning `Result<()>`. Patch applied to Task 3.2 "Thay bằng" snippet: append `Ok(())` as tail expression after the `if let` block. `std::process::exit` is diverging (`!`) so the `Ok(())` is unreachable on the err path — compile-clean, behavior unchanged. Also added explanatory sentence in Giải pháp §3 noting why the tail return is required (match-arm uniformity with `fn main() -> anyhow::Result<()>`).
- **[Mechanical note re: Task 3.2 "Find" anchor mismatch] → ACK.** Worker right — phiếu's "Find" snippet showed `Commands::ParseReport { .. } => parse_report::run()` but real main.rs:98 reads `Commands::ParseReport { input } => cli::parse_report::run(input)` (note `cli::` prefix + bound `input` instead of `..`). Worker should use the real line at main.rs:98 as the anchor for the Edit, not the phiếu's illustrative "Find" wording. The "Find" block in phiếu Task 3.2 is now best treated as "shape hint" — the Edit replaces the existing 1-line dispatch with the multi-line V2 body. No phiếu patch needed (Task 3.2 already says "Worker verify exact wording"); Architect leaves Task 3.2 "Find" as-is but logs this here so Worker EXECUTE uses main.rs:98 verbatim as search needle.

**Status:** ✅ RESPONDED — phiếu bumped V1 → V2. Ready for Worker Turn 2 surgical re-CHALLENGE (verify V2 fix only, no scope reopen).

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Turn 2 — Worker Challenge (surgical)

**Worker accepted V2 — Turn 2 surgical verify clean.**

V2 delta verified:

1. Task 3.2 "Find" anchor — `src/main.rs:98` reads exactly `Commands::ParseReport { input } => cli::parse_report::run(input),` (confirmed by direct Read). V2 phiếu now uses this verbatim as search needle. Match confirmed.

2. Task 3.2 "Thay bằng" snippet in V2 ends with `Ok(())` as tail expression after the `if let Err(e)` block. `std::process::exit` is diverging (`!`) so `Ok(())` is unreachable on the err path but satisfies match-arm uniformity against all other arms returning `Result<()>`. Type-correct.

3. No new shape issues introduced by V2 changes. No scope widening.

Anchor re-verification (surgical — only anchors touched by V2 delta):
- Anchor #10 ✅ — `main.rs:98` exact text re-confirmed.

**Status: Worker accepted V2. Ready for Chủ nhà approval gate → EXECUTE.**

### Final consensus
- Phiếu version: V2
- Total turns: 2 (Turn 1 O1.1 shape objection → Turn 2 surgical accept)
- Approved: ✅ V2 accepted by Worker Turn 2

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
| B (capability) | `cargo check` | exit 0 | exit 0 | ✅ |
| B (capability) | `cargo test row` | ≥7 tests pass (4 P002 + 3+ new parse_row + ≥2 FromStr) | 10 pass | ✅ |
| B (capability) | `cargo test --test parse_report_cli` | 3 integration tests pass | 3 pass | ✅ |
| B (capability) | `cargo run -- parse-report < tests/fixtures/agent-report-1.md` | stdout JSON `"advisories_found":2`, exit 0 | Confirmed | ✅ |
| C (migration) | (no schema change — output JSON shape is contract `advisories_found`/`rows`/`stack_scanned`) | N/A | N/A | N/A |
| D (persistence) | `grep -l "parse-report" docs/ARCHITECTURE.md` | ≥1 hit (§1 + §5) | 1 hit | ✅ |
| D (persistence) | `grep -l "parse-report" README.md` | ≥1 hit nếu Anchor #15 yêu cầu add | Updated Task 6.3 | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise bump | 0 packages | ✅ |
| E (env drift) | `cargo build --release` from clean target | exit 0, 0 warnings | exit 0, 0 warnings | ✅ |
| F (runtime state) | (no env var read, no token surface) | N/A | N/A | N/A |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B + D)

**Mục tiêu:** Worker grep + read trạng thái thật của repo TRƯỚC khi viết code. Đối chiếu Anchor table.

**Lệnh chạy (verify Anchor #1, #2, #3, #10, #11, #12, #13, #14, #15):**

```bash
# Anchor #1 — parse_report stub state
cat src/cli/parse_report.rs

# Anchor #2 — row types signature
grep -n "pub \(struct\|enum\) \(AdvisoryRow\|Status\|Severity\)" src/row.rs

# Anchor #3 — sentinel exports
grep -n "pub fn extract_block\|pub enum SentinelError" src/sentinel.rs

# Anchor #10 — main.rs ParseReport variant + input flag
grep -n -A 3 "ParseReport" src/main.rs

# Anchor #11 + #12 — tests dir state
ls tests/ 2>&1
ls tests/fixtures/ 2>&1

# Anchor #13 — FromStr expect 0 hits
grep -n "impl FromStr" src/row.rs

# Anchor #14 — allow(dead_code) location
head -5 src/row.rs

# Anchor #15 — README parse-report coverage
grep -n "parse-report" README.md

# Baseline
cargo check
cargo test --all -- --list | wc -l
```

**Output:** Worker fill kết quả vào Debate Log Turn 1 Anchor section.

**Hard Stop triggers:**
- Anchor #2 — nếu enum variant naming khác P002 spec (Status: Open/Processed/Dismissed; Severity: Critical/High/Medium/Low) → STOP escalate.
- Anchor #3 — nếu `SentinelError` variant naming khác P003 spec → STOP escalate.
- Anchor #10 — nếu `Commands::ParseReport` không tồn tại hoặc dispatch arm khác `parse_report::run(...)` → STOP escalate.
- Anchor #13 — nếu `impl FromStr` đã có cho Status/Severity → STOP escalate (drift P002).
- Anchor #14 — nếu `#![allow(dead_code)]` đã bị xoá từ trước → SKIP Task 1.4 step, log Discovery Report (P003 hoặc ai đó đã xoá).

### Task 1: Mở rộng `src/row.rs` — `parse_row` + `RowParseError` + 2 `FromStr` impl

**File:** `src/row.rs`

**Mục tiêu:** Add 5 piece mới trong cùng file (giữ scope tight per Giải pháp section):
1. `pub enum RowParseError` với 5 variant.
2. `impl FromStr for Status` (lowercase canonical).
3. `impl FromStr for Severity` (PascalCase canonical).
4. `pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError>`.
5. **Xoá `#![allow(dead_code)]`** ở đầu file (per Anchor #14).
6. Thêm ≥5 unit test (3 cho `parse_row` + 2 cho FromStr).

**Skeleton (Worker write theo skeleton — không copy raw, match project style):**

```rust
// (Xoá #![allow(dead_code)] ở đầu file — Task 1.5)

use std::str::FromStr;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ... (giữ AdvisoryRow + Status + Severity hiện có từ P002 — KHÔNG đổi)

/// Errors returned by [`parse_row`] when an inbox row line cannot be decoded.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum RowParseError {
    #[error("empty row line")]
    EmptyLine,
    #[error("expected {expected} cells, got {actual}")]
    WrongCellCount { expected: usize, actual: usize },
    #[error("invalid date `{0}` (expected YYYY-MM-DD)")]
    InvalidDate(String),
    #[error("invalid severity `{0}` (expected Critical/High/Medium/Low)")]
    InvalidSeverity(String),
    #[error("invalid status `{0}` (expected open/processed/dismissed)")]
    InvalidStatus(String),
}

impl FromStr for Status {
    type Err = RowParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Status::Open),
            "processed" => Ok(Status::Processed),
            "dismissed" => Ok(Status::Dismissed),
            other => Err(RowParseError::InvalidStatus(other.to_string())),
        }
    }
}

impl FromStr for Severity {
    type Err = RowParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Critical" => Ok(Severity::Critical),
            "High" => Ok(Severity::High),
            "Medium" => Ok(Severity::Medium),
            "Low" => Ok(Severity::Low),
            other => Err(RowParseError::InvalidSeverity(other.to_string())),
        }
    }
}

/// Parse one pipe-delimited inbox row line into an [`AdvisoryRow`].
///
/// Expects exactly 8 cells in order:
/// `Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note`.
///
/// Whitespace around each cell is trimmed. Leading/trailing `|` are stripped.
///
/// # Errors
/// - [`RowParseError::EmptyLine`] if line is empty after trimming.
/// - [`RowParseError::WrongCellCount`] if cell count != 8.
/// - [`RowParseError::InvalidDate`] if Date cell does not match `YYYY-MM-DD`.
/// - [`RowParseError::InvalidSeverity`] / [`RowParseError::InvalidStatus`] for unknown enum values.
pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(RowParseError::EmptyLine);
    }
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    if cells.len() != 8 {
        return Err(RowParseError::WrongCellCount {
            expected: 8,
            actual: cells.len(),
        });
    }
    let date = NaiveDate::parse_from_str(cells[0], "%Y-%m-%d")
        .map_err(|_| RowParseError::InvalidDate(cells[0].to_string()))?;
    let advisory_id = cells[1].to_string();
    let source_url = cells[2].to_string();
    let package = cells[3].to_string();
    let file_line = cells[4].to_string();
    let severity = Severity::from_str(cells[5])?;
    let status = Status::from_str(cells[6])?;
    let note = cells[7].to_string();
    Ok(AdvisoryRow {
        date,
        advisory_id,
        source_url,
        package,
        file_line,
        severity,
        status,
        note,
    })
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parse_row_happy_path() {
        let line = "| 2026-05-28 | CVE-2026-0001 | https://example.com/cve | next@<15.5.17 | src/middleware.ts:42 | High | open | - |";
        let row = parse_row(line).expect("happy path parses");
        assert_eq!(row.advisory_id, "CVE-2026-0001");
        assert_eq!(row.severity, Severity::High);
        assert_eq!(row.status, Status::Open);
        assert_eq!(row.note, "-");
    }

    #[test]
    fn parse_row_bad_date_errors() {
        let line = "| not-a-date | CVE-X | u | p | f:1 | High | open | - |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(err, RowParseError::InvalidDate(_)));
    }

    #[test]
    fn parse_row_bad_severity_errors() {
        let line = "| 2026-05-28 | CVE-X | u | p | f:1 | Critic | open | - |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(err, RowParseError::InvalidSeverity(s) if s == "Critic"));
    }

    #[test]
    fn parse_row_wrong_cell_count() {
        let line = "| 2026-05-28 | CVE-X | only-three |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(err, RowParseError::WrongCellCount { expected: 8, actual: 3 }));
    }

    #[test]
    fn status_from_str_roundtrip() {
        assert_eq!("open".parse::<Status>().unwrap(), Status::Open);
        assert_eq!("dismissed".parse::<Status>().unwrap(), Status::Dismissed);
        assert!("OPEN".parse::<Status>().is_err());
    }

    #[test]
    fn severity_from_str_canonical() {
        assert_eq!("Critical".parse::<Severity>().unwrap(), Severity::Critical);
        assert_eq!("Low".parse::<Severity>().unwrap(), Severity::Low);
        assert!("critical".parse::<Severity>().is_err()); // case-sensitive PascalCase
    }
}
```

**Lưu ý:**
- **Xoá `#![allow(dead_code)]`** ở đầu file (per Anchor #14 + P002 ghi chú). Sau khi xoá: `parse_row` + 2 FromStr impl đảm bảo `AdvisoryRow`/`Status`/`Severity` đều có public consumer trong binary code path qua P004 wire-in. Nếu Worker thấy clippy `dead_code` warning sau khi xoá → STOP escalate (signals incomplete wire-in).
- `chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")` trả `Result<NaiveDate, chrono::format::ParseError>` (Anchor #6 verified). Dùng `.map_err(|_| RowParseError::InvalidDate(...))` để convert vì `chrono::ParseError` không vào `RowParseError` variant signature.
- FromStr `Err = RowParseError` cho phép `?` operator trong `parse_row` chain liền mạch (`Severity::from_str(cells[5])?`).
- Test module name là `parse_tests` (không gộp với existing `tests` module nếu P002 đã có) — Worker check tên module hiện tại của P002 test block; nếu trùng → dùng `tests` module và append test. KHÔNG override existing tests.
- Existing P002 tests phải vẫn pass (Anchor B "≥7 tests pass": 4 P002 + 3 parse_row + 2 FromStr + 1 wrong cell count = 10).

### Task 2: Wire-in `src/cli/parse_report.rs`

**File:** `src/cli/parse_report.rs`

**Tìm** (P001 stub — Worker verify exact wording qua Anchor #1; expect dạng tương tự):
```rust
pub fn run() {
    println!("TODO: parse-report — wired in P004");
}
```

**Thay bằng (full file content):**

```rust
//! `advisory-inbox parse-report` — extract sentinel block from agent report markdown,
//! parse each row into [`AdvisoryRow`], emit JSON to stdout.
//!
//! See ARCHITECTURE.md §1 subcmd `parse-report` for the I/O contract.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::row::{self, AdvisoryRow};
use crate::sentinel;

/// Read agent report (stdin or `--input <FILE>`), parse rows, emit JSON.
///
/// Output JSON shape:
/// ```json
/// { "rows": [...], "stack_scanned": {}, "advisories_found": N }
/// ```
///
/// `stack_scanned` is currently always `{}` — future phiếu will populate from
/// the report `**Stack scanned:**` section.
///
/// # Errors
/// - [`sentinel::SentinelError`] (mapped to exit 1 in `main.rs`) if markers missing.
/// - [`row::RowParseError`] (mapped to exit 2) if any row line fails to parse.
/// - I/O errors (read stdin/file) → anyhow bubble.
pub fn run(input: Option<PathBuf>) -> Result<()> {
    // 1. Read source text.
    let text = match input {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("read input file {}", path.display()))?,
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .lock()
                .read_to_string(&mut buf)
                .context("read stdin")?;
            buf
        }
    };

    // 2. Extract sentinel block (SentinelError bubble → main maps to exit 1).
    let raw_lines = sentinel::extract_block(&text)?;

    // 3. Parse each line into AdvisoryRow (RowParseError bubble → main maps to exit 2).
    let mut rows: Vec<AdvisoryRow> = Vec::with_capacity(raw_lines.len());
    for line in &raw_lines {
        let row = row::parse_row(line)?;
        rows.push(row);
    }

    // 4. Build + emit output JSON (compact, single line + trailing newline).
    let out = serde_json::json!({
        "rows": rows,
        "stack_scanned": {},
        "advisories_found": rows.len(),
    });
    serde_json::to_writer(std::io::stdout().lock(), &out)
        .context("write stdout JSON")?;
    println!();
    Ok(())
}
```

**Lưu ý:**
- Function signature `pub fn run(input: Option<PathBuf>) -> anyhow::Result<()>`. P001 stub probably có signature `pub fn run()` — Task 3 (main.rs) must update dispatch để pass `input` + handle Result.
- `use crate::row::{self, AdvisoryRow}` — `self` để dùng `row::parse_row`; `AdvisoryRow` để type annotation.
- `serde_json::json!` macro outputs `serde_json::Value`. `serde_json::to_writer` writes compact (no pretty). Trailing `println!()` thêm `\n` cho POSIX line convention (downstream tools expect trailing newline).
- KHÔNG dùng `process::exit` trong `run()` — chỉ trong `main.rs` (separation of concerns; cho testability).
- KHÔNG xoá `#![allow(dead_code)]` nếu P001 đã add ở file này (Worker grep — thường stub không có); nếu có thì xoá vì function bây giờ thật.

### Task 3: Update `src/main.rs` — clap `--input` + error → exit code

**File:** `src/main.rs`

**Mục tiêu 2 phần:**

**3.1. Đảm bảo `Commands::ParseReport` clap variant có `input: Option<PathBuf>`:**

**Tìm** (Worker grep `Commands` enum block — expect existing variant, có thể đã có hoặc thiếu field):

Trường hợp A (P001 đã add `input`):
```rust
ParseReport {
    #[arg(long)]
    input: Option<PathBuf>,
},
```
→ KHÔNG đổi.

Trường hợp B (P001 stub-only, thiếu field):
```rust
ParseReport,
```
→ **Thay bằng:**
```rust
ParseReport {
    /// Read agent report from file instead of stdin.
    #[arg(long)]
    input: Option<PathBuf>,
},
```

**Lưu ý:** Worker `grep -n "ParseReport" src/main.rs` ngay trong Task 0 đã biết case nào. Nếu case khác (e.g., flag tên khác như `--file`) → STOP escalate (drift ARCHITECTURE §1).

**3.2. Cập nhật dispatch arm — handle `Result` + map exit code:**

**Tìm** (real line per Worker Turn 1 verification — main.rs:98):
```rust
Commands::ParseReport { input } => cli::parse_report::run(input),
```

> Note: This is the actual line in `src/main.rs:98` as confirmed by Worker Turn 1 (Anchor #10). Earlier draft phiếu (V1) showed an illustrative `Commands::ParseReport { .. } => parse_report::run()` "Find" string that does not match real code — Worker must use the main.rs:98 verbatim anchor above.

**Thay bằng (V2 — adds `Ok(())` tail to satisfy match-arm uniformity per Turn 1 O1.1):**
```rust
Commands::ParseReport { input } => {
    if let Err(e) = parse_report::run(input) {
        let code = if e.is::<crate::sentinel::SentinelError>() {
            1
        } else if e.is::<crate::row::RowParseError>() {
            2
        } else {
            2
        };
        eprintln!("error: {:#}", e);
        std::process::exit(code);
    }
    Ok(())
}
```

**Import bổ sung (nếu chưa có):**
```rust
use std::path::PathBuf;
```

**Lưu ý:**
- **V2 critical:** the trailing `Ok(())` is REQUIRED. `fn main() -> anyhow::Result<()>` and all other match arms evaluate to `Result<()>`. Without the tail `Ok(())`, the `if let` block evaluates to `()` → type mismatch compile error. `std::process::exit` is diverging (`!`) so `Ok(())` is unreachable on the err path — present only for the type checker on the success branch.
- The original dispatch at `src/main.rs:98` is a single expression `cli::parse_report::run(input)` (with the `cli::` module prefix). Worker Edit must match that exact line including the `cli::` prefix.
- `use std::path::PathBuf;` — Worker grep `use std::path::PathBuf` `src/main.rs`. Nếu chưa có → add ở import block đầu file.
- `anyhow::Error::is::<T>()` đòi T: `Send + Sync + 'static`. `SentinelError` (P003) + `RowParseError` (Task 1) đều là enum không có raw pointer / non-Send → auto Send+Sync+'static. Compile-time guaranteed.
- KHÔNG đổi các dispatch arm khác (dedup/append/migrate_state/state_backfill/scan_and_append/serve/init vẫn stub).
- Format `{:#}` cho anyhow Display chain (in cause chain với context).

### Task 4: Tạo fixture `tests/fixtures/agent-report-1.md`

**File:** `tests/fixtures/agent-report-1.md` (new — Worker `mkdir -p tests/fixtures` nếu chưa có per Anchor #11)

**Nội dung (Worker write exact theo skeleton — fixture là contract; deviation = test break):**

```markdown
## Advisory Scan Report — 2026-05-28

**Stack scanned:**
- pnpm-lock.yaml resolved: 42 packages
- requirements.txt exact pin: 8 deps

**Advisories found:** 2

<!-- INBOX_APPEND_START -->
| 2026-05-28 | CVE-2026-9999 | https://nvd.nist.gov/vuln/detail/CVE-2026-9999 | next@<15.5.17 | src/middleware.ts:42 | High | open | - |
| 2026-05-28 | GHSA-aaaa-bbbb | https://github.com/advisories/GHSA-aaaa-bbbb | flask@<2.3.5 | astro-service/app.py:8 | Medium | open | - |
<!-- INBOX_APPEND_END -->

(End of report.)
```

**Lưu ý:**
- 2 row trong sentinel block. Acceptance test sẽ assert `advisories_found:2` + 2 CVE/GHSA ID xuất hiện trong stdout.
- Stack scanned section là markdown text — P004 KHÔNG parse, chỉ skip (output luôn `stack_scanned: {}`).
- KHÔNG add row thứ 3 / row với bad format trong fixture này. Bad-row test trong Task 5 sẽ inline-construct bad input (không cần fixture file riêng).
- Worker preserve trailing newline cuối file (POSIX convention).

### Task 5: Tạo integration test `tests/parse_report_cli.rs`

**File:** `tests/parse_report_cli.rs` (new)

**Nội dung (Worker write skeleton — 3 test case):**

```rust
//! Integration tests for `advisory-inbox parse-report` subcmd.
//!
//! Covers: happy path (fixture file via stdin), missing sentinel exit 1,
//! bad row format exit 2.

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn parse_report_happy_path_two_rows() {
    let fixture = include_str!("fixtures/agent-report-1.md");
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""advisories_found":2"#))
        .stdout(predicate::str::contains("CVE-2026-9999"))
        .stdout(predicate::str::contains("GHSA-aaaa-bbbb"));
}

#[test]
fn parse_report_missing_sentinel_exit_1() {
    let input = "just some prose without any sentinel markers at all\n";
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(input)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("missing sentinel start marker"));
}

#[test]
fn parse_report_bad_severity_exit_2() {
    let input = "\
<!-- INBOX_APPEND_START -->
| 2026-05-28 | CVE-X | u | p | f:1 | Critic | open | - |
<!-- INBOX_APPEND_END -->
";
    Command::cargo_bin("advisory-inbox")
        .unwrap()
        .arg("parse-report")
        .write_stdin(input)
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("invalid severity"));
}
```

**Lưu ý:**
- `assert_cmd::Command::cargo_bin("advisory-inbox")` rebuilds binary nếu cần. Verify Anchor #4 đã pass.
- `predicate::str::contains` — substring match, không exact equality. Wording stderr ("missing sentinel start marker", "invalid severity") khớp `SentinelError::MissingStartMarker` Display + `RowParseError::InvalidSeverity` Display (Task 1 skeleton thiserror `#[error("...")]`).
- `write_stdin(...)` accepts `&str` (cũng `Vec<u8>`/`String`). Fixture đọc qua `include_str!` (compile-time include — fixture path tương đối từ test file).
- Nếu fixture path resolution lỗi (Worker đặt fixture ở location khác) → STOP escalate.
- 3 test này là **MIN**. Worker có thể add thêm test edge case (empty block → `advisories_found:0`) — Tầng 2 Worker self-decide, không bắt buộc trong phiếu.

### Task 6: Docs Gate updates

**File 6.1:** `docs/ARCHITECTURE.md` — §5 Scaffold status

**Tìm** (block P003 đã update — Worker grep "Scaffold status"):
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic, not yet wired into `cli/parse_report.rs`.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Thay bằng:**
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic, not yet wired into `cli/parse_report.rs`.
- P004: `cli/parse_report.rs` wired (stdin/`--input` → sentinel → row → JSON stdout); `row::parse_row` + `RowParseError` + `FromStr` for `Status`/`Severity` shipped.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Lưu ý:**
- Date `2026-05-28` của heading giữ nguyên (snapshot baseline). P004 entry là 1 dòng mới, không thay date heading.
- KHÔNG đổi §1 CLI surface (parse-report contract đã document đúng từ trước — không drift).
- KHÔNG đổi §4 sentinel format (P003 đã document).

**File 6.2:** `docs/CHANGELOG.md` — entry P004

**Thêm entry (newest at top, theo convention P001/P002/P003):**

```markdown
## P004 — parse-report subcmd (2026-MM-DD)

**Type:** feat | **Tầng:** 1 | **Lane:** Normal

- Wire `src/cli/parse_report.rs` — `run(Option<PathBuf>) -> anyhow::Result<()>` reads stdin or `--input <FILE>`, calls `sentinel::extract_block` then `row::parse_row` per line, emits JSON `{ "rows": [...], "stack_scanned": {}, "advisories_found": N }` to stdout.
- Extend `src/row.rs`: add `pub fn parse_row(&str) -> Result<AdvisoryRow, RowParseError>` (pipe-split + per-cell decode), `pub enum RowParseError` (5 variants via `thiserror`), `impl FromStr for Status` + `impl FromStr for Severity`.
- Remove `#![allow(dead_code)]` from `src/row.rs` (consumer wire-in complete per P002 Discovery follow-up). `src/state.rs` keeps the attribute until P005 dedup wire-in.
- `src/main.rs`: dispatch `Commands::ParseReport { input }` maps `SentinelError` → exit 1, `RowParseError` → exit 2, other → exit 2; `anyhow` cause chain printed to stderr.
- New fixture `tests/fixtures/agent-report-1.md` (2 rows).
- New integration test `tests/parse_report_cli.rs` (3 cases: happy path, missing sentinel → exit 1, bad severity → exit 2).
- `stack_scanned` placeholder `{}` per BACKLOG MVP scope — future phiếu may parse `**Stack scanned:**` section.

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)
```

**Lưu ý:**
- `2026-MM-DD` → Worker thay ngày ship thực.
- Newest at top — append above P003 entry (P003 hiện ở top per CHANGELOG.md state).

**File 6.3:** `README.md` quick-start (CONDITIONAL — depending Anchor #15)

**If** `grep -n "parse-report" README.md` returns 0 hits → add quick-start section:

**Tìm** (Worker find appropriate insertion point — likely after install instructions / before MCP section):

**Thêm (new section):**
````markdown
### Quick start — parse an agent report

Pipe an advisory-watch agent report to the binary; get JSON on stdout:

```bash
advisory-inbox parse-report < path/to/agent-report.md
# → { "rows": [...], "stack_scanned": {}, "advisories_found": N }
```

Exit codes:
- `0` — parsed N rows successfully (N may be 0 if block is empty)
- `1` — sentinel markers `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` missing
- `2` — row format invalid (bad date / unknown severity / wrong cell count)
````

**If** `parse-report` already mentioned (≥1 hit) → Worker review existing section + update exit codes + output JSON shape if outdated. Otherwise skip.

**Lưu ý:**
- README cập nhật là Tầng 1 (CLI subcmd behavior visibly shifts from stub TODO → real output). RULES.md §11 matrix: "CLI subcommand added/removed/renamed → README.md quick-start" — P004 đổi behavior thực sự là wire-in, treated như "subcmd activated".
- Wording phải khớp ARCHITECTURE §1 exit codes (0/1/2) — không drift.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/row.rs` | Task 1: thêm `parse_row` + `RowParseError` + 2 `FromStr` impl + ≥6 test; **xoá `#![allow(dead_code)]`** |
| `src/cli/parse_report.rs` | Task 2: stub → real impl (`run(Option<PathBuf>) -> anyhow::Result<()>` pipeline) |
| `src/main.rs` | Task 3: `Commands::ParseReport { input: Option<PathBuf> }` clap + dispatch error → exit code map; `use std::path::PathBuf` nếu thiếu |
| `tests/fixtures/agent-report-1.md` | Task 4: new — fixture 2 rows |
| `tests/parse_report_cli.rs` | Task 5: new — 3 integration test |
| `docs/ARCHITECTURE.md` | Task 6.1: §5 Scaffold status thêm P004 line |
| `docs/CHANGELOG.md` | Task 6.2: prepend P004 entry |
| `README.md` | Task 6.3: add `parse-report` quick-start nếu chưa có (conditional per Anchor #15) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/sentinel.rs` | Treat read-only; KHÔNG đổi `extract_block` signature/behavior. P004 chỉ là consumer. |
| `src/state.rs` | KHÔNG xoá `#![allow(dead_code)]` (P005 dedup sẽ xoá khi import `StateFile`). |
| `src/cli/dedup.rs` | Vẫn stub TODO — P005 wire-in. |
| `src/cli/append.rs` | Vẫn stub TODO — P006 wire-in. |
| `src/cli/{migrate_state,state_backfill,scan_and_append,serve,init}.rs` | Vẫn stub TODO. |
| `Cargo.toml` | `assert_cmd = "2"`, `predicates = "3"`, `anyhow = "1"`, `serde_json = "1"`, `thiserror = "2"` đã có (Anchor #4/#5/#7/#8/#9). KHÔNG thêm dep mới. |
| `CLAUDE.md` | KHÔNG đổi doctrine. (No tech-stack drift trong P004.) |
| `docs/RULES.md` | KHÔNG đổi. |
| `docs/PROJECT.md` | Phase status không đổi (vẫn Phase 1 in-progress). |
| `docs/BACKLOG.md` | Worker không tự strikethrough P004 — orchestrator/Sếp xử lý "Recently shipped" section post-merge. |

---

## Luật chơi (Constraints)

1. **No new deps.** Mọi crate sử dụng đã có trong `Cargo.toml` (Anchor #4/#5/#7/#8/#9 verified). KHÔNG thêm `clap_complete`, `serde_yaml`, etc.
2. **No `unsafe { ... }` block** — pure safe Rust. Escalate Sếp nếu Worker thấy cần.
3. **No `process::exit` ngoài `src/main.rs`** — `cli/parse_report.rs` bubble `anyhow::Result`, KHÔNG direct exit. Lý do: testability + separation of concerns.
4. **Output JSON shape cố định** — `{ "rows": ..., "stack_scanned": ..., "advisories_found": ... }` đúng 3 field, đúng tên (ARCHITECTURE §1 contract). KHÔNG add `version` / `timestamp` / `errors` field nào — Tầng 1 public contract, drift = escalate.
5. **Exit code semantics cố định** — 0 success, 1 sentinel missing, 2 row parse fail (hoặc IO fail). KHÔNG add code 3+. ARCHITECTURE §1 exit code table là contract.
6. **Pipe-in-cell KHÔNG support.** Markdown row cell chứa `|` literal sẽ break parser. Nếu Worker phát hiện fixture/real-world report có pipe-in-cell → STOP, Discovery Report, KHÔNG tự fix escape logic.
7. **`Status`/`Severity` enum variant naming locked** (P002 ship). FromStr impl phải khớp **exact case-sensitive** (Status lowercase, Severity PascalCase) per `#[serde(rename_all = ...)]` của P002. KHÔNG add case-insensitive matching — Tầng 1 contract.
8. **`stack_scanned: {}` cố định empty trong P004.** Future phiếu parse Stack scanned section — KHÔNG tự thêm logic parse khi out-of-scope.
9. **Conventional commits** — `feat(P004): wire parse-report subcmd` (hoặc tương tự match P001/P002/P003 commit msg pattern; Worker grep `git log --oneline -5` confirm).
10. **Docs Gate Tầng 1 mandatory** — CLI subcmd activated (behavior visibly changes from TODO → real). ARCHITECTURE §5 + CHANGELOG + README (conditional) bắt buộc.
11. **`#![allow(dead_code)]` chỉ xoá khỏi `row.rs`.** KHÔNG touch `state.rs` allow — P005 dedup phiếu sẽ xoá khi import `StateFile`. Trying to remove trong P004 = scope creep.
12. **`anyhow::Error::is::<T>()` downcast — KHÔNG chain extra error types.** Chỉ check 2 concrete error: `SentinelError`, `RowParseError`. Other error (IO, JSON write) fallthrough to exit 2 — đúng spec ARCHITECTURE §1 ("processing error").

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (≥10 row tests + 6 sentinel + 4 state + 3 parse_report_cli integration = ≥23 tests total)
- [ ] `cargo test row` — ≥10 tests (4 P002 + ≥6 mới)
- [ ] `cargo test --test parse_report_cli` — 3 integration tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (sau khi xoá `#![allow(dead_code)]` row.rs, không có dead_code warning vì consumer wire-in)
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] Happy path: `cargo run -- parse-report < tests/fixtures/agent-report-1.md` → stdout JSON với `"advisories_found":2`, exit 0.
- [ ] File flag: `cargo run -- parse-report --input tests/fixtures/agent-report-1.md` → same JSON output, exit 0.
- [ ] Missing sentinel: `echo "no markers" | cargo run -- parse-report` → stderr `error: missing sentinel start marker ...`, exit 1.
- [ ] Bad row: pipe sentinel block với `| 2026-05-28 | X | u | p | f:1 | Critic | open | - |` → exit 2, stderr contains `invalid severity \`Critic\``.
- [ ] Empty block: pipe sentinel start/end với 0 row giữa → stdout `"advisories_found":0`, `"rows":[]`, exit 0.
- [ ] Help: `cargo run -- parse-report --help` → clap usage hiển thị `--input <INPUT>` flag.

### Regression
- [ ] `cargo run -- --help` vẫn show 8 subcmd (P001 baseline).
- [ ] Other stub subcmd vẫn exit 0 với TODO message: `cargo run -- dedup`, `cargo run -- append`, etc.
- [ ] `cargo test sentinel` — 6 P003 test vẫn pass (parser pure logic không đổi).
- [ ] `cargo test state` — 4 P002 test vẫn pass.

### Docs Gate
- [ ] `docs/ARCHITECTURE.md` §5 — Scaffold status thêm P004 line (Task 6.1).
- [ ] `docs/CHANGELOG.md` — P004 entry prepended (Task 6.2), date filled.
- [ ] `README.md` — quick-start `parse-report` section present + exit code table đúng (Task 6.3 conditional).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P004.md` — full report written. **MUST include:**
  - Anchor verification results (table — 15 anchor).
  - Confirmation `Commands::ParseReport` clap variant signature thực tế (Anchor #10 outcome — case A or B).
  - Confirmation `#![allow(dead_code)]` đã xoá khỏi `row.rs` + `cargo clippy` không phát sinh `dead_code` warning sau khi xoá.
  - Confirmation `state.rs` vẫn giữ `#![allow(dead_code)]` (Constraint #11).
  - Any edge case / drift phát hiện trong fixture format hoặc clap behavior.
  - Sub-mech B + D Verification Trace fill đầy đủ.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `- 2026-MM-DD P004: parse-report wired (stdin/--input → sentinel → row → JSON), #![allow(dead_code)] removed from row.rs, anyhow downcast maps exit codes → see docs/discoveries/P004.md`.
- [ ] Sub-mechanism B + D Verification Trace filled (table above).
