# PHIẾU P002: Row + state types (serde)

> **ID format:** `P002` — counter `.phieu-counter` đã = 2 sau P001 ship.
> **Filename:** `docs/ticket/P002-row-state-types.md`
> **Branch:** `feat/P002-row-state-types`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation cho P003 sentinel parser, P004 parse-report wire-in, P005 dedup — tất cả consume `AdvisoryRow` + `StateFile`)
> **Ảnh hưởng:** `src/row.rs` (new), `src/state.rs` (new), `src/main.rs` (add 2 `mod` declarations), `docs/ARCHITECTURE.md` §5 module status, `docs/CHANGELOG.md`
> **Dependency:** P001 (CLI scaffold) — đã ship 2026-05-28
> **Lane:** Normal (state schema lock-in per RULES.md §1 — KHÔNG Fast lane vì state schema change)
> **Sub-mech áp dụng:** B (capability — `cargo check` + `cargo test`), C (state schema_version bump rule — lock = 1 for this phiếu)

---

## Context

### Vấn đề hiện tại

P001 ship CLI scaffold xong, 8 subcmd stub đều printf TODO. Phase 1 tiếp theo (P003 sentinel parser → P004 parse-report → P005 dedup → P006 append) **cần 2 type concrete để truyền data giữa subcmd**:

1. **`AdvisoryRow`** — 1 row trong inbox markdown (8 cột per ARCHITECTURE §3). Sentinel parser emit `Vec<AdvisoryRow>` → dedup filter → append render markdown.
2. **`StateFile`** — schema state JSON (`schema_version`, `last_scan_at`, `seen_advisories`, `agent_version`) per ARCHITECTURE §2. dedup reads, migrate-state writes, state-backfill mutates.

Hiện tại `src/row.rs`, `src/state.rs` chưa tồn tại (P001 Discovery Report xác nhận chỉ `src/main.rs` + `src/cli/`). Phiếu sau (P003+) wire-in logic sẽ stall nếu types chưa lock.

Reference BACKLOG.md item P002:
- Scope: `src/row.rs` `AdvisoryRow` struct (8 fields per ARCHITECTURE §3). `src/state.rs` `StateFile` struct (schema_version, last_scan_at, seen_advisories, agent_version). Serde derive. Unit tests roundtrip.
- Acceptance: Type compile clean, roundtrip JSON test passes.
- Sub-mech checks: B, C (state schema_version bump rule).

### Giải pháp

Tạo 2 module mới, mỗi module 1 type + enum phụ trợ + serde derive + inline `#[cfg(test)] mod tests` cho JSON roundtrip:

1. **`src/row.rs`** — public struct `AdvisoryRow` 8 field; public enum `Status` (`open`/`processed`/`dismissed`); public enum `Severity` (`Critical`/`High`/`Medium`/`Low`). Cả 3 derive `Serialize, Deserialize, Debug, Clone, PartialEq, Eq`. Status dùng `#[serde(rename_all = "lowercase")]`, Severity dùng `#[serde(rename_all = "PascalCase")]` (trùng default — explicit để document intent).
2. **`src/state.rs`** — public struct `StateFile` 4 field. Derive `Serialize, Deserialize, Debug, Clone, PartialEq, Eq`. `pub const SCHEMA_VERSION: u32 = 1` constant để lock current version + làm anchor cho bump rule (P007 migrate-state sẽ check).
3. **`src/main.rs`** — chỉ thêm 2 dòng `mod row;` + `mod state;` (sau dòng `mod cli;` hiện có). Không touch clap shape, không touch dispatch.
4. **Test** — inline trong mỗi file (per P001 Discovery Report convention + ARCHITECTURE §8 — unit test trong file, integration test mới ra `tests/`). Mỗi file ≥2 roundtrip test: serialize → deserialize → assert eq + 1 known-JSON parse test (fixture string match ARCHITECTURE §2/§3 example).

Lý do tách 2 file (KHÔNG gộp `types.rs`):
- ARCHITECTURE §5 explicit list `src/row.rs` + `src/state.rs` là 2 module riêng.
- P005 dedup chỉ cần `state::StateFile` + `row::AdvisoryRow` — import rõ ràng theo concern.
- P007 migrate-state chỉ touch `state.rs`, không animal touch `row.rs`.

