# PHIẾU P001: Scaffold CLI surface (clap derive, 8 subcommand stubs)

> **ID format:** `P` + 3 chữ số. Counter `.phieu-counter` = 1, increment sau khi checkout branch.
> **Filename:** `docs/ticket/P001-scaffold-cli.md`
> **Branch:** `feat/P001-scaffold-cli`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (foundation of Phase 1 — blocks P002..P011)
> **Ảnh hưởng:** `src/main.rs` (new), `src/cli/mod.rs` (new), `docs/ARCHITECTURE.md` §5, `docs/CHANGELOG.md`, `README.md` quick-start
> **Dependency:** Không (first phiếu của Phase 1)
> **Lane:** Normal (scaffold per RULES.md §1 blacklist — KHÔNG Fast lane)
> **Sub-mech áp dụng:** B (capability — cargo check / cargo test), D (persistence — RULES.md §1 + ARCHITECTURE.md §1 grep)

---

## Context

### Vấn đề hiện tại

Repo `advisory-inbox` mới bootstrap. `Cargo.toml` đã khai báo dependencies (clap 4 derive, serde, tokio, rmcp, anyhow, thiserror, tempfile, regex, chrono) nhưng **chưa có file nào trong `src/`**. `cargo build` chắc chắn fail vì missing `src/main.rs`.

Phase 1 (P001-P006) cần CLI surface ổn định để các phiếu sau (P002 row/state types, P003 sentinel parser, P004 parse-report wire-in, ...) chỉ phải đụng vào logic body của từng subcmd — KHÔNG phải đụng `main.rs` clap shape nữa.

Reference BACKLOG.md item P001:
- Scope: `src/main.rs` clap derive parse, 8 subcmd registered (no logic, just parse + exit 0). Module skeleton `src/cli/mod.rs`.
- Acceptance: `advisory-inbox --help` shows 8 subcmd. `advisory-inbox parse-report` exits 0 with TODO message.

### Giải pháp

Tạo CLI surface theo đúng ARCHITECTURE.md §1 (8 subcommand) và §5 (module layout):

1. **`src/main.rs`** — entry point. `#[derive(Parser)]` struct `Cli { #[command(subcommand)] command: Commands }`, `#[derive(Subcommand)]` enum `Commands` với 8 variant, mỗi variant carry CLI flags theo §1 spec. `fn main() -> anyhow::Result<()>` dispatch vào handler stub trong `cli::*`.
2. **`src/cli/mod.rs`** — module registry, `pub mod` declare 8 module: `parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`, `serve`, `init`.
3. **8 stub file `src/cli/<name>.rs`** — mỗi file expose 1 `pub fn run(args ...) -> anyhow::Result<()>` printf TODO và return Ok(()). Args type khớp variant ở `Commands` enum.

Lý do tách 8 stub file (thay vì 1 monolithic):
- Phiếu sau (P004 parse-report, P005 dedup, P006 append, P007 migrate-state, P008 state-backfill, P009 scan-and-append, P010 serve) sẽ wire-in logic vào đúng file của subcmd đó → diff sạch.
- ARCHITECTURE.md §5 explicit list 8 file.

Lý do KHÔNG include `mcp/` module trong phiếu này:
- BACKLOG P010 (serve subcmd) là người ship `mcp/` module. P001 chỉ stub `cli/serve.rs` printf TODO.

### Scope

- CHỈ tạo: `src/main.rs`, `src/cli/mod.rs`, `src/cli/parse_report.rs`, `src/cli/dedup.rs`, `src/cli/append.rs`, `src/cli/migrate_state.rs`, `src/cli/state_backfill.rs`, `src/cli/scan_and_append.rs`, `src/cli/serve.rs`, `src/cli/init.rs`.
- CHỈ update docs: `docs/CHANGELOG.md` (entry P001), `docs/ARCHITECTURE.md` §5 (mark "scaffolded" status), `README.md` (quick-start `cargo run -- --help` example).
- KHÔNG sửa: `Cargo.toml` (deps đã đủ — verified), `docs/PROJECT.md` (status chưa đổi phase), `docs/RULES.md`, `CLAUDE.md`, `.advisory-scan-state` (chưa tồn tại trong repo này).
- KHÔNG tạo: `src/row.rs`, `src/state.rs`, `src/sentinel.rs`, `src/inbox.rs`, `src/error.rs`, `src/mcp/` (các phiếu sau).
- KHÔNG add dep mới. KHÔNG add `[[bin]]` section (Cargo auto-detects `src/main.rs`).

