# PHIẾU P003: Sentinel parser

> **ID format:** `P003` — counter `.phieu-counter` = 3 sau P002 ship.
> **Filename:** `docs/ticket/P003-sentinel-parser.md`
> **Branch:** `feat/P003-sentinel-parser`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P004 parse-report wire-in — sentinel extract là bước đầu của pipeline parse → dedup → append)
> **Ảnh hưởng:** `src/sentinel.rs` (new), `src/main.rs` (thêm 1 dòng `mod sentinel;`), `CLAUDE.md` (Tech Stack regex line + File Structure comment), `docs/ARCHITECTURE.md` §5 module status, `docs/CHANGELOG.md`
> **Dependency:** P002 (row + state types) — đã ship 2026-05-28
> **Lane:** Normal (pure parser, no I/O, no schema change — Normal per RULES.md §1 vì module public API contract mới)
> **Sub-mech áp dụng:** B (capability — `cargo check` + `cargo test sentinel`), D (persistence — CLAUDE.md doctrine wording align với code)

---

## Context

### Vấn đề hiện tại

P002 ship 2 types (`AdvisoryRow`, `StateFile`). Bước kế tiếp pipeline parse là **trích block giữa sentinel marker** từ agent report markdown. Trước khi parse từng row thành `AdvisoryRow` (P004), cần một module thuần tách block thô — input là `&str` toàn report, output là danh sách raw row lines (mỗi dòng = 1 string ứng viên cho row parser).

Lý do tách module riêng (KHÔNG nhét trực tiếp vào `cli/parse_report.rs`):
- Reuse từ MCP tool `parse_report` (ARCHITECTURE §6) — cùng 1 logic 2 caller.
- Test unit pure logic dễ — không cần fixture file system.
- Sentinel grammar là contract giữa advisory-watch agent và advisory-inbox binary — phải có 1 owner module rõ ràng.

Hiện trạng filesystem (verify từ docs):
- `src/sentinel.rs` chưa tồn tại (ARCHITECTURE §5 list pending modules).
- `src/main.rs` đang declare `mod cli; mod row; mod state;` (per P002 spec). Cần thêm `mod sentinel;`.

Reference BACKLOG.md item P003:
- Scope: `src/sentinel.rs` regex extract block giữa `<!-- INBOX_APPEND_START -->` và `<!-- INBOX_APPEND_END -->`. Handle: missing markers, empty block, multiple markers (use first pair + warn).
- Acceptance: 5 unit tests cover all cases. Fixture: real agent report.
- Sub-mech checks: B.

### Giải pháp

Tạo `src/sentinel.rs` với:

1. **Constants** — 2 marker string literals (`SENTINEL_START`, `SENTINEL_END`) per CLAUDE.md naming convention (SCREAMING_SNAKE).
2. **`SentinelError` enum** — `thiserror::Error` derive, 2 variant: `MissingStartMarker`, `MissingEndMarker`. Phiếu này KHÔNG bao `RegexBuildError` vì pattern static compile-time-safe (test bằng `Regex::new(...).unwrap()` trong unit test).
3. **`pub fn extract_block(report_text: &str) -> Result<Vec<String>, SentinelError>`** — public API.
   - Find first `SENTINEL_START` occurrence.
   - Find next `SENTINEL_END` AFTER start marker.
   - Slice between markers, line-split.
   - Skip: blank lines (sau `trim()`), comment lines (start với `<!--` sau `trim_start()`).
   - Keep mọi line khác AS-IS (chưa parse — đó là job P004 row parser).
   - Return `Vec<String>` (chứ không `Vec<&str>`) — owned, ergonomic cho caller, cost trivial cho typical 10-row report (< 100 lines × < 200 byte).
4. **Multiple START warning** — nếu sau khi tìm cặp đầu tiên, còn `SENTINEL_START` nữa trong phần REMAINING report → emit `eprintln!("warn: multiple INBOX_APPEND_START markers found ({} total); using first pair", count)`. KHÔNG fail. Lý do: agent buggy có thể emit duplicate; safer to take first + warn than abort.
5. **Tests inline** — `#[cfg(test)] mod tests` ≥ 6 test (5 mandate + 1 optional comment-skip), cover full spec table.

#### Why use simple `str::find` thay vì compile regex?