Lý do KHÔNG dùng `BTreeSet<String>` cho `seen_advisories`:
- Spawn-prompt yêu cầu: "BTreeSet internally — but stored as Vec for serde stability". Vec stable order khi serialize (insertion order) → diff-friendly cho state file. Dedup logic ở P005 sẽ convert `Vec → HashSet` in-memory rồi back, KHÔNG ép schema dùng Set.
- ARCHITECTURE §2 example dùng JSON array (`[ "CVE-...", ... ]`) — Vec serialize ra đúng shape.

Lý do `chrono::NaiveDate` cho `Row.date` (KHÔNG `DateTime<Utc>`):
- ARCHITECTURE §3 row format: `2026-05-28` (date only, không có time). chrono `NaiveDate` default serde = `"YYYY-MM-DD"` string (context7-verified).
- `StateFile.last_scan_at` thì KHÁC: ARCHITECTURE §2 example là `"2026-05-28T09:51:35Z"` (full RFC 3339). Dùng `DateTime<Utc>` — chrono default serde = RFC 3339 string (context7-verified).

### Scope

- CHỈ tạo: `src/row.rs`, `src/state.rs`.
- CHỈ sửa: `src/main.rs` (thêm 2 dòng `mod`).
- CHỈ update docs: `docs/CHANGELOG.md` (entry P002), `docs/ARCHITECTURE.md` §5 module layout (mark `row.rs` + `state.rs` shipped).
- KHÔNG sửa: `Cargo.toml` (chrono 0.4 đã có feature `serde`, serde đã có `derive` — verified Anchor #1), `docs/PROJECT.md` (phase chưa đổi), `docs/RULES.md`, `CLAUDE.md`.
- KHÔNG tạo: `src/sentinel.rs`, `src/inbox.rs`, `src/error.rs`, `src/mcp/` (phiếu sau).
- KHÔNG wire-in subcmd logic — `src/cli/*.rs` vẫn là stub printf TODO. Types có nhưng chưa ai dùng.
- KHÔNG tạo `tests/types_roundtrip.rs` riêng. Inline `#[cfg(test)] mod tests` per file (per P001 Discovery convention + ARCHITECTURE §8).
- KHÔNG add dep mới.

### Skills consulted

Architect ran `context7 query-docs /websites/rs_chrono_chrono` for chrono serde behavior. Confirmed:
- `DateTime<Utc>` default serde = RFC 3339 string (parse + emit). Crate feature `serde` đủ — không cần `serde_with`.
- `NaiveDate` default serde = `"YYYY-MM-DD"` string. Cùng feature gate.
- Alternate format (timestamp i64) ở `chrono::serde::ts_seconds` — KHÔNG dùng ở phiếu này (spec yêu cầu RFC 3339).

See Anchor #2 + #3.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect tool envelope = Read + Write + Glob + context7. KHÔNG Bash/Grep. Mọi anchor đụng vào source code = `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `Cargo.toml` đã có `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `chrono = { version = "0.4", features = ["serde"] }`. KHÔNG cần add dep mới. | Architect đã Read `Cargo.toml` dòng 14-17 | `[verified]` | ✅ 3 dep đủ feature flag. `chrono` feature `serde` confirmed dòng 17. |
| 2 | `chrono::DateTime<chrono::Utc>` default serde = RFC 3339 string (parse + emit). Example: `"2026-05-28T09:51:35Z"`. | Architect query context7 `/websites/rs_chrono_chrono` về "serde feature DateTime Utc serialize" | `[verified]` | ✅ Docs xác nhận: "Deserialize an RFC 3339 formatted string into a `DateTime<Utc>`". Default Serialize/Deserialize impl available khi feature `serde` enabled. |
| 3 | `chrono::NaiveDate` default serde = `"YYYY-MM-DD"` ISO 8601 string. Example: `"2026-05-28"`. | context7 query về "NaiveDate serde ISO 8601" | `[verified]` | ✅ chrono docs: NaiveDate/NaiveDateTime serialize ISO 8601 string khi feature `serde`. Cùng pattern với NaiveDateTime mà context7 trả về. |
| 4 | ARCHITECTURE §3 row format 8 cột theo thứ tự: Date / Advisory ID / Source URL / Package / File:Line / Severity / Status / Note. | Architect Read `docs/ARCHITECTURE.md` dòng 154-181 | `[verified]` | ✅ Confirmed dòng 165-167 (table header + example row). |
| 5 | ARCHITECTURE §2 state schema 4 field theo thứ tự: `schema_version: u32`, `last_scan_at: ISO-8601 UTC`, `seen_advisories: Vec<String>`, `agent_version: String`. Current schema_version = 1. | Architect Read `docs/ARCHITECTURE.md` dòng 123-146 | `[verified]` | ✅ Confirmed dòng 130-139 (JSON example) + dòng 143-146 (constraints). |
| 6 | Status enum 3 variant `open` / `processed` / `dismissed` (lowercase). Severity enum 4 variant `Critical` / `High` / `Medium` / `Low` (PascalCase). | Architect Read `docs/ARCHITECTURE.md` dòng 179-180 | `[verified]` | ✅ Hai dòng spec explicit cứng. ARCHITECTURE §3 dòng 180: "Severity enum: Critical / High / Medium / Low (upstream official only per RULES.md §X)". |
| 7 | `src/row.rs` + `src/state.rs` chưa tồn tại. Greenfield 2 file mới. | Architect Glob `src/*.rs` — và P001 Discovery Report ghi rõ pending files. | `[needs Worker verify]` | ✅ `ls -la src/*.rs` = only `src/main.rs`. No `row.rs`/`state.rs`. Greenfield confirmed. |
| 8 | `src/main.rs` hiện chỉ có `mod cli;` declaration (per P001 Discovery). Thêm `mod row;` + `mod state;` không conflict. | P001 Discovery Report dòng 73 ("`src/main.rs` Rewritten — clap derive Cli + Commands (8 variants) + sync main dispatch"). | `[needs Worker verify]` | ✅ `grep -c '^mod '` = 1. Exact: line 10 `mod cli;`. No conflict. |
| 9 | `chrono::Utc` import path: `use chrono::{DateTime, Utc};` cho `last_scan_at: DateTime<Utc>`. | chrono docs canonical re-export | `[verified]` | ✅ chrono root re-exports `DateTime`, `Utc`, `NaiveDate`. Không cần `chrono::offset::Utc` form. |
| 10 | `#[serde(rename_all = "lowercase")]` áp dụng cho enum variant naming (canonical serde feature). `#[serde(rename_all = "PascalCase")]` cho Severity — trùng default Rust variant naming nhưng explicit document intent. | serde derive doc | `[verified]` | ✅ serde attribute canonical. Status `Open` → `"open"`, `Dismissed` → `"dismissed"`. Severity `Critical` → `"Critical"` (no change, nhưng attribute explicit). |
| 11 | `seen_advisories: Vec<String>` (KHÔNG `BTreeSet`/`HashSet`). Dedup logic in-memory tại P005 sẽ chuyển sang Set. | Spawn-prompt explicit + ARCHITECTURE §2 dòng 145 "Dedup via BTreeSet internal" → "internal" = runtime, không phải schema. | `[verified]` | ✅ Schema = Vec để serde stable order. Internal dedup là implementation detail của P005. |
| 12 | `pub const SCHEMA_VERSION: u32 = 1` trong `src/state.rs` để P007 migrate-state có anchor compare. | Sub-mech C bump rule (CLAUDE.md): "schema_version: u32, current = 1. Bump khi breaking change". | `[verified]` | ✅ Constant ở module level. P002 lock = 1; P007 sẽ thêm logic compare. |
| 13 | Inline `#[cfg(test)] mod tests` pattern (không tạo `tests/types_roundtrip.rs`). | ARCHITECTURE §8: "Unit tests (per module): `#[cfg(test)]` in same file. Pure logic". | `[verified]` | ✅ row.rs + state.rs đều là pure logic (struct + serde). Inline đúng convention. |

**Note:** Anchor #7 + #8 là 2 điểm Worker phải verify trước EXECUTE. Nếu `src/row.rs` hoặc `src/state.rs` đã tồn tại → Hard Stop, escalate Architect (repo state khác giả định). Nếu `src/main.rs` có thêm `mod` declaration ngoài `mod cli;` → re-read + adjust Task 3.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no challenges.**

**Anchor verification (recap):**
- Anchor #1: ✅ `Cargo.toml` lines 15-17 confirmed — `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `chrono = { version = "0.4", features = ["serde"] }`. Worker re-grepped.
- Anchor #2: ✅ Accepting Architect's context7 verification — chrono docs confirm `DateTime<Utc>` RFC 3339 serde when feature `serde` enabled.
- Anchor #3: ✅ Accepting Architect's context7 verification — `NaiveDate` ISO 8601 string serde confirmed.
- Anchor #4: ✅ ARCHITECTURE.md §3 lines 178-180 confirmed — 8-column row: Date / Advisory ID / Source URL / Package / File:Line / Severity / Status / Note.
- Anchor #5: ✅ ARCHITECTURE.md §2 lines 130-146 confirmed — 4-field StateFile JSON with `schema_version: u32 = 1`.
- Anchor #6: ✅ ARCHITECTURE.md §3 line 179-180 confirmed — Status 3 variants lowercase, Severity 4 variants PascalCase.
- Anchor #7: ✅ `ls src/*.rs` returns only `src/main.rs` (3467 bytes, 2026-05-28). No `row.rs` or `state.rs` exist. Greenfield confirmed.
- Anchor #8: ✅ `grep -c '^mod ' src/main.rs` = 1. Exact: line 10 `mod cli;`. Insertion of `mod row;` + `mod state;` after line 10 has no conflict.
- Anchor #9: ✅ Accepting Architect's chrono re-export verification — `use chrono::{DateTime, Utc}` is canonical.
- Anchor #10: ✅ Accepting Architect's serde derive doc verification — `rename_all = "lowercase"` and `rename_all = "PascalCase"` are canonical serde attributes.
- Anchor #11: ✅ `Vec<String>` schema design confirmed in ARCHITECTURE §2 line 145 — "Dedup via BTreeSet internal" = runtime only, not schema shape.
- Anchor #12: ✅ `pub const SCHEMA_VERSION: u32 = 1` — consistent with Sub-mech C bump rule doctrine in CLAUDE.md.
- Anchor #13: ✅ ARCHITECTURE §8 confirmed — inline `#[cfg(test)] mod tests` for unit tests, `tests/` only for integration tests.

**Objections (Tầng 1 only):** None.

**Task 0 commands run:**
- `ls -la src/*.rs` → only `src/main.rs` ✅
- `grep -c '^mod ' src/main.rs` → `1` ✅
- `grep -E '^(serde|serde_json|chrono)' Cargo.toml` → 3 hits, versions 1/1/0.4, chrono carries `features = ["serde"]` ✅
- `cargo check` → exit 0 (P001 baseline clean, compiled successfully) ✅

Ready for Chủ nhà approval gate.

**Status:** ✅ WORKER ACCEPTED V1

### Turn 1 — Architect Response
*(Architect fill khi invoked RESPOND mode.)*

### Final consensus
- Phiếu version: V<N>
- Total turns: <count>
- Approved (autonomous narrate or Sếp gate): [date] — code execution may begin

---

## Debug Log (advisory-cron specific)

> Worker emit observability records during EXECUTE.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

---

## Verification Trace (Sub-mechanism A-E checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A (no hook/cron) | N/A | N/A | N/A |
| B (capability) | `cargo check` | exit 0 | exit 0 | ✅ |
| B (capability) | `cargo test --all` | ≥6 tests pass (3 row + 3 state) | 8 passed | ✅ |
| B (capability) | `cargo clippy --all-targets -- -D warnings` | clean | clean | ✅ |
| C (migration) | `grep -n "SCHEMA_VERSION" src/state.rs` | =1 hit, value `1` | line 18: `pub const SCHEMA_VERSION: u32 = 1` | ✅ |
| C (migration) | `grep -n "schema_version" src/state.rs` | ≥2 hits (struct field + const) | 5 hits | ✅ |
| D (persistence) | `grep -l "AdvisoryRow\|StateFile" docs/ARCHITECTURE.md` | ≥1 hit (§5 module layout updated) | docs/ARCHITECTURE.md | ✅ |
| E (env drift) | `cargo update --dry-run` | no major bump | 0 packages bumped | ✅ |
| E (env drift) | `cargo build --release` clean target | exit 0 | exit 0, zero warnings | ✅ |
| F (runtime state) | `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' .git/config` | 0 hits | 0 hits OK | ✅ |

---

## Nhiệm vụ

### Task 0 — Verification Anchors (run BEFORE Task 1)

**Worker MUST run these checks before EXECUTE. Hard-stop on failure.**

1. `ls -la src/*.rs` → expect `src/main.rs` only. `src/row.rs` + `src/state.rs` MUST NOT exist (greenfield 2 file).
2. `grep -c '^mod ' src/main.rs` → expect `1` (just `mod cli;` from P001).
3. `grep -E '^(serde|serde_json|chrono)' Cargo.toml` → expect 3 hits with versions matching `1`/`1`/`0.4` and chrono carries `features = ["serde"]`.
4. `cargo check` (baseline before any edit) → expect exit 0 (P001 ship was clean).

If any anchor ❌ → Hard Stop, escalate Architect.

### Task 1: Create `src/row.rs` (AdvisoryRow + Status + Severity)

**File:** `src/row.rs` (new file).

**Thêm:**
```rust
//! AdvisoryRow type — 1 row in inbox markdown table (ARCHITECTURE.md §3).
//!
//! Serialized as JSON between subcommands (parse-report → dedup → append).
//! Status/Severity enums lock the wire format per upstream advisory convention.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Status of an advisory row — Sếp gates `open` → `processed`/`dismissed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Processed,
    Dismissed,
}