### Skills consulted

Architect ran `context7 query-docs /clap-rs/clap` for clap 4 derive `Subcommand` enum dispatch pattern. Result: confirmed `#[derive(Parser)]` + `#[derive(Subcommand)]` + `match cli.command { ... }` is canonical pattern. See Anchor #2.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Architect tool envelope = Read + Write + Glob + context7. KHÔNG có Bash/Grep. Mọi anchor đụng vào source code = `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `Cargo.toml` đã có `clap = { version = "4", features = ["derive"] }`, `anyhow = "1"`, `serde`, `serde_json`, `chrono`, `tokio`, `tempfile`, `regex`, `rmcp`, `thiserror`. KHÔNG cần add dep mới. | Architect đã Read `Cargo.toml` dòng 13-23 | `[verified]` | ✅ All 10 deps present |
| 2 | clap 4 derive canonical pattern là `#[derive(Parser)]` + `#[derive(Subcommand)]` enum + `Cli::parse()` + `match cli.command { ... }`. Subcommand variants có thể carry inline fields với `#[arg(...)]` attribute. | Architect đã query context7 `/clap-rs/clap` về "derive Subcommand enum dispatch pattern" | `[verified]` | ✅ Snippet xác nhận; sample git CLI 3-subcmd có Clone/Push/Add với mixed positional + flag args |
| 3 | `src/` directory chưa có file nào (greenfield). | Architect Glob `src/**/*.rs` — KHÔNG match (repo bootstrap state) | `[needs Worker verify]` | ✅ `src/main.rs` (45 bytes) duy nhất — `Hello, world!` bootstrap stub. Không phải file lạ; spawn-prompt context xác nhận stub này. Task 3 sẽ overwrite. |
| 4 | ARCHITECTURE.md §1 list 8 subcommand: `parse-report`, `dedup`, `append`, `migrate-state`, `state-backfill`, `scan-and-append`, `serve`, `init` (kebab-case CLI form). | Architect đã Read `docs/ARCHITECTURE.md` dòng 10-22 | `[verified]` | ✅ 8 subcmd đúng thứ tự. CLAUDE.md "File Structure" mention 6 subcmd cũ (parse_report/dedup/append/migrate_state/state_backfill/serve) thiếu `scan_and_append` + `init` — CLAUDE.md là tóm tắt, ARCHITECTURE.md là source of truth per RULES.md §11. |
| 5 | clap kebab-case rename: `Commands::ParseReport` enum variant renders thành CLI `parse-report` automatically (clap default rename rule). | context7 doc samples (Clone/Push/Add variants → `clone/push/add` CLI) | `[verified]` | ✅ Default rename = kebab-case. KHÔNG cần explicit `#[command(name = "parse-report")]`. |
| 6 | ARCHITECTURE.md §1 subcommand flag shapes: `parse-report [--input <FILE>]`, `dedup --state <FILE> --rows-json <FILE>`, `append --inbox <FILE> --rows-json <FILE>`, `migrate-state --state <FILE> [--dry-run]`, `state-backfill --state <FILE> --inbox <FILE> [--dry-run]`, `scan-and-append --report <STDIN_OR_FILE> --inbox <FILE> --state <FILE>`, `serve` (no flag), `init [--inbox-path <PATH>] [--state-path <PATH>]`. | Architect đã Read `docs/ARCHITECTURE.md` dòng 24-108 | `[verified]` | ✅ 8 flag signatures clear. P001 stub không validate flag content — chỉ parse + printf TODO. |
| 7 | Tokio runtime KHÔNG cần ở P001. `serve` subcmd stub chỉ printf TODO; thực tế tokio chỉ wire-in ở P010. `Cargo.toml` đã có `tokio` deps nhưng `main` không cần `#[tokio::main]`. | Architect đã Read `Cargo.toml` (tokio present) + ARCHITECTURE.md §1 Subcmd serve dòng 90-98 (rmcp stdio = future) | `[verified]` | ✅ `fn main() -> anyhow::Result<()>` sync OK. `#[tokio::main]` thêm ở P010. |
| 8 | `anyhow::Result<()>` return type cho `fn main` là idiomatic Rust 2024 + đã có anyhow dep. | Cargo.toml dòng 19 + Rust convention | `[verified]` | ✅ |
| 9 | Exit code: ARCHITECTURE.md §1 dòng 110-119 spec 0/1/2/3/5/64+. P001 stub LUÔN exit 0 (printf TODO, return Ok). Real exit-code mapping ở phiếu sau. | Architect đã Read ARCHITECTURE.md dòng 110-119 + BACKLOG.md P001 acceptance "exits 0 with TODO message" | `[verified]` | ✅ |
| 10 | `src/cli/mod.rs` chỉ chứa `pub mod ...;` declarations (8 dòng). Không có shared types/helpers ở P001 (kéo về module riêng khi cần — e.g. `src/error.rs` ở phiếu sau). | ARCHITECTURE.md §5 module layout | `[verified]` | ✅ Mod registry minimal. |