Marker là literal string không có metacharacter. `str::find("<!-- INBOX_APPEND_START -->")` faster + ít dependency surface hơn `Regex::new(...).unwrap().find()`. Tuy nhiên:
- BACKLOG dùng từ "regex extract block" → Architect interpret là "logic-level extraction", không nhất thiết `regex` crate.
- Cargo.toml đã có `regex = "1"` (verify Anchor #4) → có thể dùng nếu Worker thấy `str::find` không đáp ứng edge case.
- **Architect decide:** dùng `str::find` cho marker locate (đơn giản, đủ); KHÔNG dùng `regex` crate cho P003. Lý do: zero metacharacter trong marker, regex chỉ thêm compile overhead.
- Nếu Worker challenge muốn dùng `regex::Regex::new(r"<!--\s*INBOX_APPEND_(START|END)\s*-->")` để tolerate whitespace variations → escalate Sếp. Spec hiện hard-code exact marker string.

> **V2 amendment (post-Turn 1):** CLAUDE.md Tech Stack line 192 + File Structure comment line 269 sẽ được cập nhật trong phiếu này để doctrine khớp implementation (xem Task 5 + 6). `regex` crate vẫn declared trong Cargo.toml + Tech Stack — reserved cho `inbox.rs` row pattern matching và future use, KHÔNG required cho sentinel.

#### Why `Vec<String>` not `Vec<&str>`?

Spawn-prompt note: lifetime `&str` tied to input efficient nhưng ergonomic painful. Caller (P004 `cli/parse_report.rs`) sẽ pass slice cho row parser; nếu `Vec<&str>` thì caller phải giữ `report_text` alive xuyên suốt — workable nhưng forced. `Vec<String>` cost = 1 `.to_string()` per row × ~10 row = trivial. Architect chọn ergonomics.

#### Comment-skip rule

ARCHITECTURE §3 inbox format có HTML comment placeholder (`<!--\n| row example\n-->`). Sentinel block reuse same convention — agent CÓ THỂ emit comment-out row làm example/placeholder. Parser skip line nào `trim_start()` bắt đầu bằng `<!--`. KHÔNG xử lý multiline comment block (`<!--\n...\n-->`) — chỉ skip single-line `<!-- ... -->`. Lý do: agent report convention emit single-line marker, không multiline. Nếu agent đổi format → discovery report + phiếu mới.

### Scope

- CHỈ tạo: `src/sentinel.rs`.
- CHỈ sửa: `src/main.rs` (thêm 1 dòng `mod sentinel;` sau `mod state;`).
- CHỈ update docs: `docs/CHANGELOG.md` (entry P003), `docs/ARCHITECTURE.md` §5 module layout (mark `sentinel.rs` shipped), **`CLAUDE.md` Tech Stack line 192 + File Structure comment line 269** (V2 — doctrine align per Turn 1 O1.1 ACCEPT).
- KHÔNG sửa: `Cargo.toml` (no new dep — `thiserror` + `regex` đã có per Anchor #4+#5; phiếu này thực tế chỉ dùng `thiserror`), `src/row.rs`, `src/state.rs`, `src/cli/*.rs` (subcmd vẫn stub printf TODO).
- KHÔNG tạo: `tests/sentinel_integration.rs` (inline test đủ — pure logic không cần integration).
- KHÔNG tạo: `tests/fixtures/agent-report-1.md` ở phiếu này. Fixture sẽ ship cùng P004 wire-in. P003 dùng inline string literal trong test.
- KHÔNG wire-in subcmd — `cli/parse_report.rs` vẫn stub. P004 sẽ wire `extract_block` + row parser vào `parse_report.rs`.

### Skills consulted

Architect ran `context7 query-docs /rust-lang/regex` and `/websites/rs_thiserror_2_0_18` to verify:
- **`regex` crate**: `captures_iter` + `Regex::new` API stable in 1.x. Anchor #6 cite.
- **`thiserror` 2.0.18**: `#[derive(Error)] #[error("...")]` derive macro syntax confirmed. Anchor #7 cite.

Result: phiếu KHÔNG dùng `regex` crate (str::find đủ), CHỈ dùng `thiserror`. Anchor #4 verifies `thiserror = "2"` đã có in Cargo.toml.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `src/sentinel.rs` chưa tồn tại | Glob `src/*.rs` + ARCHITECTURE §5 "Scaffold status" liệt `sentinel.rs` pending | `[unverified]` | ✅ `ls src/sentinel.rs 2>&1` → "No such file or directory" |
| 2 | `src/main.rs` hiện declare `mod cli; mod row; mod state;` (P002 ship) | P002 phiếu Task 3 fix lock này | `[needs Worker verify]` | ✅ `grep -n "^mod " src/main.rs` → 3 lines: cli (10), row (11), state (12) — alphabetical, no sentinel yet |
| 3 | ARCHITECTURE §5 list `src/sentinel.rs` là module riêng (không gộp inbox.rs) | Read docs/ARCHITECTURE.md §5 block 215-241 | `[verified]` | ✅ Worker re-verified ARCHITECTURE.md §5 line 230: `sentinel.rs` listed standalone |
| 4 | `Cargo.toml` đã có `thiserror = "2"` (P002 ship) | P002 spec mention thiserror 2 derive | `[needs Worker verify]` | ✅ `grep "^thiserror" Cargo.toml` → `thiserror = "2"` |
| 5 | `Cargo.toml` đã có `regex = "1"` | Spawn-prompt reference + ARCHITECTURE §intro Tech Stack mention `regex` | `[needs Worker verify]` | ✅ `grep "^regex" Cargo.toml` → `regex = "1"` |
| 6 | `regex` crate 1.x has `captures_iter` + `Regex::new` returning `Result` (context7-verified) | context7 `/rust-lang/regex` query — Architect skill consultation | `[verified]` | ✅ Re-confirmed: API stable; phiếu không dùng nhưng anchor holds |
| 7 | `thiserror` 2.0.18 supports `#[derive(Error, Debug)] enum ... #[error("...")] Variant` (context7-verified) | context7 `/websites/rs_thiserror_2_0_18` query — Architect skill consultation | `[verified]` | ✅ Re-confirmed: syntax used in phiếu skeleton matches crate docs |
| 8 | ARCHITECTURE §4 sentinel marker strings exact: `<!-- INBOX_APPEND_START -->` và `<!-- INBOX_APPEND_END -->` (single space inside) | Read docs/ARCHITECTURE.md §4 block 184-208 | `[verified]` | ✅ Worker re-verified: §4 line 197 + 200 exact spelling confirmed |
| 9 | CLAUDE.md line 192 contains Tech Stack `**Regex:** \`regex\` (sentinel marker...)` text | Worker cited explicitly in Turn 1 O1.1 | `[needs Worker verify]` | ✅ Line 192 exact text confirmed: `- **Regex:** \`regex\` (sentinel marker \`<!-- INBOX_APPEND_START/END -->\` parser)` — matches Task 5 "Tìm" string exactly, unique (1 hit) |
| 10 | CLAUDE.md line 269 contains File Structure comment `└── sentinel.rs          # Sentinel marker regex + extract block` | Worker cited explicitly in Turn 1 O1.1 | `[needs Worker verify]` | ✅ Line 269 exact text confirmed: `└── sentinel.rs          # Sentinel marker regex + extract block` — matches Task 6 "Tìm" string exactly, unique (1 hit) |

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.** Hiện không có ❌.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (Turn 1 RESPOND — O1.1 ACCEPT docs amendment, O1.2 ACK intentional stderr)

### Turn 1 — Worker Challenge

**Anchor verification (recap từ Verification Anchors):**
- Anchor #1: ✅ `sentinel.rs` chưa tồn tại — confirmed
- Anchor #2: ✅ `main.rs` có đúng 3 `mod` lines: cli, row, state (alphabetical) — confirmed
- Anchor #3: ✅ ARCHITECTURE §5 list `sentinel.rs` standalone line 230 — confirmed
- Anchor #4: ✅ `thiserror = "2"` trong Cargo.toml — confirmed
- Anchor #5: ✅ `regex = "1"` trong Cargo.toml — confirmed
- Anchor #6: ✅ `regex` crate API stable — confirmed (phiếu không dùng)
- Anchor #7: ✅ `thiserror` 2.x derive syntax đúng — confirmed
- Anchor #8: ✅ Exact marker strings confirmed in ARCHITECTURE §4

**Baseline `cargo check`:** exit 0 (clean, 0 warnings)

---

**Objections (Tầng 1):**

### O1.1 [shape] — `str::find` vs CLAUDE.md–declared `regex` crate for sentinel

**Architect:** Chọn `str::find` để locate markers (documented reasoning trong Giải pháp section).

**Reality:**
- `CLAUDE.md` line 192: `**Regex:** \`regex\` (sentinel marker \`<!-- INBOX_APPEND_START/END -->\` parser)` — Tech Stack section explicitly declares `regex` crate as the tool for sentinel parsing.
- `CLAUDE.md` File Structure comment line 269: `└── sentinel.rs          # Sentinel marker regex + extract block` — module description says "regex".
- `BACKLOG.md` P003 line 29: `regex extract block between ...` — BACKLOG uses "regex" for the extraction.

These three references (CLAUDE.md Tech Stack, CLAUDE.md module comment, BACKLOG) all point to `regex` crate being the declared implementation choice. Architect's V1 deviates from these declarations.

**Functional assessment:** `str::find` DOES handle all 5 BACKLOG acceptance cases correctly — the logic is sound. The issue is not correctness but **contract alignment**: CLAUDE.md is the project constitution, and a Worker maintaining future phiếu would read CLAUDE.md Tech Stack and expect sentinel logic to use `regex`, then be confused finding `str::find`.

**Proposed alternatives (Worker recommends A):**
- **A (Recommended):** Architect add a note to CLAUDE.md Tech Stack clarifying: "`regex` crate available; `sentinel.rs` uses `str::find` for literal markers (no metacharacters) — `regex` reserved for future inbox.rs pattern matching." This updates the constitution to match the implementation decision. CLAUDE.md line 192 and the File Structure comment line 269 both need minor edits. No code change needed — `str::find` approach accepted.
- **B:** Keep `str::find` in code AND update CLAUDE.md comment to reflect actual usage. Equivalent to A but Worker can carry out the CLAUDE.md edit in EXECUTE if Architect ack.
- **C:** Switch implementation to `regex` crate per CLAUDE.md declaration. Worker objects to this — it adds unnecessary compile-time overhead for literal string matching and Architect's reasoning is technically sound.

### O1.2 [mechanical] — `eprintln!` conflicts with Definition of Done item 5

**Architect:** Uses `eprintln!("warn: multiple INBOX_APPEND_START ...")` for the multi-START warning.

**Reality:** CLAUDE.md DoD item 5 states: `✅ Không còn dbg!(), eprintln!() debug, todo!(), unused imports, commented-out code`. The literal rule bans `eprintln!()`.

However: Worker interprets intent of DoD item 5 as targeting debug-cruft (`eprintln!` used during development and forgotten). The `eprintln!` in the proposed code is intentional operational stderr output, not debug cruft — consistent with Rust CLI conventions. No `tracing` or `log` crate is in `Cargo.toml`, so there is no project-established alternative.

**Proposed alternatives (Worker recommends A):**
- **A (Recommended):** Architect ack 1-line that this `eprintln!` is intentional operational stderr (not debug cruft) and is exempt from DoD item 5's spirit. No phiếu change needed. Worker notes it in Discovery Report so future phiếu do not auto-flag it.
- **B:** Architect amend CLAUDE.md DoD item 5 wording to `✅ Không còn dbg!(), eprintln!() *debug* (unintentional), todo!()...` to make the "debug cruft" intent explicit. This clarifies for all future Workers.

Note: O1.2 has no functional impact. If Architect chooses to treat this as [mechanical] (wording clarification only), Worker accepts that reclassification.

---

**Status:** ✅ RESPONDED — phiếu bumped to V2

### Turn 1 — Architect Response (phiếu V2)

**O1.1 [shape] → ACCEPT (alt A — docs-only amendment, no code change)**

Worker's contract-alignment concern is correct. CLAUDE.md is the project constitution; a stale Tech Stack line declaring `regex` for sentinel parsing while `sentinel.rs` uses `str::find` creates a doctrine/implementation drift that will mislead future Workers reading CLAUDE.md without opening the source.

Decision: **keep `str::find` in `sentinel.rs` (technical reasoning unchanged) AND amend CLAUDE.md as part of P003 docs gate (Sub-mech D — persistence/doctrine sync).**

Justification for keeping `str::find`:
1. Marker is a literal string with **zero metacharacters** — `<!-- INBOX_APPEND_START -->` and `<!-- INBOX_APPEND_END -->`. Regex provides no expressive benefit.
2. `str::find` is monomorphic + inlinable; no runtime regex compilation cost, no `Regex::new(...).unwrap()` panic surface.
3. Smaller dependency surface in the call graph — `sentinel.rs` is reused by both `cli/parse_report.rs` and MCP `parse_report` tool. Avoiding `regex` here keeps the parser hot path branch-free.
4. `regex` crate stays declared (Anchor #5) for `inbox.rs` row-table pattern matching and future use — not removed from `Cargo.toml`.

Action taken in V2:
- **Task 5 added (new):** amend CLAUDE.md line 192 Tech Stack `Regex` entry — clarify `regex` reserved for `inbox.rs`/pattern matching, `sentinel.rs` uses `str::find` for literal markers.
- **Task 6 added (new):** amend CLAUDE.md File Structure comment line 269 — change `# Sentinel marker regex + extract block` → `# Sentinel marker extract block (str::find on literal markers)`.
- **Files cần sửa table:** `CLAUDE.md` added as new row.
- **Ảnh hưởng header:** added `CLAUDE.md`.
- **Sub-mech áp dụng:** added D (persistence — doctrine sync).
- **Scope section:** added CLAUDE.md to "CHỈ update docs" list, removed from "KHÔNG sửa" list.
- **Anchors #9 + #10 added** — CLAUDE.md exact line citations to verify during EXECUTE.

**O1.2 [mechanical] → ACK (alt A — no phiếu change)**

DoD item 5 (`Không còn dbg!(), eprintln!() debug, ...`) bans **debug-cruft** `eprintln!` — calls accidentally left over from development. The multi-START warning in `sentinel.rs` is **intentional operational stderr output** following Rust CLI convention (`stderr` for diagnostic, `stdout` for data). It is part of the module's documented public contract (Constraint 5: "Warn wording cố định ... Tầng 1 public observable behavior").

No phiếu change. Worker should note this in Discovery Report so future static checks / phiếu reviews do not flag this `eprintln!`. If the project later adopts `tracing` or `log` crate (separate phiếu, not P003), this call site migrates with the rest.

DoD item 5 wording amendment (alt B) deferred — would require touching CLAUDE.md DoD which is broader scope than P003. Tracked as low-priority hygiene; not blocking.

---

**Verdict summary:**
- [O1.1] → ACCEPT (alt A) → V2 adds Task 5 + 6 (CLAUDE.md doctrine amendment); `str::find` implementation unchanged
- [O1.2] → DEFEND/ACK (alt A) → no phiếu change; `eprintln!` is intentional operational stderr, exempt from DoD item 5 "debug" spirit; Worker logs to Discovery Report

**Status:** ✅ RESPONDED — phiếu bumped to V2 — Worker may proceed to EXECUTE (no further CHALLENGE round needed; O1.1 was docs-only, O1.2 was wording clarification)

### Turn 2 — Worker Challenge (SURGICAL)

**Scope:** Surgical re-verify of V2 changes only (Turn 1 O1.1 ACCEPT: Task 5 + Task 6 + Anchor #9 + #10).

**Anchor #9 — CLAUDE.md line ~192 Tech Stack Regex entry:**
- Command run: `grep -n "Regex" CLAUDE.md`
- Result: line 192 = `- **Regex:** \`regex\` (sentinel marker \`<!-- INBOX_APPEND_START/END -->\` parser)`
- Task 5 "Tìm" string matches exactly. Unique (1 occurrence). ✅

**Anchor #10 — CLAUDE.md line ~269 File Structure sentinel.rs comment:**
- Command run: `grep -n "sentinel.rs" CLAUDE.md`
- Result: line 269 = `└── sentinel.rs          # Sentinel marker regex + extract block`
- Task 6 "Tìm" string matches exactly. Unique (verified via `grep -c "sentinel.rs.*Sentinel marker regex" CLAUDE.md` → 1). ✅

**Task 5 spec concreteness:** "Tìm/Thay bằng" strings present, unique, well-formed. Worker can execute via exact-match string replace without relying on line number. ✅

**Task 6 spec concreteness:** "Tìm/Thay bằng" strings present, unique. Whitespace preservation note included (10 spaces after `sentinel.rs` for column alignment). ✅

**Constraint 10 scope lock:** CHỈ 2 vị trí — both target strings verified unique. Lock is enforceable. ✅

**[mechanical] informational note (non-blocking):** Task 0 command `grep -n "^\*\*Regex:\*\*" CLAUDE.md` uses wrong anchor pattern — line 192 starts with `- **Regex:**` not `**Regex:**`, so this grep produces 0 hits. Per Task 5's own note: "Worker dùng exact-match string trên thay vì line number" — the false-zero is self-corrected by Task 5 instruction. No code or spec change needed; Worker notes in Discovery Report.

**Worker accepted V2 — no new challenges.** V2 changes (Task 5 + 6 + Anchor #9 + #10 + Constraint 10) are mechanically sound and executable as written.

**Status:** ✅ CONSENSUS REACHED — Ready for Chủ-nhà approval gate.

### Final consensus
- Phiếu version: V2
- Total turns: 1 Worker CHALLENGE + 1 Architect RESPOND + 1 Worker Turn 2 SURGICAL (consensus confirmed)
- Approved: pending Chủ-nhà approval gate — code execution may begin after gate

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
| B (capability) | `cargo check` | exit 0 | exit 0 | ✅ |
| B (capability) | `cargo test sentinel` | ≥6 tests pass | 6/6 pass | ✅ |
| C (migration) | (no schema change) | N/A | N/A | N/A |
| D (persistence) | `grep -l "sentinel" docs/ARCHITECTURE.md` | ≥1 hit (§5 module + §4 marker format) | 1 file hit | ✅ |
| D (persistence) | `grep -n "str::find" CLAUDE.md` | ≥1 hit (line 192 area, V2 amendment) | 2 hits (lines 192, 269) | ✅ |
| D (persistence) | `grep -n "Sentinel marker extract block" CLAUDE.md` | 1 hit (line 269 area, V2 amendment) | 1 hit (line 269) | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | 0 packages updated | ✅ |
| E (env drift) | `cargo build --release` clean target | exit 0 | exit 0, 0 warnings | ✅ |
| F (runtime state) | (no env var read, no token surface) | N/A | N/A | N/A |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech B)

**Mục tiêu:** Worker grep + read existing state TRƯỚC khi viết code, đối chiếu Anchor table.

**Lệnh chạy:**
```bash
ls src/sentinel.rs 2>&1                          # expect "No such file" (Anchor #1)
grep -n "^mod " src/main.rs                      # expect 3 lines: cli, row, state (Anchor #2)
grep "^thiserror" Cargo.toml                     # expect thiserror = "2..." (Anchor #4)
grep "^regex" Cargo.toml                         # expect regex = "1..." (Anchor #5)
grep -n "^\*\*Regex:\*\*" CLAUDE.md              # expect 1 hit near line 192 (Anchor #9, V2)
grep -n "sentinel.rs" CLAUDE.md                  # expect 1 hit near line 269 (Anchor #10, V2)
cargo check                                       # expect exit 0 (baseline clean)
```

**Output:** Worker fill kết quả vào Debate Log Turn 1 Anchor section. Nếu Anchor #2 cho thấy `mod sentinel;` ĐÃ tồn tại (drift) → STOP, escalate Sếp. Nếu Anchor #9/#10 line numbers lệch ±5 → OK, dùng exact match string thay vì line number để locate. Nếu lệch > 5 hoặc text không match → STOP, escalate.

### Task 1: Tạo `src/sentinel.rs`

**File:** `src/sentinel.rs` (new file)

**Tạo nội dung (Worker write từ skeleton sau, không copy nguyên — match project convention):**

```rust
//! Sentinel marker block extractor.
//!
//! Agent advisory-watch emits report markdown containing a block delimited by
//! `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->`. This module
//! locates the first such pair and extracts the raw row lines between them,
//! skipping blank and HTML-comment lines.
//!
//! See ARCHITECTURE.md §4 for the format contract.

use thiserror::Error;

/// Sentinel marker opening the appendable block.
pub const SENTINEL_START: &str = "<!-- INBOX_APPEND_START -->";

/// Sentinel marker closing the appendable block.
pub const SENTINEL_END: &str = "<!-- INBOX_APPEND_END -->";

/// Errors returned by [`extract_block`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum SentinelError {
    #[error("missing sentinel start marker `<!-- INBOX_APPEND_START -->` in report")]
    MissingStartMarker,
    #[error("missing sentinel end marker `<!-- INBOX_APPEND_END -->` after start")]
    MissingEndMarker,
}

/// Extract raw row lines from the first sentinel block in `report_text`.
///
/// Returns each non-blank, non-comment line between the first
/// `<!-- INBOX_APPEND_START -->` and the next `<!-- INBOX_APPEND_END -->`.
/// If multiple START markers exist, only the first pair is used; a warning
/// is emitted to stderr.
///
/// # Errors
/// - [`SentinelError::MissingStartMarker`] if START not found.
/// - [`SentinelError::MissingEndMarker`] if START found but no END after it.
pub fn extract_block(report_text: &str) -> Result<Vec<String>, SentinelError> {
    // 1. Locate first START marker.
    let start_idx = report_text
        .find(SENTINEL_START)
        .ok_or(SentinelError::MissingStartMarker)?;
    let after_start = start_idx + SENTINEL_START.len();

    // 2. Locate END marker AFTER first START.
    let end_offset = report_text[after_start..]
        .find(SENTINEL_END)
        .ok_or(SentinelError::MissingEndMarker)?;
    let end_idx = after_start + end_offset;

    // 3. Warn if extra START markers exist beyond the first pair.
    let remainder = &report_text[end_idx + SENTINEL_END.len()..];
    let extra_starts = remainder.matches(SENTINEL_START).count();
    if extra_starts > 0 {
        eprintln!(
            "warn: multiple INBOX_APPEND_START markers found ({} extra after first pair); using first pair only",
            extra_starts
        );
    }

    // 4. Slice between markers + filter lines.
    let block = &report_text[after_start..end_idx];
    let rows: Vec<String> = block
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| {
            let trimmed = l.trim_start();
            !trimmed.is_empty() && !trimmed.starts_with("<!--")
        })
        .map(|l| l.to_string())
        .collect();

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_two_rows() {
        let report = "\
some preamble\n\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-2026-0001 | url1 | pkg1 | f:1 | High | open | - |\n\
| 2026-05-28 | CVE-2026-0002 | url2 | pkg2 | f:2 | Medium | open | - |\n\
<!-- INBOX_APPEND_END -->\n\
trailing\n";
        let rows = extract_block(report).expect("should parse");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("CVE-2026-0001"));
        assert!(rows[1].contains("CVE-2026-0002"));
    }

    #[test]
    fn empty_block_returns_empty_vec() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("empty block is valid");
        assert!(rows.is_empty());
    }

    #[test]
    fn missing_start_marker_errors() {
        let report = "no markers here at all, just prose";
        let err = extract_block(report).unwrap_err();
        assert_eq!(err, SentinelError::MissingStartMarker);
    }

    #[test]
    fn missing_end_marker_errors() {
        let report = "<!-- INBOX_APPEND_START -->\n| row | ... |\n";
        let err = extract_block(report).unwrap_err();
        assert_eq!(err, SentinelError::MissingEndMarker);
    }

    #[test]
    fn multiple_start_uses_first_pair() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-FIRST | u | p | f:1 | High | open | - |\n\
<!-- INBOX_APPEND_END -->\n\
between\n\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-SECOND | u | p | f:2 | Low | open | - |\n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("first pair valid");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("CVE-FIRST"));
        assert!(!rows[0].contains("CVE-SECOND"));
    }

    #[test]
    fn block_with_blank_and_comment_lines_skipped() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
\n\
| 2026-05-28 | CVE-REAL | u | p | f:1 | High | open | - |\n\
<!-- placeholder example -->\n\
   \n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("ok");
        assert_eq!(rows.len(), 1, "only real row kept");
        assert!(rows[0].contains("CVE-REAL"));
    }
}
```

**Lưu ý:**
- `&str` ↔ `String` boundary: input `&str`, output `Vec<String>` (cố ý, see Giải pháp).
- Đừng dùng `regex` crate cho marker locate — `str::find` đủ và nhanh hơn. Anchor #5 chỉ note `regex` available, không yêu cầu dùng.
- `trim_end()` line giữ leading whitespace (markdown pipe row có thể bắt đầu bằng `|`, không cần trim_start). Filter blank dùng `trim_start().is_empty()` để bắt cả `"   "` (whitespace-only).
- Comment-skip: chỉ check `trim_start().starts_with("<!--")` — đơn giản, KHÔNG check `-->` ở cuối (sentinel marker chính cũng start với `<!--` nhưng marker đã được consumed bởi `find()`, không vào loop filter).
- `eprintln!` warn format chính xác như string trong code (Worker không tự đổi wording — sẽ là Tầng 1 escalation nếu cần). **Per Turn 1 O1.2 ACK: đây là intentional operational stderr, exempt from DoD item 5 debug-cruft ban — Worker note trong Discovery Report.**
- Test gồm 6 case (5 mandate + 1 optional comment-skip). Đủ Acceptance "≥5".

### Task 2: Đăng ký module trong `src/main.rs`

**File:** `src/main.rs`

**Tìm** (P002 ship, expect dạng này):
```rust
mod cli;
mod row;
mod state;
```

**Thay bằng:**
```rust
mod cli;
mod row;
mod sentinel;
mod state;
```

**Lưu ý:**
- Thứ tự alphabetical (cli < row < sentinel < state). Match Rust idiom + P002 đã alphabetical (cli < row < state).
- Nếu Worker grep ra `mod` order khác (P002 ship khác spec) — DỪNG, escalate Sếp (drift Anchor #2).
- KHÔNG thêm `pub mod sentinel;` — keep crate-private; P004 sẽ `use crate::sentinel::extract_block` qua module path (re-export not needed).

### Task 3: Update `docs/ARCHITECTURE.md` §5 module status

**File:** `docs/ARCHITECTURE.md`

**Tìm** (ARCHITECTURE.md "Scaffold status" block hiện tại):
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `sentinel.rs`, `mcp/`, `error.rs`.
```

**Thay bằng:**
```markdown
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- P003: `sentinel.rs` (`extract_block` + `SentinelError`) shipped — pure logic, not yet wired into `cli/parse_report.rs`.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `mcp/`, `error.rs`.
```

**Lưu ý:**
- Date `2026-05-28` giữ nguyên — đây là "scaffold status" snapshot, không phải per-phiếu ship date. Nếu Worker ship ngày khác → KHÔNG đổi date này, ghi note ở CHANGELOG.
- KHÔNG đổi §4 sentinel marker format spec (đã đúng). KHÔNG đổi §5 module layout tree (đã list `sentinel.rs`).

### Task 4: Append CHANGELOG entry

**File:** `docs/CHANGELOG.md`

**Thêm entry (newest at top, theo convention P001 + P002 đã set):**

```markdown
## P003 — Sentinel parser (2026-MM-DD)

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
```

**Lưu ý:**
- `2026-MM-DD` → Worker thay bằng ngày ship thực tế.
- Format match P001/P002 entry style (Worker grep `docs/CHANGELOG.md` xác nhận trước khi append).

### Task 5: Amend CLAUDE.md Tech Stack — `Regex` entry (V2, Turn 1 O1.1 ACCEPT)

**File:** `CLAUDE.md`

**Tìm** (around line 192, exact-match string):
```markdown
- **Regex:** `regex` (sentinel marker `<!-- INBOX_APPEND_START/END -->` parser)
```

**Thay bằng:**
```markdown
- **Regex:** `regex` (reserved for `inbox.rs` row/table pattern matching; **`sentinel.rs` uses `str::find` on literal markers `<!-- INBOX_APPEND_START/END -->` — no metacharacters, no regex needed**)
```

**Lưu ý:**
- Worker dùng exact-match string trên thay vì line number — line 192 là approximate (CLAUDE.md có thể drift). Nếu grep `^\*\*Regex:\*\*` CLAUDE.md trả về 0 hits hoặc > 1 hit → STOP, escalate.
- KHÔNG đổi `regex` crate version hoặc bỏ khỏi list. Crate vẫn declared cho `inbox.rs` + future use.
- KHÔNG đổi Tech Stack lines khác (chỉ dòng Regex).

### Task 6: Amend CLAUDE.md File Structure comment — `sentinel.rs` line (V2, Turn 1 O1.1 ACCEPT)

**File:** `CLAUDE.md`

**Tìm** (around line 269, inside the File Structure ASCII tree):
```
└── sentinel.rs          # Sentinel marker regex + extract block
```

**Thay bằng:**
```
└── sentinel.rs          # Sentinel marker extract block (str::find on literal markers)
```

**Lưu ý:**
- Match exact whitespace của tree (Worker preserve 10 spaces sau `sentinel.rs` để cột `#` align với các module khác).
- Nếu Worker grep `sentinel.rs.*Sentinel marker regex` CLAUDE.md trả về 0 hits → STOP, escalate (drift Anchor #10).
- KHÔNG đổi các comment khác trong tree (chỉ dòng sentinel.rs).
- Sau khi Task 5 + 6 xong: doctrine align với implementation. Future Worker đọc CLAUDE.md sẽ không nhầm.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/sentinel.rs` | Task 1: new file (module + enum + fn + 6 tests) |
| `src/main.rs` | Task 2: add `mod sentinel;` (1 line) |
| `docs/ARCHITECTURE.md` | Task 3: update §5 Scaffold status block (P003 line) |
| `docs/CHANGELOG.md` | Task 4: prepend P003 entry |
| `CLAUDE.md` | **V2 (Turn 1 O1.1 ACCEPT):** Task 5 amend Tech Stack `Regex` entry (~line 192); Task 6 amend File Structure comment for `sentinel.rs` (~line 269) — doctrine align with `str::find` impl |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `Cargo.toml` | `thiserror = "2..."` already present (Anchor #4); `regex = "1..."` still declared (Anchor #5); KHÔNG thêm dep mới |
| `src/row.rs` | Standalone, sentinel KHÔNG import row::AdvisoryRow (raw `String` boundary — P004 sẽ parse) |
| `src/state.rs` | Standalone, sentinel KHÔNG touch state |
| `src/cli/parse_report.rs` | Vẫn stub printf TODO — P004 wire-in |
| `docs/PROJECT.md` | Phase status không đổi (vẫn Phase 1 in-progress) |
| `docs/RULES.md` | KHÔNG đổi (no doctrine change — V2 amendment chỉ Tech Stack + File Structure trong CLAUDE.md) |

---

## Luật chơi (Constraints)

1. **No new deps.** Phiếu chỉ dùng `thiserror` (đã có). KHÔNG add `regex`, `once_cell`, `lazy_static`, etc. Anchor #5 ghi `regex` available nhưng phiếu chọn `str::find`. **V2 note:** CLAUDE.md Tech Stack `Regex` entry vẫn list `regex` crate — không remove, chỉ làm rõ scope (Task 5).
2. **No `unsafe { ... }` block** — pure safe Rust. Escalate Sếp nếu Worker thấy cần (impossibly không cần ở phiếu này).
3. **No `pub` leak ngoài cần thiết.** Public surface: `SENTINEL_START`, `SENTINEL_END`, `SentinelError`, `extract_block`. Internal helper (nếu Worker tạo) phải `fn` private (no `pub`).
4. **Comment skip rule:** chỉ skip line `trim_start().starts_with("<!--")` — KHÔNG xử lý multiline comment. Nếu agent đổi format → Discovery Report + phiếu mới (KHÔNG tự fix).
5. **Warn wording cố định** — `eprintln!` message chính xác text trong Task 1 skeleton. Đây là Tầng 1 (public observable behavior) — Worker đổi wording = drift, escalate. **V2 note (Turn 1 O1.2 ACK):** `eprintln!` là intentional operational stderr, exempt from DoD item 5 (`eprintln!()` debug ban). Worker log fact này vào Discovery Report.
6. **Test inline `#[cfg(test)] mod tests`** — KHÔNG tạo `tests/sentinel_*.rs` integration test (P004 ship fixture + integration sau).
7. **Match existing module style** — Worker grep `src/row.rs` + `src/state.rs` (P002 ship) để align doc comment style (`//! ...` module-level, `///` item-level).
8. **Conventional commits** — `feat(P003): add sentinel block extractor` (hoặc tương tự match P001/P002 commit msg pattern; Worker grep `git log --oneline -5` confirm).
9. **Docs Gate Tầng 1 mandatory** — vì module mới + public API contract mới. ARCHITECTURE §5 update + CLAUDE.md Tech Stack/File Structure amendment (Task 5+6) bắt buộc. CHANGELOG mandatory.
10. **CLAUDE.md amendment scope cố định (V2)** — CHỈ 2 vị trí (line ~192 Tech Stack Regex line + line ~269 File Structure comment). KHÔNG touch DoD section, KHÔNG touch Hard Stops, KHÔNG touch other Tech Stack entries. Mọi amendment khác = scope creep, escalate.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — all pass (≥6 new tests trong `sentinel::tests`)
- [ ] `cargo test sentinel` — 6 tests pass, run < 1s
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] Compile-check chỉ module: `cargo check -p advisory-inbox` exit 0.
- [ ] Stdout/stderr boundary: trong test `multiple_start_uses_first_pair`, run với `cargo test multiple_start -- --nocapture` → confirm `warn:` line xuất hiện trên stderr (not stdout).

### Regression
- [ ] `advisory-inbox parse-report` (stub from P001) vẫn exit 0 với TODO message — sentinel module chưa wire-in, KHÔNG được break stub behavior.
- [ ] `advisory-inbox --help` vẫn show 8 subcmd như P001.
- [ ] `cargo test row` (P002 tests) vẫn pass — sentinel.rs không impact row.rs.
- [ ] `cargo test state` (P002 tests) vẫn pass.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P003 entry prepended (newest at top), date filled.
- [ ] `docs/ARCHITECTURE.md` §5 — Scaffold status updated với P003 line.
- [ ] **`CLAUDE.md` — Tech Stack `Regex` line amended (Task 5) + File Structure comment for `sentinel.rs` amended (Task 6).** Verify: `grep "str::find on literal markers" CLAUDE.md` → 2 hits (1 in Tech Stack, 1 in File Structure tree comment).
- [ ] `README.md` — KHÔNG cần update (sentinel internal, no CLI surface change at P003 — P004 sẽ update khi parse-report wire-in).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P003.md` — full report written (anchor verify results, any drift found, decisions made). **MUST include:** O1.2 ACK record — `eprintln!` in `sentinel.rs::extract_block` is intentional operational stderr (per Turn 1 RESPOND), exempt from DoD item 5 debug-cruft ban; future phiếu/static checks should not flag.
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top).
- [ ] Sub-mechanism B Verification Trace filled (table above): `cargo check` ✅, `cargo test sentinel` ✅, `cargo update --dry-run` clean, `cargo build --release` clean.
- [ ] **Sub-mechanism D Verification Trace filled (V2):** `grep -n "str::find" CLAUDE.md` ≥1 hit, `grep -n "Sentinel marker extract block" CLAUDE.md` 1 hit (line 269 area).
- [ ] D check (existing): `grep -l "sentinel" docs/ARCHITECTURE.md` → ≥1 hit (§4 + §5).