/// Severity per upstream advisory (Critical/High/Medium/Low only — RULES.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// One row in the inbox markdown table — 8 columns per ARCHITECTURE.md §3.
///
/// Column order (markdown): Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note.
/// JSON field order matches struct field order (serde default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvisoryRow {
    /// Date the advisory was observed (YYYY-MM-DD).
    pub date: NaiveDate,
    /// Advisory ID (CVE-YYYY-NNNNN, GHSA-xxxx-yyyy, RUSTSEC-YYYY-NNNN, etc.).
    pub advisory_id: String,
    /// Upstream advisory URL.
    pub source_url: String,
    /// Affected package spec (e.g., `next@<15.5.17`).
    pub package: String,
    /// Code location (`path/to/file.ext:line` or `indirect` for transitive).
    pub file_line: String,
    /// Severity per upstream.
    pub severity: Severity,
    /// Current status (open until Sếp gates).
    pub status: Status,
    /// Free-form note (`-` placeholder when empty).
    pub note: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> AdvisoryRow {
        AdvisoryRow {
            date: NaiveDate::from_ymd_opt(2026, 5, 28).expect("valid date"),
            advisory_id: "CVE-2026-9999".to_string(),
            source_url: "https://nvd.nist.gov/vuln/detail/CVE-2026-9999".to_string(),
            package: "next@<15.5.17".to_string(),
            file_line: "src/middleware.ts:42".to_string(),
            severity: Severity::High,
            status: Status::Open,
            note: "-".to_string(),
        }
    }

    #[test]
    fn row_roundtrip_json() {
        let row = sample_row();
        let json = serde_json::to_string(&row).expect("serialize");
        let back: AdvisoryRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(row, back);
    }

    #[test]
    fn status_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Status::Open).unwrap(), "\"open\"");
        assert_eq!(serde_json::to_string(&Status::Processed).unwrap(), "\"processed\"");
        assert_eq!(serde_json::to_string(&Status::Dismissed).unwrap(), "\"dismissed\"");
    }

    #[test]
    fn severity_serializes_pascalcase() {
        assert_eq!(serde_json::to_string(&Severity::Critical).unwrap(), "\"Critical\"");
        assert_eq!(serde_json::to_string(&Severity::High).unwrap(), "\"High\"");
        assert_eq!(serde_json::to_string(&Severity::Medium).unwrap(), "\"Medium\"");
        assert_eq!(serde_json::to_string(&Severity::Low).unwrap(), "\"Low\"");
    }

    #[test]
    fn row_parses_known_json() {
        // ARCHITECTURE.md §3 example row as JSON (field order = struct order).
        let json = r#"{
            "date": "2026-05-28",
            "advisory_id": "CVE-2026-9999",
            "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-9999",
            "package": "next@<15.5.17",
            "file_line": "src/middleware.ts:42",
            "severity": "High",
            "status": "open",
            "note": "-"
        }"#;
        let row: AdvisoryRow = serde_json::from_str(json).expect("parse known JSON");
        assert_eq!(row.date, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
        assert_eq!(row.severity, Severity::High);
        assert_eq!(row.status, Status::Open);
    }
}
```

**Lưu ý:**
- `NaiveDate::from_ymd_opt` is the post-deprecation API (chrono ≥ 0.4.20). KHÔNG dùng `from_ymd` (deprecated).
- Field names use snake_case (`advisory_id`, `source_url`, `file_line`) — serde keeps as-is (no `rename_all` on the struct). P005 dedup/append code reads/writes these names verbatim in JSON.
- `Eq` derive bắt buộc bên cạnh `PartialEq` để cho phép Vec/HashSet dedup downstream nếu cần.
- `note` is `String` (not `Option<String>`). ARCHITECTURE §3 uses `-` literal placeholder. Empty-string canonical = `"-"`.
- `file_line` is single `String` (not split). ARCHITECTURE §3 column literal là "File:Line". Worker parser sau (P006 append/render) sẽ format trực tiếp.

### Task 2: Create `src/state.rs` (StateFile + SCHEMA_VERSION)

**File:** `src/state.rs` (new file).

**Thêm:**
```rust
//! StateFile type — `.advisory-scan-state` JSON schema (ARCHITECTURE.md §2).
//!
//! Atomic write at runtime (P005 dedup updates, P007 migrate-state rewrites,
//! P008 state-backfill unions). Schema version locks the wire shape; bump
//! requires migrate-state path (Sub-mech C).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current state file schema version. Bump on breaking change (Sub-mech C).
///
/// P007 migrate-state compares stored `schema_version` against this constant
/// to decide migration path. P002 locks V1.
pub const SCHEMA_VERSION: u32 = 1;