**Note: Anchor #3 là điểm duy nhất Worker phải verify trước EXECUTE.** Nếu `src/` không empty → Hard Stop, escalate Architect (có thể repo state khác giả định, cần phiếu refactor trước).

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Worker accepted V1 — no Tầng 1 objections.**

#### Anchor verification results

| # | Result | Note |
|---|--------|------|
| 1 | ✅ | `Cargo.toml`: clap 4 + derive feature ✅, anyhow ✅, serde/serde_json/chrono/tokio/tempfile/regex/rmcp/thiserror ✅. All 10 deps confirmed. |
| 2 | ✅ | Architect-verified via context7. No re-grep needed (not `[needs Worker verify]`). |
| 3 | ✅ | `find src -type f` → `src/main.rs` only (45 bytes, `Hello, world!` stub). Not a "file lạ" — spawn-prompt context explicitly declared this bootstrap stub. Task 3 overwrites it. |
| 4 | ✅ | ARCHITECTURE.md §1 lists exactly 8 subcmds in stated order. Confirmed by Worker Read. |
| 5 | ✅ | Architect-verified via context7. No re-grep needed. |
| 6 | ✅ | ARCHITECTURE.md §1 flag shapes confirmed by Worker Read (lines 27-108). All 8 signatures match phiếu Task 2/3 spec exactly. |
| 7 | ✅ | Cargo.toml confirms tokio present; sync `fn main()` correct for P001. |
| 8 | ✅ | anyhow dep present at line 19. Convention confirmed. |
| 9 | ✅ | Exit-code table at ARCHITECTURE.md §1 lines 110-119 confirmed. P001 stub always exits 0. |
| 10 | ✅ | ARCHITECTURE.md §5 module layout lists `cli/mod.rs` as registry only. |

#### Sub-mechanism Layer 2 checks (pre-CHALLENGE)

| Sub-mech | Command run | Result |
|----------|-------------|--------|
| A | N/A (no hook/cron) | N/A |
| B | `cargo check` — not run in CHALLENGE mode (EXECUTE only) | deferred to EXECUTE Task 0 |
| C | N/A (no schema migration) | N/A |
| D | `grep -l "parse-report\|parse_report" docs/ARCHITECTURE.md docs/BACKLOG.md docs/RULES.md` | ✅ ARCHITECTURE.md + BACKLOG.md matched. RULES.md = 0 hits (expected — RULES.md is process doctrine, not CLI spec). Stated expected "≥1 hit (ARCHITECTURE.md §1)" satisfied. |
| D | `grep -c "## §1. CLI Surface" docs/ARCHITECTURE.md` | ✅ = 1 |
| E | `cargo update --dry-run` | ✅ "Locking 0 packages" — no surprise bump. |
| F | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' .git/config` | ✅ 0 hits. No token leak. |

#### One mechanical observation (non-blocking)

`[mechanical]` — Phiếu Context §Vấn đề hiện tại says "chưa có file nào trong `src/`" but reality is `src/main.rs` exists as a bootstrap stub. This contradicts Anchor #3 original text ("directory chưa có file nào"). The file is an expected Hello World placeholder — not a scope conflict — and Task 3 intentionally overwrites it. **No Tầng 1 action needed.** Recommend Architect note this wording in future phiếu Context sections: "only Hello World bootstrap stub exists; Task 3 replaces it."

**Status: Ready for Chủ nhà approval gate.**

---

## Debug Log

> Worker emit observability records during EXECUTE.

*(empty — Worker fills during EXECUTE)*

---

## Verification Trace (Sub-mechanism A-F checks)

> Worker MUST run applicable Layer 2 capability checks BEFORE marking phiếu DONE.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | N/A — không có hook/cron/MCP ship ở P001 | N/A | N/A | N/A |
| B (capability) | `cargo check` | exit 0 | exit 0 | ✅ |
| B (capability) | `cargo build --release` | exit 0, binary tại `target/release/advisory-inbox` | exit 0, binary confirmed | ✅ |
| B (capability) | `cargo run -- --help` | exit 0, stdout chứa 8 subcmd name | exit 0, 8 subcmd in stdout | ✅ |
| B (capability) | `cargo run -- parse-report` (with empty stdin or no stdin) | exit 0, stdout/stderr chứa "TODO" | exit 0, "TODO: parse-report (input=None) — wired in P004" | ✅ |
| B (capability) | `cargo test --all` | tests pass (P001 may have 0 tests — acceptable for scaffold) | 0 tests, result: ok | ✅ |
| C (migration) | N/A — không có state schema change ở P001 | N/A | N/A | N/A |
| D (persistence) | `grep -l "parse-report\|parse_report" docs/ARCHITECTURE.md docs/BACKLOG.md docs/RULES.md` | ≥1 hit (ARCHITECTURE.md §1) | ARCHITECTURE.md + BACKLOG.md | ✅ |
| D (persistence) | `grep -c "## §1. CLI Surface" docs/ARCHITECTURE.md` | =1 (anchor preserved) | 1 | ✅ |
| E (env drift) | `cargo update --dry-run` | no surprise major bump | "Locking 0 packages" | ✅ |
| E (env drift) | `rm -rf target/ && cargo build --release` | exit 0 from clean target | exit 0 in 20.77s | ✅ |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' .git/config` | 0 hits | 0 hits | ✅ |

---

## Nhiệm vụ

### Task 0 — Pre-execute verification (Layer 2 per RULES.md §7)

**Trước khi viết code bất kỳ:**

1. Verify Anchor #3: chạy `ls src/ 2>/dev/null` hoặc `find src -type f` → expect empty hoặc directory không tồn tại. Nếu có file `.rs` lạ → **Hard Stop §12.1** (escalate, abandon phiếu, return to Architect).
2. Verify Anchor #1: `grep -c "^clap = " Cargo.toml` → expect 1. `grep -c "^anyhow = " Cargo.toml` → expect 1.
3. Verify clap derive feature flag: `grep 'features.*derive' Cargo.toml` → expect 1 hit on clap line.
4. Record results in Debug Log.

### Task 1 — Tạo `src/cli/mod.rs` (module registry)

**File:** `src/cli/mod.rs` (NEW)

**Tạo nội dung:**

```rust
//! CLI subcommand module registry.
//!
//! Each submodule exposes a `run` function called from `main.rs` after clap
//! dispatch. Logic bodies are stubs in P001; phiếu sau wire in real handlers
//! (P004 parse-report, P005 dedup, P006 append, etc.).

pub mod append;
pub mod dedup;
pub mod init;
pub mod migrate_state;
pub mod parse_report;
pub mod scan_and_append;
pub mod serve;
pub mod state_backfill;
```

**Lưu ý:**
- Thứ tự alphabetical để rustfmt happy.
- KHÔNG add shared types/helper ở P001 (e.g. tránh `pub struct CommonArgs` premature abstraction).

### Task 2 — Tạo 8 stub file `src/cli/<name>.rs`

**File:** `src/cli/parse_report.rs` (NEW)

```rust
//! Stub for `parse-report` subcommand. Real logic wired in P004.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(input: Option<PathBuf>) -> Result<()> {
    println!("TODO: parse-report (input={:?}) — wired in P004", input);
    Ok(())
}
```