/// On-disk shape of `.advisory-scan-state`.
///
/// `seen_advisories` is `Vec<String>` (not `BTreeSet`) for serde-stable order
/// (insertion order preserved → diff-friendly). Runtime dedup logic in P005
/// converts to `HashSet` in-memory then back to `Vec` before atomic write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateFile {
    /// Schema version of this file. Current = `SCHEMA_VERSION` (1).
    pub schema_version: u32,
    /// Timestamp of last scan, RFC 3339 UTC (e.g., `2026-05-28T09:51:35Z`).
    pub last_scan_at: DateTime<Utc>,
    /// Advisory IDs already processed (CVE-..., GHSA-..., etc.). Dedup source.
    pub seen_advisories: Vec<String>,
    /// Free-form version tag of the emitting agent (e.g., `advisory-watch@0.1.0`).
    pub agent_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> StateFile {
        StateFile {
            schema_version: SCHEMA_VERSION,
            last_scan_at: DateTime::parse_from_rfc3339("2026-05-28T09:51:35Z")
                .expect("valid RFC 3339")
                .with_timezone(&Utc),
            seen_advisories: vec![
                "CVE-2026-9256".to_string(),
                "GHSA-xxxx-yyyy".to_string(),
                "CVE-2026-27205".to_string(),
            ],
            agent_version: "advisory-watch@0.1.0".to_string(),
        }
    }

    #[test]
    fn state_roundtrip_json() {
        let state = sample_state();
        let json = serde_json::to_string(&state).expect("serialize");
        let back: StateFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back);
    }

    #[test]
    fn state_schema_version_constant_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
        assert_eq!(sample_state().schema_version, 1);
    }

    #[test]
    fn state_parses_known_json() {
        // ARCHITECTURE.md §2 example verbatim.
        let json = r#"{
            "schema_version": 1,
            "last_scan_at": "2026-05-28T09:51:35Z",
            "seen_advisories": [
                "CVE-2026-9256",
                "GHSA-xxxx-yyyy",
                "CVE-2026-27205"
            ],
            "agent_version": "advisory-watch@0.1.0"
        }"#;
        let state: StateFile = serde_json::from_str(json).expect("parse known JSON");
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.seen_advisories.len(), 3);
        assert_eq!(state.agent_version, "advisory-watch@0.1.0");
    }

    #[test]
    fn state_preserves_seen_advisories_order() {
        let state = sample_state();
        let json = serde_json::to_string(&state).expect("serialize");
        let back: StateFile = serde_json::from_str(&json).expect("deserialize");
        // Insertion order preserved (Vec semantics, not Set).
        assert_eq!(back.seen_advisories[0], "CVE-2026-9256");
        assert_eq!(back.seen_advisories[1], "GHSA-xxxx-yyyy");
        assert_eq!(back.seen_advisories[2], "CVE-2026-27205");
    }
}
```

**Lưu ý:**
- `DateTime::parse_from_rfc3339` returns `DateTime<FixedOffset>` → `.with_timezone(&Utc)` normalizes. Canonical pattern (context7-verified).
- `SCHEMA_VERSION: u32 = 1` is `pub const` (not `static`) — compile-time inline, no allocation, usable in `match` arms downstream.
- Field order in struct matches ARCHITECTURE §2 JSON example order exactly (schema_version → last_scan_at → seen_advisories → agent_version). Serde emits in this order.
- `Vec<String>` deliberate (not `BTreeSet`/`HashSet`) — see Anchor #11.

### Task 3: Register modules in `src/main.rs`

**File:** `src/main.rs`.

**Tìm:** dòng đầu file có `mod cli;` declaration (Worker grep `^mod cli;` to locate exact line — P001 Discovery confirms only `mod cli;` exists at module-decl level).

**Thay bằng / Thêm:**

Add 2 dòng ngay sau `mod cli;` (alphabetical: cli → row → state):
```rust
mod cli;
mod row;
mod state;
```

**Lưu ý:**
- Worker confirm exact insertion point — `mod cli;` may be at top of file OR after `use` statements. Insert `mod row;` + `mod state;` adjacent to it, preserve alphabetical order.
- KHÔNG thêm `use row::AdvisoryRow;` hay `use state::StateFile;` ở `main.rs` — không ai dùng yet. P004+ wire-in sẽ thêm `use` ở subcmd module riêng.
- KHÔNG đổi `Cli` struct, `Commands` enum, hay `fn main()` dispatch logic — types declared, không invoke.

### Task 4: Update `docs/ARCHITECTURE.md` §5 module status

**File:** `docs/ARCHITECTURE.md`.

**Tìm:** dòng (xấp xỉ 238) ghi:
```
**Scaffold status (2026-05-28, P001):** `main.rs` + `cli/` 8 stub files shipped. `state.rs`, `inbox.rs`, `row.rs`, `sentinel.rs`, `mcp/`, `error.rs` pending Phase 1+ phiếu (see BACKLOG.md).
```

**Thay bằng:**
```
**Scaffold status (2026-05-28):**
- P001: `main.rs` + `cli/` 8 stub files shipped.
- P002: `row.rs` (`AdvisoryRow` + `Status` + `Severity` enums) + `state.rs` (`StateFile` + `SCHEMA_VERSION = 1`) shipped — types only, not yet wired into subcmd logic.
- Pending Phase 1+ phiếu (see BACKLOG.md): `inbox.rs`, `sentinel.rs`, `mcp/`, `error.rs`.
```

**Lưu ý:**
- Mark P002 ship date when Worker commits (replace `2026-05-28` if commit date differs).
- KHÔNG touch §1 (CLI surface), §2 (state schema — already documents the JSON shape P002 implements), §3 (inbox format), §5 module layout tree (file list unchanged — still 16 expected files; just shipping 2 of them).

### Task 5: Update `docs/CHANGELOG.md`

**File:** `docs/CHANGELOG.md`.

**Thêm** entry mới ở top (per P001 Discovery note: docs-gate requires `## YYYY-MM-DD — <title>` heading, NOT `## Unreleased`):