**File:** `src/cli/dedup.rs` (NEW)

```rust
//! Stub for `dedup` subcommand. Real logic wired in P005.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!(
        "TODO: dedup (state={:?}, rows_json={:?}) — wired in P005",
        state, rows_json
    );
    Ok(())
}
```

**File:** `src/cli/append.rs` (NEW)

```rust
//! Stub for `append` subcommand. Real logic wired in P006.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(inbox: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!(
        "TODO: append (inbox={:?}, rows_json={:?}) — wired in P006",
        inbox, rows_json
    );
    Ok(())
}
```

**File:** `src/cli/migrate_state.rs` (NEW)

```rust
//! Stub for `migrate-state` subcommand. Real logic wired in P007.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, dry_run: bool) -> Result<()> {
    println!(
        "TODO: migrate-state (state={:?}, dry_run={}) — wired in P007",
        state, dry_run
    );
    Ok(())
}
```

**File:** `src/cli/state_backfill.rs` (NEW)

```rust
//! Stub for `state-backfill` subcommand. Real logic wired in P008.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, inbox: PathBuf, dry_run: bool) -> Result<()> {
    println!(
        "TODO: state-backfill (state={:?}, inbox={:?}, dry_run={}) — wired in P008",
        state, inbox, dry_run
    );
    Ok(())
}
```

**File:** `src/cli/scan_and_append.rs` (NEW)

```rust
//! Stub for `scan-and-append` composite subcommand. Real logic wired in P009.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(report: Option<PathBuf>, inbox: PathBuf, state: PathBuf) -> Result<()> {
    println!(
        "TODO: scan-and-append (report={:?}, inbox={:?}, state={:?}) — wired in P009",
        report, inbox, state
    );
    Ok(())
}
```

**File:** `src/cli/serve.rs` (NEW)

```rust
//! Stub for `serve` (MCP) subcommand. Real logic wired in P010.

use anyhow::Result;

pub fn run() -> Result<()> {
    println!("TODO: serve (MCP stdio JSON-RPC) — wired in P010");
    Ok(())
}
```

**File:** `src/cli/init.rs` (NEW)

```rust
//! Stub for `init` subcommand. Real logic wired in a Phase 1 follow-up phiếu.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(inbox_path: Option<PathBuf>, state_path: Option<PathBuf>) -> Result<()> {
    println!(
        "TODO: init (inbox_path={:?}, state_path={:?}) — wired in Phase 1 follow-up",
        inbox_path, state_path
    );
    Ok(())
}
```

**Lưu ý chung cho 8 stub:**
- KHÔNG dùng `todo!()` macro (panics — vi phạm BACKLOG acceptance "exits 0 with TODO message"). Dùng `println!` + `Ok(())`.
- KHÔNG `eprintln!` debug (Definition of Done §5).
- Mỗi handler signature khớp đúng flag shape ở Anchor #6.
- `Option<PathBuf>` cho flag optional (`--input`, `--inbox-path`, `--state-path`, `--report` lúc stdin-mode). `PathBuf` cho flag required.
- `dry_run: bool` cho `--dry-run` flag (default false).

### Task 3 — Tạo `src/main.rs` (entry point + clap derive)

**File:** `src/main.rs` (NEW)

```rust
//! advisory-inbox — CLI + MCP dual-mode binary.
//! Entry point: clap parse → dispatch to `cli::*` subcommand handler.
//!
//! See `docs/ARCHITECTURE.md` §1 for CLI surface, §5 for module layout.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;

#[derive(Parser)]
#[command(name = "advisory-inbox", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse sentinel block from agent report (stdin or --input <FILE>)
    ParseReport {
        /// Path to agent report markdown. Defaults to stdin.
        #[arg(long)]
        input: Option<PathBuf>,
    },
    /// Filter rows against state seen_advisories[]
    Dedup {
        /// Path to state JSON file
        #[arg(long)]
        state: PathBuf,
        /// Path to rows JSON (output of parse-report)
        #[arg(long = "rows-json")]
        rows_json: PathBuf,
    },
    /// Insert rows after `## Rows` heading in inbox markdown
    Append {
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Path to rows JSON
        #[arg(long = "rows-json")]
        rows_json: PathBuf,
    },
    /// Convert legacy single-line ISO state file to JSON schema
    MigrateState {
        /// Path to state file
        #[arg(long)]
        state: PathBuf,
        /// Detect + report but do not write
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Extract advisory IDs from inbox rows into state seen_advisories[]
    StateBackfill {
        /// Path to state JSON
        #[arg(long)]
        state: PathBuf,
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Compute + report but do not write
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
    /// Composite: parse → dedup → append + state update
    ScanAndAppend {
        /// Path to agent report. Omit for stdin.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Path to inbox markdown
        #[arg(long)]
        inbox: PathBuf,
        /// Path to state JSON
        #[arg(long)]
        state: PathBuf,
    },
    /// Start MCP server on stdin/stdout (JSON-RPC 2.0)
    Serve,
    /// Generate default config templates
    Init {
        /// Where to create the inbox markdown
        #[arg(long = "inbox-path")]
        inbox_path: Option<PathBuf>,
        /// Where to create the state JSON
        #[arg(long = "state-path")]
        state_path: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::ParseReport { input } => cli::parse_report::run(input),
        Commands::Dedup { state, rows_json } => cli::dedup::run(state, rows_json),
        Commands::Append { inbox, rows_json } => cli::append::run(inbox, rows_json),
        Commands::MigrateState { state, dry_run } => cli::migrate_state::run(state, dry_run),
        Commands::StateBackfill { state, inbox, dry_run } => {
            cli::state_backfill::run(state, inbox, dry_run)
        }
        Commands::ScanAndAppend { report, inbox, state } => {
            cli::scan_and_append::run(report, inbox, state)
        }
        Commands::Serve => cli::serve::run(),
        Commands::Init { inbox_path, state_path } => cli::init::run(inbox_path, state_path),
    }
}
```

**Lưu ý:**
- Clap default rename: enum variant `ParseReport` → CLI `parse-report` (kebab-case). KHÔNG cần `#[command(name = ...)]` override (Anchor #5).
- Flag rename `rows_json` → `--rows-json`, `dry_run` → `--dry-run`, `inbox_path` → `--inbox-path`, `state_path` → `--state-path`: explicit `#[arg(long = "...")]`. (Clap default tự kebab-case từ field name, nhưng explicit để diff sạch + grep-friendly.)
- `version` derive macro tự pull từ `Cargo.toml` version.
- `about` derive macro tự pull từ `Cargo.toml` description.
- `fn main() -> anyhow::Result<()>` sync — KHÔNG `#[tokio::main]` ở P001 (Anchor #7). P010 sẽ refactor `Commands::Serve` arm sang spawn tokio runtime cục bộ.
- KHÔNG mod khác (`mod row;`, `mod state;`, ...) ở P001 — chỉ `mod cli;`.

### Task 4 — Update `docs/CHANGELOG.md`

**File:** `docs/CHANGELOG.md`

**Tìm:** (file đã có hoặc chưa — Worker check; nếu chưa có, tạo header `# CHANGELOG — advisory-inbox` + section `## Unreleased`)

**Thêm entry (newest at top, dưới `## Unreleased`):**

```markdown
### P001 — Scaffold CLI surface (clap derive, 8 subcommand stubs)

- Added `src/main.rs` with clap 4 derive `Cli` + `Commands` enum (8 variants).
- Added `src/cli/` module skeleton with 8 stub handlers (`parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`, `serve`, `init`).
- Each stub prints `TODO: <subcmd> — wired in P<NNN>` and exits 0 per BACKLOG acceptance.
- No new dependency added. `Cargo.toml` unchanged.
- Lane: Normal. Sub-mech checks: B (cargo check + cargo build), D (ARCHITECTURE §1 + §5 grep preserved).

home: docs/CHANGELOG.md (operational), docs/ARCHITECTURE.md §5 (durable scaffold reference)
```

### Task 5 — Update `docs/ARCHITECTURE.md` §5

**File:** `docs/ARCHITECTURE.md`

**Tìm:** Section §5 Module Layout (dòng ~212-236).

**Thay đổi (additive, không xóa structure):**

- Phía dưới code block `src/` tree, thêm 1 dòng status note:

```markdown
**Scaffold status (2026-05-28, P001):** `main.rs` + `cli/` 8 stub files shipped. `state.rs`, `inbox.rs`, `row.rs`, `sentinel.rs`, `mcp/`, `error.rs` pending Phase 1+ phiếu (see BACKLOG.md).
```

**Lưu ý:** KHÔNG xóa các module pending — table giữ nguyên nhằm signal target architecture. Chỉ thêm status row.

### Task 6 — Update `README.md` (quick-start)

**File:** `README.md`

**Tìm:** Section `## Quick Start` (nếu tồn tại) hoặc tạo mới sau header.

**Thêm/thay:**

```markdown
## Quick Start (P001 scaffold)

```bash
# Build
cargo build --release

# Show all 8 subcommands
./target/release/advisory-inbox --help

# Each subcommand currently prints a TODO message (logic wired in P004-P011)
./target/release/advisory-inbox parse-report
./target/release/advisory-inbox dedup --state /tmp/s.json --rows-json /tmp/r.json
```

See `docs/ARCHITECTURE.md` §1 for full CLI surface and `docs/BACKLOG.md` for phiếu pipeline.
```

**Lưu ý:** Nếu `README.md` chưa tồn tại, tạo minimal version với (a) 1 dòng description copy từ `Cargo.toml`, (b) quick-start block trên, (c) link to ARCHITECTURE.md + BACKLOG.md.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/main.rs` | NEW — clap derive Cli + Commands enum (8 variants) + main dispatch (Task 3) |
| `src/cli/mod.rs` | NEW — module registry, 8 `pub mod` declarations (Task 1) |
| `src/cli/parse_report.rs` | NEW — stub `run(Option<PathBuf>)` (Task 2) |
| `src/cli/dedup.rs` | NEW — stub `run(PathBuf, PathBuf)` (Task 2) |
| `src/cli/append.rs` | NEW — stub `run(PathBuf, PathBuf)` (Task 2) |
| `src/cli/migrate_state.rs` | NEW — stub `run(PathBuf, bool)` (Task 2) |
| `src/cli/state_backfill.rs` | NEW — stub `run(PathBuf, PathBuf, bool)` (Task 2) |
| `src/cli/scan_and_append.rs` | NEW — stub `run(Option<PathBuf>, PathBuf, PathBuf)` (Task 2) |
| `src/cli/serve.rs` | NEW — stub `run()` (Task 2) |
| `src/cli/init.rs` | NEW — stub `run(Option<PathBuf>, Option<PathBuf>)` (Task 2) |
| `docs/CHANGELOG.md` | Add P001 entry (Task 4) |
| `docs/ARCHITECTURE.md` | Add §5 scaffold status note (Task 5) |
| `README.md` | Add/refresh quick-start (Task 6) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `Cargo.toml` | Verify (do NOT edit): clap 4 + anyhow + serde + others present. Anchor #1. |
| `docs/PROJECT.md` | Verify (do NOT edit): Phase 1 vẫn marked Bootstrap. Status flip sang "Phase 1 in progress" thuộc phiếu khác hoặc post-P001 housekeeping. |
| `docs/BACKLOG.md` | Verify (do NOT edit): P001 row vẫn ⬜. Worker chỉ flip sang ✅ sau merge, không trong phiếu này. |
| `docs/RULES.md`, `CLAUDE.md`, `.claude/agents/*.md` | KHÔNG động (durable doctrine). |
| `.advisory-scan-state`, `docs/security/advisory-inbox.md` | KHÔNG tạo (chưa thuộc scope P001). |

---

## Luật chơi (Constraints)

1. **KHÔNG add dependency mới.** `Cargo.toml` đã đủ — verified Anchor #1. Add dep = Hard Stop §12.2.
2. **KHÔNG dùng `todo!()` / `unimplemented!()`.** Panics vi phạm BACKLOG acceptance "exits 0 with TODO message". Dùng `println!("TODO: ...") + Ok(())`.
3. **KHÔNG dùng `unsafe { }`.** Hard Stop §12.7 + AUTO Tầng 1 docs gate (RULES.md §11) — scaffold không cần unsafe.
4. **KHÔNG add `[[bin]]` section** vào `Cargo.toml`. Cargo auto-detects `src/main.rs` → binary `advisory-inbox` (lấy từ `[package].name`).
5. **KHÔNG `#[tokio::main]` ở `main.rs`.** Sync `fn main()` OK (Anchor #7). P010 sẽ refactor.
6. **KHÔNG tạo `src/error.rs`, `src/state.rs`, `src/row.rs`, `src/sentinel.rs`, `src/inbox.rs`, `src/mcp/`.** Các phiếu sau ship.
7. **KHÔNG `eprintln!`/`dbg!`/commented-out code.** Definition of Done §5.
8. **CLI flag shape phải khớp ARCHITECTURE.md §1** (Anchor #6). Đổi flag = Hard Stop §12.3 (escalate Architect).
9. **8 subcmd name (kebab-case) phải khớp ARCHITECTURE.md §1** (Anchor #4). Đổi name = Hard Stop §12.3.
10. **Module file naming snake_case** (`migrate_state.rs`, `state_backfill.rs`, `scan_and_append.rs`, `parse_report.rs`) — match CLAUDE.md "Naming" + clap kebab-case CLI rendering convention.
11. **`docs-gate --all --verbose` MUST pass** trước commit (Definition of Done + RULES.md §14).
12. **Discovery Report mandatory** (`docs/discoveries/P001.md` + 1-line entry trong `docs/DISCOVERIES.md`).

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — exit 0, zero warnings, binary tại `target/release/advisory-inbox`
- [ ] `cargo test --all` — pass (0 tests acceptable for scaffold; nếu thêm test thì must pass)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — no diff
- [ ] `cargo check` — exit 0 (Sub-mech B)

### Manual Testing
- [ ] `./target/release/advisory-inbox --help` → stdout liệt kê đủ 8 subcmd name: `parse-report`, `dedup`, `append`, `migrate-state`, `state-backfill`, `scan-and-append`, `serve`, `init`.
- [ ] `./target/release/advisory-inbox parse-report` → exit 0, stdout chứa "TODO: parse-report".
- [ ] `./target/release/advisory-inbox parse-report --input /tmp/x.md` → exit 0, stdout chứa "TODO: parse-report (input=Some(...))".
- [ ] `./target/release/advisory-inbox dedup --state /tmp/s.json --rows-json /tmp/r.json` → exit 0, stdout chứa "TODO: dedup".
- [ ] `./target/release/advisory-inbox serve` → exit 0, stdout "TODO: serve (MCP stdio JSON-RPC) — wired in P010".
- [ ] `./target/release/advisory-inbox migrate-state --state /tmp/s.json --dry-run` → exit 0, stdout chứa "dry_run=true".
- [ ] `./target/release/advisory-inbox bogus-cmd` → exit ≠0, stderr clap "unrecognized subcommand" (clap default).
- [ ] `./target/release/advisory-inbox --version` → stdout matches `Cargo.toml` version (`0.1.0`).

### Regression
- [ ] N/A (P001 = first phiếu, no prior subcmd to regress).
- [ ] Verify `Cargo.toml` UNCHANGED via `git diff Cargo.toml` → empty.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P001 entry added at top of `## Unreleased`
- [ ] `docs/ARCHITECTURE.md` — §5 scaffold status note added (Tầng 1: module added)
- [ ] `README.md` — quick-start refreshed
- [ ] `docs-gate --all --verbose` — pass

### Discovery Report
- [ ] `docs/discoveries/P001.md` — full report written per RULES.md §13 schema
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top)
- [ ] Verification Trace table (Sub-mech A-F) filled — B + D + E rows completed; A/C/F marked N/A or 0-hit confirmed

### Lane + PR Body
- [ ] PR body includes `## Lane override` section per RULES.md §9 (original=normal, requested=N/A, reason=N/A)
- [ ] PR title `feat(P001): scaffold CLI surface`
- [ ] Branch `feat/P001-scaffold-cli`
- [ ] `.phieu-counter` incremented from 1 → 2 (post-checkout, before any code; rollback if checkout fail)