```markdown
## 2026-MM-DD — P002 row/state types (serde)

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
```

**Lưu ý:**
- `YYYY-MM-DD` = commit date.
- Format must match P001 entry heading style (docs-gate validates).

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/row.rs` | NEW — `AdvisoryRow` + `Status` + `Severity` + 4 inline tests (Task 1) |
| `src/state.rs` | NEW — `StateFile` + `SCHEMA_VERSION` + 4 inline tests (Task 2) |
| `src/main.rs` | Add 2 lines: `mod row;` + `mod state;` after `mod cli;` (Task 3) |
| `docs/ARCHITECTURE.md` | §5 scaffold status: append P002 line (Task 4) |
| `docs/CHANGELOG.md` | New entry at top: P002 row/state types (Task 5) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `Cargo.toml` | No dep added. `git diff Cargo.toml` MUST be empty. Existing serde/serde_json/chrono with serde feature already cover P002. |
| `src/cli/*.rs` (all 8 stubs) | Untouched. `git diff src/cli/` MUST be empty. Subcmd wire-in is P003+. |
| `docs/PROJECT.md` | Phase status unchanged (still Phase 1 ongoing). |
| `docs/RULES.md`, `CLAUDE.md` | Doctrine untouched. |
| `docs/BACKLOG.md` | P002 line stays as-is (no strikethrough until ship complete). Sprint progress section may add `✅ P002` AFTER commit (Worker discretion per CLAUDE.md). |

---

## Luật chơi (Constraints)

1. **No new dependency.** `cargo add` is forbidden. Existing serde/serde_json/chrono cover everything. Hard Stop if tempted.
2. **No `unsafe { }` block.** Pure serde derive + struct — zero reason for `unsafe`. Escalate if ever tempted.
3. **No `tests/types_roundtrip.rs` external file.** Tests inline per ARCHITECTURE §8 + P001 Discovery convention. External integration tests reserved for subcmd wire-in (P004+).
4. **Field order matches ARCHITECTURE.** Both struct field order and JSON wire order must match ARCHITECTURE §2 (StateFile) and §3 (AdvisoryRow column order). serde default emits struct field order.
5. **Status lowercase, Severity PascalCase.** `#[serde(rename_all = "lowercase")]` on Status, `#[serde(rename_all = "PascalCase")]` on Severity — explicit, even when PascalCase matches Rust default (document intent + future-proof against rust-analyzer/clippy rename auto-fix).
6. **`seen_advisories: Vec<String>`.** Schema is Vec (not Set/BTreeSet). Internal dedup is P005's runtime concern, not schema's.
7. **`SCHEMA_VERSION = 1` locked.** Do NOT change in this phiếu. P007 migrate-state owns version bump path.
8. **No clippy `#[allow(...)]` escape hatches.** `cargo clippy --all-targets -- -D warnings` must pass clean. If clippy complains about a derive or rename — escalate Architect, do not silence.
9. **Match rustfmt edition 2024.** Per P001 Discovery: rustfmt may wrap multi-line on struct destructure / long expressions. Spec code above is illustrative — final `cargo fmt --check` output is canonical.
10. **No `dbg!`, `eprintln!`, `todo!()`, commented-out code** in final diff (Definition of Done #5).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings
- [ ] `cargo test --all` — ≥8 tests pass (4 in `row::tests` + 4 in `state::tests`)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff

### Manual Testing
- [ ] `cargo test row::tests::row_roundtrip_json` — pass
- [ ] `cargo test state::tests::state_roundtrip_json` — pass
- [ ] `cargo test state::tests::state_parses_known_json` — pass (ARCHITECTURE §2 example parses clean)
- [ ] `cargo test row::tests::row_parses_known_json` — pass (ARCHITECTURE §3 example parses clean)

### Regression
- [ ] `cargo run -- --help` — still shows 8 subcmd (P001 surface intact)
- [ ] `cargo run -- parse-report` — still prints TODO and exits 0 (no wire-in this phiếu)
- [ ] `git diff Cargo.toml` — empty (no dep added)
- [ ] `git diff src/cli/` — empty (no subcmd touched)

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P002 entry added with date-stamped heading per P001 Discovery format
- [ ] `docs/ARCHITECTURE.md` §5 — scaffold status updated to reflect P002 ship (row.rs + state.rs)
- [ ] `README.md` — no change required (CLI surface unchanged; quick-start still accurate)
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P002.md` — full report written (sai lệch nếu có, anchors verified, Sub-mech trace)
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended at top
- [ ] Sub-mechanism A-F Verification Trace filled in this phiếu (table above)
