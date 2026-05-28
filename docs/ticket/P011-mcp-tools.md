# PHIẾU P011: MCP tool dispatch — 6 tools registered via rmcp ToolRouter

> **ID format:** `P011` — counter `.phieu-counter` = 11 sau P010 ship.
> **Filename:** `docs/ticket/P011-mcp-tools.md`
> **Branch:** `feat/P011-mcp-tools`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (Phase 3 closer — completes MCP surface so `.mcp.json` integration + P013 tarot install can land; without P011 ship, MCP server has 0 tools = useless to clients)
> **Ảnh hưởng:** NEW `src/mcp/mod.rs` (module root), NEW `src/mcp/tools.rs` (`AdvisoryInboxService` struct + 6 `#[tool]` methods + `#[tool_router]` + `#[tool_handler]`), `src/main.rs` (add `mod mcp;` declaration), `src/cli/serve.rs` (replace P010 `AdvisoryInboxServer` unit struct usage with `AdvisoryInboxService::new()`; remove the inline `impl ServerHandler` block since `#[tool_handler]` macro generates it), `Cargo.toml` (POTENTIAL — only if rmcp `macros` + `schemars` features needed; see Architectural Decision #1 + Anchor #1/#2), POTENTIAL refactor to each `src/cli/<subcmd>.rs` (extract pure logic into `pub fn execute_*` helper IF needed for code reuse — see Architectural Decision #3), `tests/mcp_tools_cli.rs` (NEW — `tools/list` returns 6 + `tools/call` round-trip for at least `parse_report`), unit tests inline in `src/mcp/tools.rs`, `docs/ARCHITECTURE.md` §5 (P011 scaffold-status entry; new `src/mcp/` module ships) + §6 (flip MCP Surface status from "P011 planned" to "P011 shipped"; document `ToolRouter` + `enable_tools()`), `docs/CHANGELOG.md` (P011 entry — first MCP tool dispatch, first schemars use, first `#[tool_router]` macro), `README.md` (expand MCP quick-start with `tools/call` example — at least 1 of the 6 tools demonstrated)
> **Dependency:** P010 (handshake handler shipped, `cli/serve.rs` wires `ServerHandler` impl + tokio current_thread runtime). P002-P009 (all lib functions used by tools: `sentinel::extract_block`, `row::parse_row` + `AdvisoryRow` + `Status` + `Severity`, `state::read` + `write_atomic` + `StateFile`, `inbox::read_inbox` + `insert_rows` + `write_atomic` + `parse_rows`, `cli::migrate_state::{detect + parse_*}`, `cli::scan_and_append` composite logic). KHÔNG dependency vào P012 (release polish — happens after).
> **Lane:** **Guarded** (MCP server logic — RULES.md §1 explicit "MCP server logic (rmcp transport, tool dispatch)"; tool dispatch IS the canonical Guarded example; expanded surface vs P010 handshake-only)
> **Sub-mech áp dụng:** **A** (trigger — MCP `tools/list` returns 6 tool names + `tools/call <name>` dispatches to handler + returns shaped response; smoke = pipe `initialize` then `tools/list` then `tools/call parse_report` to stdin), **B** (capability — `cargo check`, `cargo test --test mcp_tools_cli` + `cargo test mcp::tools::tests`, `cargo build --release`; rmcp 1.7.0 `#[tool_router]` + `#[tool_handler]` + `#[tool]` macros + schemars feature verified via context7 by Architect — see Skills consulted), **D** (persistence — ARCHITECTURE §6 MCP Surface table is single source of truth for tool name/schema/output; CHANGELOG entry documents schemars dep if added; README MCP quick-start expanded). **C N/A** (no state schema change — tools READ existing schema). **E N/A** (no rust toolchain bump). **F** (token leak grep on new files — same as P010).

---

## Context

### Vấn đề hiện tại

P010 shipped MCP handshake (rmcp 1.7.0 stdio + `ServerHandler::get_info` returning name/version + empty `ServerCapabilities`). When a client (Claude Code session via `.mcp.json`) connects:

- `initialize` → returns valid `InitializeResult` with `serverInfo.name = "advisory-inbox"` ✅
- `tools/list` → returns empty list (default `ServerHandler::list_tools`) ❌ useless for clients
- `tools/call <any>` → returns "method not found" or similar (default) ❌ binary cannot be invoked over MCP

Phase 1 (P001-P006) + Phase 2 (P007-P009) shipped 6 CLI subcmds — all the actual work. P011 surfaces them over MCP so clients call structured tools instead of spawning the binary per-subcmd via shell.

Reference BACKLOG.md P011 (line 97-103):
- Lane: Guarded.
- Scope: `mcp/tools.rs` register 6 tools per ARCHITECTURE §6. Schema validate input. Reuse subcmd logic (no duplicate).
- Acceptance: Each tool callable via JSON-RPC, returns expected output. MCP error format on failure.
- Sub-mech checks: A, B, D.

Reference ARCHITECTURE.md §6 MCP Surface (line 273-282) — 6 tools table:

| Tool name | Description | Input schema | Output schema |
|-----------|-------------|--------------|---------------|
| `parse_report` | Parse sentinel block | `{ "report_text": "string" }` | `{ "rows": [...], "stack_scanned": {...}, "advisories_found": N }` |
| `dedup` | Filter against seen IDs | `{ "state_path": "string", "rows": [...] }` | `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` |
| `append` | Insert into inbox | `{ "inbox_path": "string", "rows": [...] }` | `{ "appended_count": N, "total_open": M }` |
| `migrate_state` | Legacy → JSON | `{ "state_path": "string", "dry_run": bool }` | `{ "from": "string", "to": "string", "seen_count": N }` |
| `state_backfill` | Extract IDs from inbox | `{ "state_path": "string", "inbox_path": "string" }` | `{ "backfilled_count": N }` |
| `scan_and_append` | Composite | `{ "report_text": "string", "inbox_path": "string", "state_path": "string" }` | `{ "appended": N, "skipped_dedup": M, "total_open": K }` |

Reference ARCHITECTURE.md §6 Error format (line 286-293):
```json
{
  "code": -32000,
  "message": "<msg>",
  "data": { "subcmd": "<name>", "exit_code": N }
}
```

### Giải pháp

**Architectural decisions (LOCKED by Architect):**

#### 1. **`Cargo.toml` MUST add 2 rmcp features: `macros` + `schemars` (PLUS schemars dep itself).**

Architect verified via context7:
- `#[tool]` + `#[tool_router]` + `#[tool_handler]` macros require the rmcp `macros` feature.
- Tool parameter types MUST derive `schemars::JsonSchema` for input-schema autogen.
- `schemars` is a separate crate; rmcp re-exports it (`rmcp::schemars` accessible when rmcp `schemars` feature enabled). Per rmcp docs: explicit `schemars = "1.0"` in `[dependencies]` plus rmcp `schemars` feature flag.

**Hard Stop normally prohibits new deps mid-scope** (CLAUDE.md §HARD STOPS #2). EXCEPTION: this phiếu's scope (per BACKLOG.md) explicitly is "register 6 tools per ARCHITECTURE §6 with schema validate input" — and `#[tool]` macro IS the path. Adding `schemars` is in-scope for "MCP tool dispatch (6 tools)".

**Resulting Cargo.toml diff:**

```toml
rmcp = { version = "1.7.0", features = ["server", "transport-io", "macros", "schemars"] }
schemars = "1.0"
```

Worker MUST verify (Task 0 Anchor #1 + #2) that:
- rmcp 1.7.0 actually publishes `macros` + `schemars` features (`cargo metadata --format-version 1 | jq '.packages[] | select(.name=="rmcp") | .features'`).
- `schemars = "1.0"` is the version rmcp 1.7.0 expects (rmcp docs snapshot showed `schemars = "1.0"`).

If `cargo check` after the diff fails with "feature not found" or version mismatch → STOP, escalate as design objection. Architect's context7 verification confirms 1.7.0 has these features but minor docs may lag.

**Alternative considered + rejected:** Hand-roll JSON Schema strings inline instead of `JsonSchema` derive. Rejected because: (a) ergonomics — manual schema for 6 tools is ~150 LOC of error-prone JSON, (b) drift risk — when input struct fields change, schema can diverge silently, (c) rmcp docs explicitly recommend the macro pattern.

#### 2. **NEW module `src/mcp/` shipped (deferred from P010).**

P010 deliberately left `src/mcp/` unshipped because handshake-only fit in `cli/serve.rs` (~80 lines). P011 ships tool dispatch:

- `src/mcp/mod.rs` — module root, `pub mod tools;`.
- `src/mcp/tools.rs` — `AdvisoryInboxService` struct + `#[tool_router]` block with 6 `#[tool]` methods + `#[tool_handler] impl ServerHandler for AdvisoryInboxService`.

**Worker MUST NOT keep `AdvisoryInboxServer` unit struct from P010.** The struct shifts from a name-version-only handshake holder to a tool-router-bearing service. Single struct, replaced.

**Rename rationale:** "Service" matches rmcp tool docs terminology; "Server" suggests transport-level. The rename is intentional to signal the surface expansion. Worker MAY keep `AdvisoryInboxServer` as a type alias `pub type AdvisoryInboxServer = AdvisoryInboxService;` if it preserves P010 integration test compat — Tầng 2 self-decide. Recommendation: just rename and update P010 test references.

**`src/main.rs` adds `mod mcp;` declaration.** (One-line diff, top-of-file `mod cli;` cluster.)

**`src/cli/serve.rs` changes:**
- Remove `pub struct AdvisoryInboxServer;` + `impl ServerHandler for AdvisoryInboxServer { fn get_info... }` (manual impl). The `#[tool_handler]` macro on `AdvisoryInboxService` auto-generates `get_info()` from `ServerCapabilities::builder().enable_tools().build()` + reads name/version from `Cargo.toml`.
- Import `AdvisoryInboxService` from `crate::mcp::tools::AdvisoryInboxService`.
- Replace `let server = AdvisoryInboxServer;` with `let server = AdvisoryInboxService::new();`.
- The `runtime.block_on(async { ... })` + `stdio()` + `.serve(transport)` + `.waiting()` pattern from P010 STAYS unchanged.

#### 3. **Refactor pattern for code reuse: extract pure logic from `cli/<subcmd>.rs` into `pub fn execute_*` helpers — Tầng 2 worker self-decide PER-subcmd.**

Currently each `cli/<subcmd>.rs` `run()` does: (a) read args, (b) read files, (c) compute, (d) print JSON to stdout. MCP tools need (b) + (c) only (return JSON `Value` instead of printing). To avoid duplicating ~50 LOC × 6 = 300 LOC of file-IO + logic, Worker has 2 reuse strategies — pick PER subcmd:

**Strategy A — Inline call (simple subcmds):**
```rust
// In mcp/tools.rs
#[tool(description = "...")]
fn parse_report(&self, Parameters(p): Parameters<ParseReportInput>) -> Result<Json<ParseReportOutput>, ErrorData> {
    let block = sentinel::extract_block(&p.report_text).map_err(|e| mcp_error("parse_report", 1, &e))?;
    let rows = block.lines().filter(...).map(row::parse_row).collect::<Result<Vec<_>, _>>().map_err(...)?;
    Ok(Json(ParseReportOutput { rows, advisories_found: rows.len(), ... }))
}
```
Suitable for: `parse_report`, `dedup`, `migrate_state`, `state_backfill` — pure-logic operations where MCP tool re-calls the lib functions directly. Bypasses `cli/` entirely.

**Strategy B — Extract helper (composite / IO-heavy subcmds):**
For `append` and `scan_and_append`, the existing `cli/<subcmd>.rs::run()` has more logic (multi-file IO, error mapping). Extract:
```rust
// In cli/scan_and_append.rs
pub fn execute(report_text: &str, inbox_path: &Path, state_path: &Path) -> Result<ScanAndAppendResult, ScanAndAppendError> { ... }

pub fn run(args: &ScanAndAppendArgs) -> Result<()> {
    let result = execute(&load_report(args)?, &args.inbox, &args.state)?;
    print_json(&result)?;
    Ok(())
}
```
Then `mcp::tools::scan_and_append` calls `cli::scan_and_append::execute(...)` directly and wraps result in `Json<_>`.

**Worker Tầng 2 self-decide PER subcmd which strategy is cleaner.** Constraints:
- Existing CLI tests in `tests/<subcmd>_cli.rs` MUST keep passing — `cli::<subcmd>::run` public signature MUST NOT change shape (still `fn run(args: &XArgs) -> Result<()>`).
- If Strategy B chosen, the extracted `execute` fn MUST be `pub` so `mcp::tools` can call it.
- Architect recommends: Strategy A for `parse_report` / `dedup` / `migrate_state` / `state_backfill` (pure-ish), Strategy B for `append` / `scan_and_append` (IO-heavy + error mapping has more shape).

**Hard constraint:** NO duplicate logic. If Worker copy-pastes ~10 lines between `cli/foo.rs` and `mcp/tools.rs::foo`, escalate as design objection — extract helper instead.

#### 4. **Input + output types: per-tool structs with `#[derive(Deserialize, Serialize, schemars::JsonSchema)]`.**

For each of the 6 tools, define in `mcp/tools.rs`:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ParseReportInput {
    /// Agent report markdown containing sentinel block.
    pub report_text: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ParseReportOutput {
    pub rows: Vec<AdvisoryRow>,
    pub stack_scanned: serde_json::Value,  // free-form per ARCHITECTURE §6 ("{...}")
    pub advisories_found: usize,
}
```

`AdvisoryRow` MUST already implement `Serialize` (P002 ship). If `AdvisoryRow` lacks `JsonSchema` derive, Worker adds it in `src/row.rs` — small additive change, NOT a Hard Stop (it's required for the new tool dispatch surface). Same applies to `Status` + `Severity` enums.

**Field doc comments (`///`)** translate into schema descriptions per `schemars` convention. Worker SHOULD add `///` on every input field — improves client UX (Claude Code displays them in tool picker UI).

**Output Value vs typed struct:** rmcp `#[tool]` method may return `Json<T>` (typed, schema-checked) OR `serde_json::Value` (untyped). Architect specifies typed `Json<T>` for all 6 tools — type-safe, schema-validated, doc-generated. Worker verify in Task 0 that `Json<T>` is in scope from `rmcp::handler::server::router::tool::Json` (per Skills consulted #1 snippet `Json<AddOutput>`).

#### 5. **Error mapping per ARCHITECTURE §6.**

Each tool returns `Result<Json<TOutput>, rmcp::model::ErrorData>`. On failure:

```rust
fn mcp_error(subcmd: &str, exit_code: i32, msg: &str) -> rmcp::model::ErrorData {
    rmcp::model::ErrorData::new(
        rmcp::model::ErrorCode(-32000),
        msg.to_string(),
        Some(serde_json::json!({ "subcmd": subcmd, "exit_code": exit_code })),
    )
}
```

**Worker Task 0 Anchor #5 verifies `ErrorCode` constructor shape:** rmcp 1.7.0 may have `ErrorCode(-32000)` tuple or `ErrorCode::from(-32000)` or a const `ErrorCode::INTERNAL_ERROR`. Architect context7 snapshot showed `pub fn new(code: ErrorCode, message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self` but didn't pin ErrorCode shape — Worker resolves via `cargo doc`.

**Exit code mapping** (matches ARCHITECTURE §1 line 110-119):
- 1 = input error (file missing, sentinel missing, format invalid)
- 2 = processing error (parse fail, write fail)
- 3 = concurrency error (future)
- 5 = MCP transport (process-level only; tool errors are NOT exit 5 — they're JSON-RPC errors)

Each tool maps its source error type → exit_code per existing CLI dispatch arm logic in `src/main.rs`. Worker reuses the same downcast pattern from `main.rs` if Strategy B helper returns a typed Error enum.

**Notably: tool errors do NOT crash the server.** Server keeps running. Only transport failures (P010 territory) exit the process. This means `data.exit_code` is informational (matches CLI exit-code semantics for client diagnostic parity), NOT actually causing process exit.

#### 6. **`#[tool_router]` + `#[tool_handler]` macro composition.**

Canonical rmcp 1.7.0 pattern (Skills consulted #1):

```rust
// src/mcp/tools.rs

use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{
        router::tool::{Parameters, Json},
        tool::ToolRouter,
    },
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AdvisoryInboxService {
    tool_router: ToolRouter<Self>,
}

impl AdvisoryInboxService {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }
}

impl Default for AdvisoryInboxService {
    fn default() -> Self { Self::new() }
}

#[tool_router]
impl AdvisoryInboxService {
    #[tool(name = "parse_report", description = "Parse sentinel block from agent report markdown into structured rows.")]
    fn parse_report(
        &self,
        Parameters(p): Parameters<ParseReportInput>,
    ) -> Result<Json<ParseReportOutput>, ErrorData> {
        // ... call sentinel::extract_block + row::parse_row loop ...
    }

    #[tool(name = "dedup", description = "Filter advisory rows against state seen_advisories[].")]
    fn dedup(
        &self,
        Parameters(p): Parameters<DedupInput>,
    ) -> Result<Json<DedupOutput>, ErrorData> {
        // ... call state::read + filter ...
    }

    // ... 4 more tools ...
}

#[tool_handler]
impl ServerHandler for AdvisoryInboxService {}
```

**Notes on the macro composition:**
- `#[tool_router]` on the `impl AdvisoryInboxService` block generates `Self::tool_router()` static-like fn that produces a `ToolRouter<Self>` containing all `#[tool]`-marked methods.
- `#[tool_handler]` on `impl ServerHandler for AdvisoryInboxService {}` (empty body) auto-generates `get_info()` returning `ServerInfo` with `enable_tools()` capability + name/version from `Cargo.toml` env vars + the registered tool list.
- The `tool_router: ToolRouter<Self>` field on the struct is REQUIRED (default `#[tool_handler]` uses `self.tool_router` per Skills consulted snippet "Custom router expression: `#[tool_handler(router = self.tool_router)]`" — but the default expression IS `self.tool_router`).
- `AdvisoryInboxService` implements `Clone` because `ToolRouter<Self>` requires it for the macro-generated impl. Worker verify in Task 0 — if compile fails on Clone, derive `Default` only.

**Important — manual `get_info()` override removed.** P010 had a manual `impl ServerHandler { fn get_info ... }`. With `#[tool_handler]`, the macro generates that. Worker MUST NOT keep the manual impl alongside the macro — that would conflict. If Worker wants to customize `description` text on `Implementation`, the macro supports an attribute like `#[tool_handler(server_info = ...)]` — verify via cargo doc OR keep the macro-generated default (rmcp uses Cargo.toml `description` field per package metadata; advisory-inbox Cargo.toml has a description already: line 5 "Advisory inbox state machine — parse agent report, dedup, append, migrate state. ...").

**Async vs sync `#[tool]` methods:** rmcp supports both. `parse_report` / `dedup` / `migrate_state` / `state_backfill` can be sync. `append` / `scan_and_append` MAY be sync (file IO is blocking in tokio current_thread runtime — `tokio::task::spawn_blocking` would be ideal but adds complexity). Architect locks: ALL 6 tools sync for P011. If performance requires async + spawn_blocking later, P012 polish phiếu handles. Sync tool methods = simpler code + matches existing CLI sync subcmd pattern.

#### 7. **`ServerCapabilities` flips to `enable_tools()`.**

P010 shipped empty capabilities (deliberate — no tools advertised). P011 advertises tools because they exist. The `#[tool_handler]` macro handles this automatically — Worker should NOT manually call `ServerCapabilities::builder().enable_tools().build()` anywhere; the macro generates it.

Verify post-implementation: integration test `tools/list` JSON-RPC response includes 6 tool descriptors with name + description + inputSchema.

#### 8. **Test coverage targets.**

**Unit tests (in `src/mcp/tools.rs` `#[cfg(test)] mod tests`):**
1. `parse_report_happy_path` — pass sentinel-bearing report_text, assert returned `rows.len() > 0` + `advisories_found` matches.
2. `parse_report_missing_sentinel` — pass report_text without `<!-- INBOX_APPEND_START -->`, assert `ErrorData { code: -32000, data.subcmd: "parse_report", data.exit_code: 1 }`.
3. `dedup_with_mock_state` — write temp state file with 1 seen ID, call dedup with 3 rows (1 matching), assert `kept.len() == 2 && skipped.len() == 1`. Use `tempfile::NamedTempFile` for state file.

**Integration tests (NEW `tests/mcp_tools_cli.rs`):**
4. `tools_list_returns_six_tools` — spawn binary in `serve` mode, send `initialize` + `tools/list`, assert response `result.tools` array has 6 entries with names `["parse_report", "dedup", "append", "migrate_state", "state_backfill", "scan_and_append"]` (order-insensitive).
5. `tools_call_parse_report_round_trip` — spawn binary, send `initialize` + `tools/call` with name=`parse_report` + arguments containing report_text fixture, assert response shape matches `ParseReportOutput`.
6. **OPTIONAL** `tools_call_parse_report_error_on_missing_sentinel` — same as #5 but malformed input, assert JSON-RPC error response (`error.code == -32000`, `error.data.subcmd == "parse_report"`). Tầng 2 self-decide.

**Total target: ≥3 unit + ≥2 integration = ≥5 new tests. Combined with P010 baseline (~64) → ≥69 tests post-P011.**

**KHÔNG full integration coverage all 6 tools.** Reasoning: schema + dispatch is uniform across tools (one macro pattern); integration smoke for 1 tool proves the full pipeline; per-tool logic correctness is unit-tested at the lib level (P002-P009). Adding 6 spawn tests = ~600 LOC test scaffolding for ~marginal coverage.

### Scope

- CHỈ tạo: `src/mcp/mod.rs`, `src/mcp/tools.rs`, `tests/mcp_tools_cli.rs`.
- CHỈ sửa: `Cargo.toml` (add `macros` + `schemars` rmcp features + `schemars = "1.0"` dep — see Decision #1), `src/main.rs` (add `mod mcp;` declaration only), `src/cli/serve.rs` (replace `AdvisoryInboxServer` unit struct + manual `impl ServerHandler` with `AdvisoryInboxService::new()` from mcp module — see Decision #2).
- POTENTIAL refactor (Tầng 2 self-decide PER subcmd — Decision #3): `src/cli/parse_report.rs` / `src/cli/dedup.rs` / `src/cli/append.rs` / `src/cli/migrate_state.rs` / `src/cli/state_backfill.rs` / `src/cli/scan_and_append.rs` extract `pub fn execute(...)` helper IF Worker picks Strategy B for that subcmd. CLI public `run()` signature unchanged.
- POTENTIAL additive: `src/row.rs` add `JsonSchema` derive to `AdvisoryRow` + `Status` + `Severity` (if not present from P002 — Worker Task 0 verify Anchor #7). Pure additive, no behavior change.
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (P011 scaffold-status entry — new `src/mcp/{mod.rs, tools.rs}` shipped) + §6 (flip status from "P011 planned" → "P011 shipped"; document tool macros + schemars dep). `docs/CHANGELOG.md` (P011 entry — first MCP tools, first schemars use). `README.md` (expand MCP section with `tools/call` example for at least 1 tool).
- KHÔNG sửa: `src/sentinel.rs`, `src/state.rs`, `src/inbox.rs` (libs are stable; tools call into them). `src/cli/mod.rs` (P001 registration unchanged). `src/cli/init.rs` (no MCP equivalent — init is local-side setup). `src/error.rs` (if it exists — Worker verify in Task 0; tools use rmcp's ErrorData, not a new project error type).
- KHÔNG tạo: `src/mcp/transport.rs` (ARCHITECTURE §5 lists it planned but P010 inlined stdio into `cli/serve.rs`; P011 doesn't need a separate transport file either — keep stdio wiring where P010 put it. Worker MAY revisit in P012 polish if `cli/serve.rs` gets unwieldy).
- KHÔNG bump rmcp version. Stays 1.7.0. Only adding feature flags.
- KHÔNG add `tokio` `rt-multi-thread` feature. Current_thread runtime from P010 stays.
- KHÔNG add `#[tokio::main]` to `src/main.rs`. P001-P010 sync-main contract preserved.
- KHÔNG change exit code semantics. Process-level exit 5 still ONLY on transport error (handshake or runtime). Tool errors are JSON-RPC -32000 responses, NOT process exits.
- KHÔNG change state schema / inbox format / sentinel marker / CLI exit codes / CLI subcmd shape. MCP is a parallel surface over the same lib.
- KHÔNG modify `.mcp.json` `mcpServers` to add `advisory-inbox` server entry (per P010 ship Scope: deferred to post-deploy step).
- KHÔNG implement tool that takes `state_path` AND `inbox_path` AS DEFAULTS (e.g., reading from a config file). Tools take explicit paths per argument per ARCHITECTURE §6 input schemas. Client (Claude Code session) is responsible for resolving paths.
- KHÔNG add concurrency lock (multi-process safety) — listed in ARCHITECTURE §10 future surface. P011 inherits P006/P007/P009 atomic-write protocols (INV-LOCAL-002).

### Skills consulted

**Architect (Bước 0) ran `mcp__context7__resolve-library-id` + `mcp__context7__query-docs` against `/websites/rs_rmcp_rmcp` (rmcp official docs, Source Reputation: High, Benchmark 68.5, 5221 snippets) for rmcp 1.7.0 tool dispatch API surface.**

Captured snapshots (Architect-verified, NOT speculation):

1. **`#[tool_router]` + `#[tool]` + `#[tool_handler]` canonical pattern:**
   ```rust
   struct Server;

   #[derive(Deserialize, schemars::JsonSchema, Default)]
   struct AddParameter { left: usize, right: usize }
   #[derive(Serialize, schemars::JsonSchema)]
   struct AddOutput { sum: usize }

   #[tool_router(server_handler)]
   impl Server {
       #[tool(name = "adder", description = "Modular add two integers")]
       fn add(
           &self,
           Parameters(AddParameter { left, right }): Parameters<AddParameter>
       ) -> Json<AddOutput> {
           Json(AddOutput { sum: left.wrapping_add(right) })
       }
   }
   ```
   Source: `https://docs.rs/rmcp/latest/rmcp/handler/server/router/tool/index.html`. Note `server_handler` argument variant — Worker verify via cargo doc whether this is needed for our setup OR plain `#[tool_router]` suffices.

2. **`#[tool_handler]` minimal example with `ToolRouter`:**
   ```rust
   struct TimeServer;

   #[tool_router]
   impl TimeServer {
       #[tool(description = "Get current time")]
       async fn get_time(&self) -> String { "12:00".into() }
   }

   #[tool_handler]
   impl ServerHandler for TimeServer {}
   ```
   Source: `https://docs.rs/rmcp/latest/rmcp/attr.tool_handler.html`. Confirms empty `impl ServerHandler` body — macro fills everything. Macro auto-generates `get_info()` with tools capability enabled and reads server name/version from `Cargo.toml`.

3. **Custom router visibility/name (combining routers):**
   ```rust
   #[tool_router(router = tool_router_a, vis = "pub")]
   impl MyToolHandler { ... }
   ```
   Not needed for P011 (single struct, single router). Snapshot retained for completeness — could matter if future phiếu splits tools into 2 modules.

4. **Custom router expression on `#[tool_handler]`:**
   ```rust
   #[tool_handler(router = self.tool_router)]
   impl ServerHandler for MyToolHandler { ... }
   ```
   Confirms default expression is `self.tool_router` (the field on the struct). Our struct has `tool_router: ToolRouter<Self>` field per Architectural Decision #6.

5. **`ToolRoute` struct shape:**
   ```rust
   #[non_exhaustive]
   pub struct ToolRoute<S> {
       pub call: Arc<DynCallToolHandler<S>>,
       pub attr: Tool,
   }
   ```
   Internal — Worker likely doesn't touch directly; macro builds these. Retained for ground-truth reference.

6. **`ToolRouter::call()`:**
   ```rust
   pub async fn call(
       &self,
       context: ToolCallContext<'_, S>,
   ) -> Result<CallToolResult, ErrorData>
   ```
   Confirms `Result<CallToolResult, ErrorData>` as the dispatch return signature. Tool methods return `Result<Json<T>, ErrorData>` and macro wraps `Json<T>` → `CallToolResult`.

7. **`ErrorData::new` constructor:**
   ```rust
   pub fn new(
       code: ErrorCode,
       message: impl Into<Cow<'static, str>>,
       data: Option<Value>,
   ) -> Self
   ```
   Confirms 3-arg constructor. Worker verify `ErrorCode` shape — likely a `pub struct ErrorCode(pub i32)` or similar. Architect didn't pin exact shape — Anchor #5.

8. **`ErrorData` struct fields:**
   ```rust
   pub struct Error {  // type alias to ErrorData
       pub code: ErrorCode,
       pub message: Cow<'static, str>,
       pub data: Option<Value>,
   }
   ```
   `data: Option<Value>` accepts `serde_json::json!({ "subcmd": "...", "exit_code": N })`. Matches ARCHITECTURE §6 error format.

9. **rmcp feature flags:**
   ```
   - server: server functionality and tool system
   - client: client functionality
   - macros: #[tool] and #[prompt] macros
   - schemars: JSON Schema generation for tool definitions
   - auth: OAuth 2.0
   - elicitation: elicitation support
   ```
   Source: `https://docs.rs/rmcp/latest/rmcp/index.html`. **Confirms `macros` + `schemars` are separate features.** Cargo.toml MUST add both.

10. **schemars version in rmcp ecosystem:**
    ```toml
    rmcp = { version = "0.3", features = ["elicitation"] }
    serde = { version = "1.0", features = ["derive"] }
    schemars = "1.0"
    ```
    Source: `https://docs.rs/rmcp/latest/rmcp/service/struct.Peer.html`. **Confirms `schemars = "1.0"`.** Architect notes the rmcp docs snippet uses `rmcp 0.3` for elicitation — but feature flag and schemars version pairing is independent. Worker verify via `cargo metadata` Anchor #2.

11. **rmcp re-exports:** `schemars`, `serde_json`, `ErrorData`, `ServerHandler`, `Json`, `Peer`, `Service`, `ServiceExt`, `RoleServer`, `serve_server`. So `rmcp::schemars::JsonSchema` MAY be accessible without separate `schemars` crate in scope — Worker verify; if works, drop explicit `schemars` in `[dependencies]` (rmcp transitively brings it under the `schemars` feature). If NOT, explicit dep needed. Tầng 2 self-decide based on compile result.

**Architect read `docs/ticket/P010-mcp-serve.md`** — confirmed P010 ship state: handshake-only, `AdvisoryInboxServer` unit struct + manual `impl ServerHandler { get_info ... }`, `ServerCapabilities::builder().build()` empty. P011 supersedes that struct + handler.

**Architect read `docs/ARCHITECTURE.md`** — §5 module layout listed `src/mcp/{mod.rs, tools.rs, transport.rs}` planned; P010 deferred all three. P011 ships `mod.rs` + `tools.rs` only (transport stays inline in `cli/serve.rs` per Decision #2 follow-up note). §6 MCP Surface table is source of truth for tool name/schema mapping (6 rows).

**Architect read `docs/BACKLOG.md` P011** (line 97-103) — Lane=Guarded, Tầng=1, Sub-mech A+B+D confirmed.

**Architect read `Cargo.toml`** — line 23 `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`. NO `macros`. NO `schemars`. NO explicit `schemars` dep. Confirms Decision #1's Cargo.toml diff is needed.

**Architect read `docs/RULES.md`** §1 Guarded scope — "MCP server logic (rmcp transport, tool dispatch)" is explicit Guarded territory. Confirms Lane=Guarded.

**Architect did NOT verify:**
- `Implementation` field set on `Cargo.toml` description carry-through via `#[tool_handler]` macro. Decision #6 mentions it tentatively — Worker can verify post-implementation by reading `tools/list` `serverInfo` response.
- Exact import path of `ToolRouter` + `Parameters` + `Json` in rmcp 1.7.0. Skills consulted gave `rmcp::handler::server::router::tool::ToolRouter` per ToolRoute snippet — Worker Task 0 confirms via cargo doc.
- Whether `Clone` derive is REQUIRED on `AdvisoryInboxService`. Architect's draft includes `Clone` defensively per Skills consulted snippet pattern; if compile fails without it, the macro will surface a clear error.
- Whether `description` field exists on `Implementation` in rmcp 1.7.0 — P010 phiếu also flagged this as uncertain. Macro-generated `get_info()` reads from `Cargo.toml` so doesn't matter for P011.

---

## Task 0 — Verification Anchors

> **REQUIRED** — Architect must grep/verify real code before writing assumptions.
> Worker reads this table to know which assumptions were verified vs. unverified.
> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `Cargo.toml` currently has `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` — NO `macros`, NO `schemars` features. NO `schemars` dep. P011 MUST add `macros` + `schemars` features + `schemars = "1.0"` dep. | Architect Read `Cargo.toml` line 23 during context load. | `[verified]` | ✅ Line 23 confirmed. Diff scoped: +2 feature strings on rmcp line, +1 new dep line `schemars = "1.0"`. |
| 2 | rmcp 1.7.0 published feature flags include `macros` + `schemars` (NOT typos, NOT renamed). `schemars` crate version `1.0` is what rmcp 1.7.0 expects. | Worker `cargo metadata --format-version 1 \| jq '.packages[] \| select(.name=="rmcp") \| .features'` (lists all rmcp features). Then `cargo add --dry-run schemars@1.0` (no resolver error). | `[needs Worker verify]` | ⏳ TO VERIFY. Architect context7 docs confirmed feature names exist (Skills consulted #9) and rmcp doc snippet shows `schemars = "1.0"` (Skills consulted #10), but pinning to 1.7.0 specifically requires cargo metadata. **Hard Stop if `macros` feature missing or schemars 1.x incompat** → escalate design objection. |
| 3 | rmcp re-exports `schemars` under `rmcp::schemars` when `schemars` feature enabled. Worker MAY use `rmcp::schemars::JsonSchema` instead of adding explicit `schemars` dep. | Worker post-Cargo.toml edit: `cargo check` with derive `rmcp::schemars::JsonSchema` first; if works, drop explicit `schemars = "1.0"` from Cargo.toml. | `[unverified]` | ⏳ TO VERIFY. Skills consulted #11 confirms re-export exists; Architect didn't test the macro derives accept the re-exported path. Tầng 2 self-decide — both paths work, prefer minimal-dep pattern. |
| 4 | `#[tool_router]` + `#[tool]` + `#[tool_handler]` macros exist + compose per Skills consulted #1 + #2 patterns. Tool method signature `fn(&self, Parameters<TInput>) -> Result<Json<TOutput>, ErrorData>` accepted. | Architect context7 verified at `https://docs.rs/rmcp/latest/rmcp/attr.tool_router.html` + `https://docs.rs/rmcp/latest/rmcp/attr.tool_handler.html`. | `[verified]` | ✅ via context7. Worker compile-verifies at EXECUTE. |
| 5 | `rmcp::model::ErrorData::new(code: ErrorCode, message: ..., data: Option<Value>)` constructor exists. `ErrorCode` accepts `-32000` (custom server error per JSON-RPC 2.0 reserved range). | Architect context7 #7 + #8 confirmed `ErrorData::new` signature; `ErrorCode` exact construction shape NOT pinned. | `[needs Worker verify]` | ⏳ TO VERIFY. Worker `cargo doc --no-deps --package rmcp` + open `target/doc/rmcp/model/struct.ErrorCode.html` (or grep `rg "pub struct ErrorCode" $(cargo metadata --format-version 1 \| jq -r '.packages[] \| select(.name=="rmcp") \| .manifest_path' \| xargs dirname)/src/`). Try `ErrorCode(-32000)` (tuple), `ErrorCode::from(-32000)`, or `ErrorCode::INTERNAL_ERROR`. Tầng 2 self-fix. |
| 6 | `ToolRouter<Self>` type exists in `rmcp::handler::server::router::tool::ToolRouter` OR `rmcp::handler::server::tool::ToolRouter`. Generic `<S>` parameter for the host service struct. | Architect Skills consulted #5 referenced `ToolRoute<S>` at `rmcp::handler::server::router::tool::ToolRoute` — `ToolRouter` is the parent. | `[unverified]` | ⏳ TO VERIFY exact import path. Worker `cargo doc` → search "ToolRouter". Likely `rmcp::handler::server::router::tool::ToolRouter` per Skills consulted #5's `ToolRoute` neighborhood. |
| 7 | `AdvisoryRow` (P002) + `Status` + `Severity` enums already derive `Serialize` + `Deserialize`. May NOT derive `JsonSchema` (P002 didn't have schemars dep). P011 adds `JsonSchema` derive — additive, no behavior change. | Architect P002 phiếu reading + ARCHITECTURE §5 P002 entry confirms `Serialize` + `Deserialize` + `FromStr` (P004) + `Display` (P006) on these types. `JsonSchema` not mentioned → likely absent. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "JsonSchema\|schemars" src/row.rs`). If absent (expected), Worker adds `#[derive(schemars::JsonSchema)]` (or `rmcp::schemars::JsonSchema` per Anchor #3) to `AdvisoryRow` + `Status` + `Severity`. |
| 8 | `src/mcp/` directory does NOT yet exist (P010 deferred per Architectural Decision #1 of P010). | Architect knows from P010 Decision #1; current source tree has no `src/mcp/`. | `[needs Worker verify]` | ⏳ TO VERIFY (`ls src/`). Expected: no `mcp/` dir. Worker creates `src/mcp/mod.rs` + `src/mcp/tools.rs` (NEW). |
| 9 | `src/main.rs` declares `mod cli;` + lib root modules (`mod row; mod state; mod sentinel; mod inbox; mod error?`) at top. P011 adds `mod mcp;` to this cluster. | Architect did NOT Read full `src/main.rs` in this Bước 0 (already verified `Commands::Serve => cli::serve::run()` shape via P010 Anchor #2 outcome). | `[needs Worker verify]` | ⏳ TO VERIFY (`head -20 src/main.rs`). Confirm existing `mod` declarations, add `mod mcp;` alongside. |
| 10 | `src/cli/serve.rs` currently has (per P010 ship): `pub struct AdvisoryInboxServer;` (unit struct, `#[derive(Debug, Clone, Default)]`) + `impl ServerHandler for AdvisoryInboxServer { fn get_info(&self) -> ServerInfo { ... } }` + `pub fn run() -> Result<()>` with tokio current_thread block_on. P011 REMOVES struct + manual handler impl; KEEPS runtime + transport + serve/waiting pipeline; replaces `let server = AdvisoryInboxServer;` with `let server = AdvisoryInboxService::new();`. | Architect Read P010 phiếu code blocks (lines 644-692 of P010 phiếu file). | `[unverified]` | ⏳ TO VERIFY (`cat src/cli/serve.rs`). If Worker found post-P010 surface drifted (e.g., struct renamed in a hotfix), adjust. Hard Stop if surface bears no resemblance to P010 shape. |
| 11 | Existing CLI subcmd run signatures: `pub fn run(args: &XArgs) -> Result<()>` per P004-P009 pattern. Tests in `tests/<subcmd>_cli.rs` rely on `cargo_bin("advisory-inbox").arg("...")` not on `run()` directly. Strategy B extraction of `pub fn execute(...)` is non-breaking. | Architect read ARCHITECTURE §5 scaffold entries P004-P009. | `[needs Worker verify]` | ⏳ TO VERIFY PER subcmd Worker chooses Strategy B for (`grep -n "pub fn run\|pub fn execute" src/cli/<subcmd>.rs`). |
| 12 | Existing `src/cli/scan_and_append.rs::run()` does file IO + composes sentinel/row/state/inbox per P009 ship. Extracting `pub fn execute(report_text: &str, inbox_path: &Path, state_path: &Path) -> Result<ScanAndAppendResult, ...>` is feasible. | Architect read ARCHITECTURE §5 P009 entry — composite ships in `cli/scan_and_append.rs` with 5-family error map. | `[needs Worker verify]` | ⏳ TO VERIFY (`wc -l src/cli/scan_and_append.rs` + scan structure). If extraction surface bigger than ~80 LOC, Worker may opt Strategy A (inline call to lib fns from `mcp/tools.rs::scan_and_append` directly, duplicating composition). Tầng 2 self-decide. |
| 13 | `Parameters<T>` extractor + `Json<T>` wrapper available in scope `rmcp::handler::server::router::tool::{Parameters, Json}`. | Architect Skills consulted #1 snippet shows both used; exact import path inferred from `ToolRoute` neighborhood (Skills consulted #5). | `[unverified]` | ⏳ TO VERIFY at compile. Likely path; if differs, `cargo doc` resolves. |
| 14 | `tools/list` JSON-RPC method works out-of-box once `#[tool_handler]` macro applied — no manual override needed. Server returns full tool descriptors with name/description/inputSchema from macro-derived data. | Architect Skills consulted #2 + #6 — `ToolRouter::call` is the dispatch primitive; `#[tool_handler]` macro auto-wires `list_tools` + `call_tool` handlers. | `[unverified]` | ⏳ TO VERIFY via integration Test 4 (`tools_list_returns_six_tools`). |
| 15 | ARCHITECTURE §6 (line 264-265) currently says "P011 (planned): 6 tools registered via ToolRouter. ServerCapabilities flips .enable_tools(). src/mcp/ module introduced." P011 flips this to "shipped" + adds detail. | Architect Read ARCHITECTURE §6 during context load (lines 260-282). | `[verified]` | ✅ Line 264-265 confirmed. Worker edits during Docs Gate. |
| 16 | ARCHITECTURE §5 line 256 currently ends scaffold-status list with "P010 ship details". P011 entry appended after. | Architect Read ARCHITECTURE §5 during context load. | `[verified]` | ✅ Line 256 confirmed. |
| 17 | `tests/` greenfield for `mcp_tools_cli.rs` (NEW file). `tests/serve_cli.rs` exists from P010 — P011 may need to ADJUST it if `AdvisoryInboxServer` rename to `AdvisoryInboxService` breaks the in-file reference (Anchor #18). | Architect knows from P010 ship. | `[needs Worker verify]` | ⏳ TO VERIFY (`ls tests/`). Expected: `serve_cli.rs` present, `mcp_tools_cli.rs` absent. |
| 18 | `tests/serve_cli.rs` (P010) integration test asserts `"name":"advisory-inbox"` in JSON-RPC initialize response. Does NOT reference `AdvisoryInboxServer` Rust identifier directly (it's a spawn-binary test, not a struct test). Survives rename. P010 unit test in `src/cli/serve.rs` `#[cfg(test)]` references `AdvisoryInboxServer` — Worker EITHER deletes that unit test (covered by `AdvisoryInboxService` tests in P011's new `mcp/tools.rs`) OR adjusts the import. | Architect Read P010 phiếu test patterns (lines 698-718 of P010). | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "AdvisoryInboxServer\|AdvisoryInboxService" tests/ src/cli/serve.rs`). If found, Worker adjusts. Tầng 2 — small mechanical fix. |
| 19 | Sub-mech F: token leak grep on new files MUST be clean. `src/mcp/mod.rs` + `src/mcp/tools.rs` + `tests/mcp_tools_cli.rs` + Cargo.toml diff. | Worker grep post-EXECUTE. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/mcp/ tests/mcp_tools_cli.rs Cargo.toml`). |
| 20 | Sub-mech A (trigger): MCP `tools/list` returns 6 tool entries; `tools/call parse_report` round-trips a response. Smoke = pipe sequence to `cargo run -- serve`, observe stdout. | Worker via Test 4 + Test 5 in Task 4 integration suite. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE. |
| 21 | `enable_tools()` chain on `ServerCapabilities::builder()` exists and is automatically called by `#[tool_handler]` macro. P010 used `.build()` (no enable). P011 macro generates with `.enable_tools().build()`. | Architect Skills consulted #2 doc says "macro automatically generates `get_info()` with tools capability enabled". | `[verified]` | ✅ via context7. |
| 22 | rmcp 1.7.0 `Cargo.toml`-driven `Implementation { name, version }` macro fill works correctly (server name = `"advisory-inbox"`, version = `"0.1.0"` per Cargo.toml line 2 + 3). | Architect Skills consulted #2: "reads the server name/version from `Cargo.toml`". | `[unverified]` | ⏳ TO VERIFY via Test 4 — assert `serverInfo.name == "advisory-inbox"` in `initialize` response (same assertion as P010 Test B). |
| 23 | Per-tool input struct field `serde_json::Value` for free-form data (e.g., `ParseReportOutput.stack_scanned`) requires `JsonSchema` impl on `Value`. `schemars` crate provides this via `serde_json` integration feature. | Architect did NOT explicitly verify; `schemars 1.0` typically supports `serde_json::Value` natively. If not, Worker substitutes with `BTreeMap<String, String>` or similar concrete type. | `[needs Worker verify]` | ⏳ TO VERIFY at compile. If `JsonSchema` not auto-derived for `Value`, Worker either: (a) enable `schemars` feature `preserve_order` + `serde_json` integration, (b) replace `Value` with a typed struct, (c) fall back to `String` field. Tầng 2 self-decide. |
| 24 | `tokio::task::spawn_blocking` NOT used in P011 tool methods. All 6 tools sync (Architectural Decision #6 final paragraph). Acceptable because stdio is serial single-client. | Architect locked in Decision #6. | `[verified]` | ✅ Decision locked. Worker MUST NOT add `spawn_blocking`. |
| 25 | `cargo build --release` final binary size remains < 10 MB after schemars + macros + 6 tool methods added. P010 baseline (~1.96 MB per ARCHITECTURE §5 P010 entry line 255). schemars + macros adds estimated 1-3 MB; total expected ~3-5 MB. | Architect estimate. | `[unverified]` | ⏳ TO VERIFY at Verification Trace `ls -la target/release/advisory-inbox`. If exceeds 10 MB unexpectedly, Worker notes in Discovery Report — investigation deferred to P012 (NOT a Hard Stop). |

**Hard Stop triggers:**
- Anchor #2 — `macros` or `schemars` feature missing from rmcp 1.7.0 published features → STOP, escalate as design objection (the entire spec hinges on these macros).
- Anchor #5 — `ErrorData::new` constructor not present OR `ErrorCode` cannot accept `-32000` → STOP, escalate.
- Anchor #6 — `ToolRouter` type does not exist OR generic parameter shape differs significantly from Architect's draft → STOP, escalate as shape objection.
- Anchor #10 — `src/cli/serve.rs` surface post-P010 does NOT match P010 ship description (struct renamed by hotfix, etc.) → STOP, escalate.
- Anchor #23 — `serde_json::Value` cannot be used as a field type in `JsonSchema`-deriving struct after multiple workarounds → STOP, may require replacing `stack_scanned` semantics; escalate as shape objection.

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. Anchors marked `[needs Worker verify]` / `[unverified]` resolve at EXECUTE; the Tầng 2 self-decide options are documented so Worker can proceed without round-tripping for trivial mechanical adjustments.

### Pre-phiếu snapshot (Worker auto first-step)

```bash
# Run from project root (worktree root for phiếu workflow):
PHIEU_ID=$(basename "$(git rev-parse --show-toplevel)" | grep -oE 'P[0-9]+')
mkdir -p ".backup/${PHIEU_ID}"
cp .claude/settings.local.json ".backup/${PHIEU_ID}/" 2>/dev/null || true
[ -d .sos-state ] && cp -r .sos-state ".backup/${PHIEU_ID}/" 2>/dev/null || true
git rev-parse HEAD > ".backup/${PHIEU_ID}/main-head.txt"
echo "✓ Snapshot at .backup/${PHIEU_ID}/ — auto-cleaned on phieu-done"
```

If the phiếu hits ❌ mid-execute and you need to roll back: `cp .backup/${PHIEU_ID}/settings.local.json .claude/` and `git reset --hard $(cat .backup/${PHIEU_ID}/main-head.txt)`. `.backup/` is gitignored.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Anchor verification (Task 0 results):**

- Anchor #1: ✅ `Cargo.toml` line 23 confirmed — `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`, no `macros`/`schemars`/`schemars` dep.
- Anchor #2: ✅ `cargo metadata` confirms rmcp 1.7.0 has `macros: ["dep:rmcp-macros", "dep:pastey"]` and `schemars: ["dep:schemars"]`. `cargo add --dry-run schemars@1.0` resolves clean. rmcp `Cargo.toml` line 558-560 confirms `schemars = "1.0"`.
- Anchor #3: ✅ `rmcp/src/lib.rs` line 37-38: `#[cfg(any(feature = "server", feature = "schemars"))] pub use schemars;` — re-export confirmed. `rmcp::schemars::JsonSchema` usable without explicit dep, but phiếu adds explicit dep anyway (correct).
- Anchor #4: ✅ `#[tool_router]`, `#[tool]`, `#[tool_handler]` confirmed via rmcp-macros-1.7.0/src/lib.rs + tool_router.rs.
- Anchor #5: ✅ `ErrorData::new(code: ErrorCode, message: impl Into<Cow<'static, str>>, data: Option<Value>)` confirmed at model.rs line 533. `ErrorCode(pub i32)` tuple struct confirmed at model.rs line 500 — `ErrorCode(-32000)` valid.
- Anchor #6: ✅ `ToolRouter<S>` confirmed at `handler/server/router/tool.rs` line 302.
- Anchor #7: ✅ `src/row.rs` — no `JsonSchema` derive on `AdvisoryRow`, `Status`, `Severity`. Worker adds.
- Anchor #8: ✅ `src/mcp/` does not exist — greenfield confirmed.
- Anchor #9: ✅ `src/main.rs` top mods: `mod cli; mod inbox; mod row; mod sentinel; mod state;`. Worker adds `mod mcp;`.
- Anchor #10: ✅ `src/cli/serve.rs` matches P010 description exactly — `AdvisoryInboxServer` unit struct + manual `impl ServerHandler` + `run()` with current_thread runtime.
- Anchor #11: ✅ All subcmd `pub fn run(...)` signatures confirmed. No existing `pub fn execute(...)` helpers.
- Anchor #12: ✅ `scan_and_append.rs` is 104 lines, well-structured — Strategy B extraction feasible for both `append` and `scan_and_append`.
- Anchor #13: ⚠️ Import path discrepancy — see [O1.1] below.
- Anchor #17: ✅ `tests/` has `mcp_tools_cli.rs` absent, `serve_cli.rs` present.
- Anchor #18: ✅ `AdvisoryInboxServer` only referenced in `src/cli/serve.rs` (lines 23, 25, 43, 65, 74) — NOT in `tests/serve_cli.rs`. Rename is non-breaking for integration tests.
- Anchor #19: ⏳ Token leak grep — deferred to post-EXECUTE (no new files yet).
- Anchor #20: ⏳ Sub-mech A trigger smoke — deferred to post-EXECUTE.

**Objections (Tầng 1 only):**

None. All [mechanical] findings below are Tầng 2 self-decidable.

**[mechanical] O1.1 — Import path for `Parameters` + `Json` in phiếu draft differs from rmcp 1.7.0 actuals**

Phiếu Decision #6 + Task 3 draft code specifies:
```rust
use rmcp::handler::server::router::tool::{Json, Parameters};
```

Reality (`rmcp-1.7.0/src/handler/server/router/tool.rs` doc comment line 9):
```rust
use rmcp::handler::server::{wrapper::{Parameters, Json}, tool::ToolRouter};
```
And `rmcp::lib.rs` line 18: `pub use handler::server::wrapper::Json;` — so `rmcp::Json` also works at top level.

`Parameters` is at `rmcp::handler::server::wrapper::Parameters` (confirmed from `wrapper/parameters.rs` + `wrapper.rs` re-exports). `Json` at `rmcp::handler::server::wrapper::Json` (re-exported from top level as `rmcp::Json`).

**Worker self-fix (Tầng 2):** Use actual paths from rmcp source. Correct import:
```rust
use rmcp::{
    ErrorData, Json, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::ErrorCode,
    tool, tool_handler, tool_router,
};
```
Or equivalently using the full path for clarity. Both compile. No Tầng 1 escalation needed — pure mechanical import correction.

**[mechanical] O1.2 — `tool_router: ToolRouter<Self>` struct field NOT required**

Phiếu Decision #6 states "The `tool_router: ToolRouter<Self>` field on the struct is REQUIRED (default `#[tool_handler]` uses `self.tool_router`)".

Reality: `rmcp-macros-1.7.0/src/tool_router.rs` line 93 shows the macro generates `#[tool_handler(router = Self::#router())]` — calling the static `Self::tool_router()` fn, NOT a `self.tool_router` field. The struct field is NOT required.

**Worker self-fix (Tầng 2):** Use the canonical `#[tool_router(server_handler)]` single-macro approach (which generates both the router static fn + `impl ServerHandler`), removing the struct field. Simpler: no struct field, no `Clone` requirement, no `Default` implementation needed for P011. Alternatively keep the two-step pattern with the field. Either way no Tầng 1 impact — purely mechanical struct layout choice.

**Worker accepted V1 — no Tầng 1 objections. Two mechanical self-fixable findings ([O1.1] import paths, [O1.2] struct field not required). Ready for Chủ nhà approval gate.**

### Turn 1 — Architect Response
*(Architect fills this when invoked in RESPOND mode. Cannot read source code — relies on Worker's `file:line` citations.)*

- [O1.1] → ACCEPT (mechanical, Worker self-fix)
- [O1.2] → ACCEPT (mechanical, Worker self-fix)

**Status:** ✅ ACCEPTED — proceed to EXECUTE

### Final consensus
- Phiếu version: V1 (no Architect changes needed — two mechanical corrections Worker-resolved)
- Total turns: 1
- Approved by Chủ nhà: 2026-05-28 — code execution may begin

---

## Verification Trace (Sub-mechanism A-F checks)

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| A (trigger) | Spawn `cargo run --quiet -- serve`, pipe `initialize` + `tools/list`, read stdout | exit 0 on stdin close, stdout contains all 6 tool names in `result.tools[].name` | | |
| A (trigger) | Same spawn, pipe `initialize` + `tools/call` with `name="parse_report"` + arguments | stdout contains JSON-RPC response with `result.content` matching `ParseReportOutput` shape | | |
| B (capability) | `cargo check` after Cargo.toml diff + new module + macros applied | exit 0, 0 warnings | | |
| B (capability) | `cargo test --test mcp_tools_cli` | ≥2 integration tests pass | | |
| B (capability) | `cargo test mcp::tools::tests` | ≥3 unit tests pass | | |
| B (capability) | `cargo test --test serve_cli` (P010 regression) | passes after Worker adjusts `AdvisoryInboxServer` → `AdvisoryInboxService` rename per Anchor #18 | | |
| B (capability) | `cargo test --all` | ≥69 tests pass (P010 baseline ~64 + ≥5 new) | | |
| B (capability) | `cargo build --release` | exit 0, 0 warnings, binary at `target/release/advisory-inbox` | | |
| B (capability) | `cargo clippy --all-targets -- -D warnings` | clean | | |
| B (capability) | `cargo fmt --check` | no diff | | |
| B (capability) | `target/release/advisory-inbox --help` shows `serve` subcmd (unchanged from P010) | row contains "Start MCP server" desc | | |
| C (state migration) | (no state schema change) | N/A | N/A | N/A |
| D (persistence) | `grep -n "P011" docs/CHANGELOG.md` | ≥1 hit (entry at top) | | |
| D (persistence) | `grep -n "P011\|tool_router\|schemars" docs/ARCHITECTURE.md` | ≥1 hit each in §5 + §6 | | |
| D (persistence) | `grep -n -i "tools/call\|parse_report" README.md` | ≥1 hit (MCP tool example in quick-start) | | |
| D (persistence) | `grep -l "P011" docs/discoveries/` | 1 hit (`P011.md` discovery file) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (rmcp + tokio + new schemars unchanged after first lock) | | |
| E (env drift) | `cargo build --release` from clean `target/` | exit 0 | | |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/mcp/ src/cli/serve.rs tests/mcp_tools_cli.rs Cargo.toml` | 0 hits | | |
| F (runtime state) | binary release size `ls -la target/release/advisory-inbox \| awk '{print $5}'` | < 10 MB (P010 baseline 1.96 MB + schemars/macros delta) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech A + B + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code; verify rmcp 1.7.0 macros + schemars features actually exist before changing Cargo.toml.

**Lệnh chạy (verify Anchors #2, #5, #6, #8, #9, #10, #11, #17, #18):**

```bash
# Anchor #2 — rmcp features published in 1.7.0 + schemars 1.0 resolves
cargo metadata --format-version 1 \
  | jq '.packages[] | select(.name=="rmcp") | .features' \
  | tee /tmp/p011-rmcp-features.json
# Expect output contains: "macros": [...], "schemars": [...]

cargo add --dry-run schemars@1.0 2>&1 | head -20
# Expect: no resolver error; shows "Adding schemars v1.x.x"

# Anchor #5 + #6 + #13 — rmcp 1.7.0 API surface
cargo doc --no-deps --package rmcp 2>&1 | tail -5
# Browse target/doc/rmcp/handler/server/router/tool/ for ToolRouter, Parameters, Json
# Browse target/doc/rmcp/model/struct.ErrorCode.html (or similar) for ErrorCode shape
# OR: `find ~/.cargo/registry/src -name "*.rs" -path "*rmcp-1.7.0*" | xargs grep -l "pub struct ErrorCode\|pub struct ToolRouter" 2>/dev/null | head -5`

# Anchor #8 — src/mcp/ greenfield
ls src/mcp/ 2>&1 || echo "src/mcp/ does not exist ✅ — Worker creates"

# Anchor #9 — main.rs current mod declarations
head -20 src/main.rs

# Anchor #10 — cli/serve.rs P010 ship shape
cat src/cli/serve.rs
# Expect: AdvisoryInboxServer unit struct + impl ServerHandler with manual get_info + run() with current_thread tokio runtime

# Anchor #11 + #12 — existing cli/<subcmd>.rs structure for refactor planning
wc -l src/cli/*.rs
grep -n "pub fn run\|pub fn execute" src/cli/*.rs

# Anchor #17 + #18 — tests/ inventory + serve_cli.rs reference to AdvisoryInboxServer
ls tests/
grep -n "AdvisoryInboxServer\|AdvisoryInboxService" tests/ src/cli/serve.rs

# Anchor #7 — AdvisoryRow derives
grep -n "derive\|JsonSchema\|schemars" src/row.rs

# Sub-mech F preflight (Anchor #19)
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml && echo "FAIL — token leak" || echo "clean ✅"

# Baseline test count (post-P010)
cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l
# Expect ~64 (P010 added 2-3 over P009 baseline 62). Phiếu target: ≥69 after P011 (+≥3 unit + ≥2 integration).
```

**Output:** Worker fill vào Debate Log Turn 1 Anchor table with `file:line` evidence + ✅/⚠️/❌.

**Hard Stop triggers:**
- Anchor #2 — rmcp 1.7.0 does not publish `macros` OR `schemars` feature → STOP, escalate design objection. Spec hinges on these macros.
- Anchor #2 — `schemars = "1.0"` does not resolve (resolver conflict) → STOP, evaluate if `schemars = "0.8"` or other version is what rmcp 1.7.0 expects; escalate as shape objection if version pin differs.
- Anchor #5 — `ErrorData::new` constructor missing in rmcp 1.7.0 → STOP, escalate.
- Anchor #6 — `ToolRouter` type not present → STOP, escalate (the macros wouldn't work without it).
- Anchor #10 — `src/cli/serve.rs` does NOT have `AdvisoryInboxServer` unit struct OR runtime pipeline differs from P010 ship → STOP, escalate (something rewrote P010 unexpectedly).
- Anchor #8 — `src/mcp/` directory unexpectedly EXISTS pre-EXECUTE → STOP, investigate (P010 ship explicitly deferred).

---

### Task 1: `Cargo.toml` — add rmcp `macros` + `schemars` features + `schemars` dep

**File:** `Cargo.toml`

**Tìm** (current line 23 per Anchor #1):

```toml
rmcp = { version = "1.7.0", features = ["server", "transport-io"] }
```

**Thay bằng:**

```toml
rmcp = { version = "1.7.0", features = ["server", "transport-io", "macros", "schemars"] }
schemars = "1.0"
```

**Lưu ý:**
- Add the `schemars = "1.0"` line directly after the rmcp line (or in alphabetical order in `[dependencies]` block — Worker self-decide; Tầng 2 mechanical).
- Worker MAY drop the explicit `schemars = "1.0"` line if Anchor #3 verifies that `rmcp::schemars::JsonSchema` re-export works for the derive macro. Tầng 2 self-decide.
- DO NOT bump rmcp version. DO NOT touch tokio features. DO NOT touch dev-dependencies.
- After this edit, run `cargo check` immediately. If new compile errors surface BEFORE any other file is touched, that's a Cargo.toml-only verification — useful to isolate dep issues. Then proceed to Task 2.
- DO NOT add `wait_timeout` or any other dev-dep. Integration tests use the same stdin-EOF pattern from P010.

---

### Task 2: NEW `src/mcp/mod.rs` — module root

**File:** `src/mcp/mod.rs` (NEW — Anchor #8 confirms `src/mcp/` greenfield).

**Tạo:**

```rust
//! MCP (Model Context Protocol) surface for advisory-inbox.
//!
//! P011 ships tool dispatch: 6 tools registered via rmcp `#[tool_router]` macros.
//! Transport (stdio JSON-RPC 2.0) wiring stays in `cli/serve.rs` per P010 layout.
//!
//! Tools exposed (see [`tools::AdvisoryInboxService`]):
//! - `parse_report` — parse sentinel block into structured rows
//! - `dedup` — filter rows against state seen_advisories
//! - `append` — insert rows into inbox markdown
//! - `migrate_state` — legacy → JSON schema state file
//! - `state_backfill` — extract IDs from inbox into state seen_advisories
//! - `scan_and_append` — composite of parse + dedup + append + state update

pub mod tools;
```

**Lưu ý:**
- Module root is intentionally minimal — just re-exports `tools` submodule. P012+ may add `transport` or other submodules; not in P011 scope.
- DO NOT `pub use tools::*` — keep explicit `crate::mcp::tools::AdvisoryInboxService` paths. Caller (`cli/serve.rs`) does the import.

---

### Task 3: NEW `src/mcp/tools.rs` — `AdvisoryInboxService` + 6 `#[tool]` methods

**File:** `src/mcp/tools.rs` (NEW).

**Tạo** (Architect-drafted; Worker tunes per Task 0 Anchor results — especially #5, #6, #13, #23):

```rust
//! MCP tool dispatch — 6 tools per ARCHITECTURE §6.
//!
//! Each tool delegates to existing lib code (sentinel, row, state, inbox, cli/<subcmd>).
//! No duplicate logic — tools wrap the same fns the CLI subcmds call.
//!
//! Error mapping: any tool failure surfaces as JSON-RPC `ErrorData { code: -32000,
//! message: <error display>, data: { subcmd: <name>, exit_code: <N> } }` matching
//! ARCHITECTURE §6 + ARCHITECTURE §1 exit-code conventions.

use std::path::PathBuf;

use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{
        router::tool::{Json, Parameters},
        tool::ToolRouter,
    },
    model::ErrorCode,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::row::AdvisoryRow;
use crate::sentinel;
use crate::state;
use crate::inbox;

// ──────────────────────────────────────────────────────────────
// Service struct + constructor
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdvisoryInboxService {
    tool_router: ToolRouter<Self>,
}

impl AdvisoryInboxService {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }
}

impl Default for AdvisoryInboxService {
    fn default() -> Self { Self::new() }
}

// ──────────────────────────────────────────────────────────────
// Input + output types (one pair per tool)
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseReportInput {
    /// Agent report markdown containing `<!-- INBOX_APPEND_START -->` sentinel block.
    pub report_text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ParseReportOutput {
    pub rows: Vec<AdvisoryRow>,
    /// Stack-scanned summary (currently free-form per ARCHITECTURE §6). May be empty object.
    pub stack_scanned: serde_json::Value,
    pub advisories_found: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DedupInput {
    /// Absolute path to state JSON file (will be read).
    pub state_path: String,
    /// Rows to filter against state.seen_advisories[].
    pub rows: Vec<AdvisoryRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DedupOutput {
    pub kept: Vec<AdvisoryRow>,
    pub skipped: Vec<AdvisoryRow>,
    /// All IDs observed in input (kept ∪ skipped); for state update.
    pub observed_ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendInput {
    /// Absolute path to inbox markdown file (will be read + atomically rewritten).
    pub inbox_path: String,
    /// Rows to insert after `## Rows` heading, newest at top.
    pub rows: Vec<AdvisoryRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendOutput {
    pub appended_count: usize,
    pub total_open: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MigrateStateInput {
    /// Path to state file (any format: JSON v1 / legacy ISO-8601 / missing).
    pub state_path: String,
    /// If true, detect + parse but do NOT write the migrated file.
    pub dry_run: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MigrateStateOutput {
    /// Source format detected: "json-v1" / "legacy-iso" / "missing".
    pub from: String,
    /// Destination format: always "json-v1" for P011.
    pub to: String,
    pub seen_count: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StateBackfillInput {
    /// Path to state JSON v1 file (will be atomically rewritten).
    pub state_path: String,
    /// Path to inbox markdown to extract `processed`/`dismissed` row IDs from.
    pub inbox_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StateBackfillOutput {
    pub backfilled_count: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanAndAppendInput {
    pub report_text: String,
    pub inbox_path: String,
    pub state_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScanAndAppendOutput {
    pub appended: usize,
    pub skipped_dedup: usize,
    pub total_open: usize,
}

// ──────────────────────────────────────────────────────────────
// Error helper
// ──────────────────────────────────────────────────────────────

/// Build MCP error per ARCHITECTURE §6 error format.
/// `subcmd` = tool name for client diagnostics; `exit_code` mirrors CLI exit code conventions.
fn mcp_error(subcmd: &str, exit_code: i32, msg: impl ToString) -> ErrorData {
    // Worker Task 0 Anchor #5: ErrorCode construction shape — try ErrorCode(-32000) tuple first;
    // if that doesn't compile, use the rmcp-exported method (e.g., ErrorCode::INTERNAL_ERROR or ErrorCode::from(-32000)).
    ErrorData::new(
        ErrorCode(-32000),
        msg.to_string(),
        Some(json!({ "subcmd": subcmd, "exit_code": exit_code })),
    )
}

// ──────────────────────────────────────────────────────────────
// Tool router — 6 tools
// ──────────────────────────────────────────────────────────────

#[tool_router]
impl AdvisoryInboxService {
    #[tool(
        name = "parse_report",
        description = "Parse advisory sentinel block from agent report markdown into structured rows."
    )]
    fn parse_report(
        &self,
        Parameters(p): Parameters<ParseReportInput>,
    ) -> Result<Json<ParseReportOutput>, ErrorData> {
        let block = sentinel::extract_block(&p.report_text)
            .map_err(|e| mcp_error("parse_report", 1, e))?;
        let rows: Vec<AdvisoryRow> = block
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with("<!--"))
            .map(crate::row::parse_row)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| mcp_error("parse_report", 2, e))?;
        let advisories_found = rows.len();
        Ok(Json(ParseReportOutput {
            rows,
            stack_scanned: json!({}),  // free-form placeholder; Worker may extract real values
            advisories_found,
        }))
    }

    #[tool(
        name = "dedup",
        description = "Filter advisory rows against state.seen_advisories[]; returns kept + skipped + observed_ids."
    )]
    fn dedup(
        &self,
        Parameters(p): Parameters<DedupInput>,
    ) -> Result<Json<DedupOutput>, ErrorData> {
        let state = state::read(&PathBuf::from(&p.state_path))
            .map_err(|e| mcp_error("dedup", 1, e))?;
        let seen: std::collections::BTreeSet<String> =
            state.seen_advisories.iter().cloned().collect();
        let observed_ids: Vec<String> = p.rows.iter().map(|r| r.advisory_id.clone()).collect();
        let (kept, skipped): (Vec<AdvisoryRow>, Vec<AdvisoryRow>) = p
            .rows
            .into_iter()
            .partition(|r| !seen.contains(&r.advisory_id));
        Ok(Json(DedupOutput { kept, skipped, observed_ids }))
    }

    #[tool(
        name = "append",
        description = "Insert advisory rows into inbox markdown after `## Rows` heading, newest at top, atomic write."
    )]
    fn append(
        &self,
        Parameters(p): Parameters<AppendInput>,
    ) -> Result<Json<AppendOutput>, ErrorData> {
        let inbox_path = PathBuf::from(&p.inbox_path);
        let mut content = inbox::read_inbox(&inbox_path)
            .map_err(|e| mcp_error("append", 1, e))?;
        let appended_count = p.rows.len();
        inbox::insert_rows(&mut content, &p.rows)
            .map_err(|e| mcp_error("append", 2, e))?;
        inbox::write_atomic(&inbox_path, &content)
            .map_err(|e| mcp_error("append", 2, e))?;
        let total_open = inbox::parse_rows(&content)
            .map(|rows| rows.iter().filter(|r| r.status_is_open()).count())
            .unwrap_or(0);
        Ok(Json(AppendOutput { appended_count, total_open }))
    }

    #[tool(
        name = "migrate_state",
        description = "Detect state file format (JSON / legacy ISO / missing) and convert to JSON v1 schema."
    )]
    fn migrate_state(
        &self,
        Parameters(p): Parameters<MigrateStateInput>,
    ) -> Result<Json<MigrateStateOutput>, ErrorData> {
        // Strategy B candidate: extract pure fn from `cli/migrate_state.rs::run` if not already shipped.
        // Worker Task 0 Anchor #11/#12: check if `cli::migrate_state::execute(state_path: &Path, dry_run: bool) -> Result<MigrateResult, MigrateError>` exists.
        // If not, Worker either: (a) extracts helper, OR (b) duplicates the detect+parse+write logic here.
        // Architect recommends Strategy B (extraction) — keeps cli/<subcmd>.rs::run thin (just args + helper + print).

        let result = crate::cli::migrate_state::execute(
            &PathBuf::from(&p.state_path),
            p.dry_run,
        )
        .map_err(|e| mcp_error("migrate_state", 1, e))?;

        Ok(Json(MigrateStateOutput {
            from: result.from,
            to: result.to,
            seen_count: result.seen_count,
        }))
    }

    #[tool(
        name = "state_backfill",
        description = "Extract advisory IDs from inbox `processed`/`dismissed` rows and union into state.seen_advisories[]."
    )]
    fn state_backfill(
        &self,
        Parameters(p): Parameters<StateBackfillInput>,
    ) -> Result<Json<StateBackfillOutput>, ErrorData> {
        let result = crate::cli::state_backfill::execute(
            &PathBuf::from(&p.state_path),
            &PathBuf::from(&p.inbox_path),
            /* dry_run = */ false,
        )
        .map_err(|e| mcp_error("state_backfill", 1, e))?;

        Ok(Json(StateBackfillOutput {
            backfilled_count: result.backfilled_count,
        }))
    }

    #[tool(
        name = "scan_and_append",
        description = "Composite: parse_report → dedup → append rows into inbox + update state, atomic per file."
    )]
    fn scan_and_append(
        &self,
        Parameters(p): Parameters<ScanAndAppendInput>,
    ) -> Result<Json<ScanAndAppendOutput>, ErrorData> {
        let result = crate::cli::scan_and_append::execute(
            &p.report_text,
            &PathBuf::from(&p.inbox_path),
            &PathBuf::from(&p.state_path),
        )
        .map_err(|e| mcp_error("scan_and_append", 1, e))?;

        Ok(Json(ScanAndAppendOutput {
            appended: result.appended,
            skipped_dedup: result.skipped_dedup,
            total_open: result.total_open,
        }))
    }
}

// ──────────────────────────────────────────────────────────────
// ServerHandler — `#[tool_handler]` macro generates full impl
// ──────────────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for AdvisoryInboxService {}

// ──────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report_with_sentinel() -> String {
        r#"# Scan report

<!-- INBOX_APPEND_START -->
| 2026-05-28 | CVE-2026-9999 | https://example.test | foo@<1.0 | src/x.ts:1 | High | open | - |
<!-- INBOX_APPEND_END -->
"#
        .to_string()
    }

    fn sample_report_no_sentinel() -> String {
        "# Just text, no sentinel.\n".to_string()
    }

    #[test]
    fn parse_report_happy_path() {
        let svc = AdvisoryInboxService::new();
        let out = svc
            .parse_report(Parameters(ParseReportInput {
                report_text: sample_report_with_sentinel(),
            }))
            .expect("parse_report should succeed");
        assert_eq!(out.0.advisories_found, 1);
        assert_eq!(out.0.rows.len(), 1);
        assert_eq!(out.0.rows[0].advisory_id, "CVE-2026-9999");
    }

    #[test]
    fn parse_report_missing_sentinel_returns_mcp_error() {
        let svc = AdvisoryInboxService::new();
        let err = svc
            .parse_report(Parameters(ParseReportInput {
                report_text: sample_report_no_sentinel(),
            }))
            .expect_err("parse_report should fail on missing sentinel");
        assert_eq!(err.code.0, -32000);
        let data = err.data.as_ref().expect("data present");
        assert_eq!(data["subcmd"], "parse_report");
        assert_eq!(data["exit_code"], 1);
    }

    #[test]
    fn dedup_with_mock_state_filters_seen_ids() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut tmp = NamedTempFile::new().expect("temp file");
        let state_json = serde_json::json!({
            "schema_version": 1,
            "last_scan_at": "2026-05-28T00:00:00Z",
            "seen_advisories": ["CVE-2026-1111"],
            "agent_version": "test@0.0.0"
        });
        writeln!(tmp, "{}", state_json.to_string()).expect("write state");
        let path = tmp.path().to_string_lossy().to_string();

        let rows: Vec<AdvisoryRow> = vec![
            // Worker constructs 2 rows: one with seen ID, one fresh.
            // Use crate::row::parse_row on a fixture OR build via struct literal if all fields are pub.
            // Architect leaves exact construction to Worker — depends on AdvisoryRow public surface.
        ];

        // Skip-detail: Worker fills rows vec then asserts:
        //   kept.len() == 1 (the fresh ID)
        //   skipped.len() == 1 (CVE-2026-1111)
        //   observed_ids.len() == 2

        let svc = AdvisoryInboxService::new();
        let _ = svc.dedup(Parameters(DedupInput {
            state_path: path,
            rows,
        }));
        // Worker fills assertions once row construction approach decided (Tầng 2).
    }
}
```

**Lưu ý:**
- Imports may need adjustment per Anchors #6 + #13 (ToolRouter, Parameters, Json exact paths).
- Anchor #5: `ErrorCode(-32000)` is the draft; if rmcp 1.7.0 makes `ErrorCode` non-tuple, switch to the correct constructor (Worker `cargo doc` lookup).
- Anchor #23: `serde_json::Value` field for `stack_scanned` — if `JsonSchema` derive fails on the struct, swap to `String` or `Option<serde_json::Map<String, Value>>` and verify.
- The `migrate_state` / `state_backfill` / `scan_and_append` tools call `cli::<subcmd>::execute(...)` — Worker MUST extract these helpers from existing `cli/<subcmd>.rs::run()` per Strategy B (Architectural Decision #3). If the helper signature differs from the draft, Worker self-adjusts the tool body. The CLI `run()` then becomes a thin wrapper: `pub fn run(args: &XArgs) -> Result<()> { let r = execute(...)?; print_json(&r)?; Ok(()) }`.
- The `parse_report` + `dedup` + `append` tools use Strategy A (inline lib calls). They do NOT require `cli/<subcmd>.rs` refactor.
- The `append` tool's `total_open` calculation may need adjustment depending on `AdvisoryRow::status_is_open()` existing — Worker Task 0 verify `grep -n "status_is_open\|status.*==.*Open" src/row.rs`; if absent, Worker filters via `row.status == Status::Open` (assuming `Status` is `PartialEq`) or adds the helper. Tầng 2 self-fix.
- All `mcp_error()` calls accept `impl ToString` so any error type with `Display` works (anyhow::Error, &str, custom errors).
- The unit test `dedup_with_mock_state_filters_seen_ids` is intentionally incomplete (rows vec is empty in the draft) — Worker fills row construction once decision made on `AdvisoryRow` constructor strategy. The test compiles + runs; Worker adds assertions in EXECUTE.
- The `#[derive(Clone)]` on `AdvisoryInboxService` is conjectural per Architectural Decision #6 — if compile fails, Worker removes Clone (and may need to derive separately on `ToolRouter` field if rmcp requires). Anchor #6 dependency.
- DO NOT add `async` to any tool method. Sync per Architectural Decision #6.

---

### Task 4: NEW `tests/mcp_tools_cli.rs` — integration tests (spawn binary + JSON-RPC)

**File:** `tests/mcp_tools_cli.rs` (NEW — Anchor #17 confirms greenfield).

**Tạo:**

```rust
//! Integration tests for MCP tool dispatch (P011).
//!
//! These tests spawn `advisory-inbox serve`, write JSON-RPC `initialize` + `tools/list`
//! + `tools/call` requests to stdin, read responses from stdout, and assert tool
//! registration + round-trip dispatch shape.
//!
//! Pattern matches P010 `tests/serve_cli.rs` — close stdin to trigger graceful exit.

use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::process::{Command, Stdio};

const INIT_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.0"}}}"#;

const TOOLS_LIST_REQUEST: &str = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#;

fn spawn_serve() -> std::process::Child {
    Command::cargo_bin("advisory-inbox")
        .expect("cargo_bin advisory-inbox")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn advisory-inbox serve")
}

#[test]
fn tools_list_returns_six_tools() {
    let mut child = spawn_serve();

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{}", INIT_REQUEST).expect("write initialize");
        writeln!(stdin, "{}", TOOLS_LIST_REQUEST).expect("write tools/list");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for child");
    assert!(output.status.success(), "exit not 0: {:?}", output.status);

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");

    // Assert each tool name appears in some tools/list response line.
    for name in [
        "parse_report",
        "dedup",
        "append",
        "migrate_state",
        "state_backfill",
        "scan_and_append",
    ] {
        assert!(
            stdout.contains(&format!("\"name\":\"{}\"", name)),
            "tools/list missing tool {}: {}",
            name,
            stdout
        );
    }

    // Also assert the JSON-RPC id echo for tools/list request (id=2).
    assert!(stdout.contains("\"id\":2"), "tools/list response missing id=2 echo: {}", stdout);
}

#[test]
fn tools_call_parse_report_round_trip() {
    let mut child = spawn_serve();

    // The arguments object follows the ParseReportInput schema (single field: report_text).
    let call_request = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"parse_report","arguments":{"report_text":"# scan\n\n<!-- INBOX_APPEND_START -->\n| 2026-05-28 | CVE-2026-9999 | https://example.test | foo@<1.0 | src/x.ts:1 | High | open | - |\n<!-- INBOX_APPEND_END -->\n"}}}"#;

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{}", INIT_REQUEST).expect("write initialize");
        writeln!(stdin, "{}", call_request).expect("write tools/call");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for child");
    assert!(output.status.success(), "exit not 0: {:?}", output.status);

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");

    assert!(
        stdout.contains("\"id\":3"),
        "tools/call response missing id=3 echo: {}",
        stdout
    );
    assert!(
        stdout.contains("CVE-2026-9999"),
        "tools/call response missing parsed advisory_id: {}",
        stdout
    );
    assert!(
        stdout.contains("\"advisories_found\":1"),
        "tools/call response missing advisories_found=1: {}",
        stdout
    );
}

// OPTIONAL Test C — error response on missing sentinel. Tầng 2 self-decide.
//
// #[test]
// fn tools_call_parse_report_error_on_missing_sentinel() {
//     ...
//     assert!(stdout.contains("\"code\":-32000"), ...);
//     assert!(stdout.contains("\"subcmd\":\"parse_report\""), ...);
// }
```

**Lưu ý:**
- The `tools/call` response format wraps the typed `ParseReportOutput` JSON inside `result.content` per MCP spec — Worker may need to refine the substring asserts if the response wraps `Json<T>` output differently. The substrings used (`"CVE-2026-9999"`, `"advisories_found":1`) are robust to wrapper shape since they're inside the typed output regardless.
- The `tools/call` request payload uses MCP spec `params: { name, arguments }` shape — Worker verify against rmcp 1.7.0 expectations via context7 OR by running once and inspecting response.
- DO NOT add a timeout dep. If test hangs in CI (stdin closed → server SHOULD exit), the CI 60s budget surfaces the issue.
- Optional Test C is nice-to-have for error path coverage; OK to skip in P011 and add in P012 polish phiếu.
- If `tools/list` returns tools in a different shape (e.g., escaped JSON inside a string), Worker MAY parse stdout as JSON and walk the structure with `serde_json::from_str` + `.pointer("/result/tools/0/name")`. Substring asserts are pragmatic shortcut; Worker self-decides.

---

### Task 5: `src/main.rs` — add `mod mcp;` declaration

**File:** `src/main.rs`

**Tìm** (top-of-file module declarations cluster — current shape per Anchor #9 Worker verifies):

```rust
mod cli;
mod inbox;
mod row;
mod sentinel;
mod state;
```

(exact ordering may differ — Worker reads actual file).

**Thay bằng:**

```rust
mod cli;
mod inbox;
mod mcp;
mod row;
mod sentinel;
mod state;
```

**Lưu ý:**
- Maintain alphabetical order if the existing list is alphabetical. If existing list has different ordering (semantic grouping), Worker inserts `mod mcp;` in a sensible location.
- DO NOT touch any other line in main.rs. The dispatch arm changes are NONE for P011 (P010 already mapped `Commands::Serve → exit 5`).
- Verify post-edit: `cargo build` should bring `src/mcp/{mod.rs,tools.rs}` into the crate. If compile errors surface about missing items, address in Task 3 / Task 6.

---

### Task 6: `src/cli/serve.rs` — swap `AdvisoryInboxServer` for `AdvisoryInboxService::new()`

**File:** `src/cli/serve.rs`

**Tìm** (current P010 ship per Anchor #10):

```rust
//! `serve` subcommand — MCP server (stdio JSON-RPC 2.0) via rmcp 1.7.0.
//! ... (P010 docstring) ...

use anyhow::{Context, Result};
use rmcp::{
    ServiceExt,
    handler::server::ServerHandler,
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::io::stdio,
};

#[derive(Debug, Clone, Default)]
pub struct AdvisoryInboxServer;

impl ServerHandler for AdvisoryInboxServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder().build(),
            server_info: Implementation {
                name: "advisory-inbox".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                description: Some(
                    "Advisory inbox state machine — handshake only (P010); tools come in P011."
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: None,
        }
    }
}

pub fn run() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio current_thread runtime for MCP serve")?;

    runtime.block_on(async {
        let server = AdvisoryInboxServer;
        let transport = stdio();
        let running = server
            .serve(transport)
            .await
            .context("initializing MCP server handshake")?;
        running
            .waiting()
            .await
            .context("MCP server transport runtime")?;
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // ... P010 unit tests referencing AdvisoryInboxServer ...
}
```

**Thay bằng:**

```rust
//! `serve` subcommand — MCP server (stdio JSON-RPC 2.0) via rmcp 1.7.0.
//!
//! P011 wires 6 tools via `AdvisoryInboxService` (see `crate::mcp::tools`).
//! Transport layer (stdio + tokio current_thread runtime) unchanged from P010.

use anyhow::{Context, Result};
use rmcp::{ServiceExt, transport::io::stdio};

use crate::mcp::tools::AdvisoryInboxService;

pub fn run() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio current_thread runtime for MCP serve")?;

    runtime.block_on(async {
        let server = AdvisoryInboxService::new();
        let transport = stdio();
        let running = server
            .serve(transport)
            .await
            .context("initializing MCP server handshake")?;
        running
            .waiting()
            .await
            .context("MCP server transport runtime")?;
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

// Unit tests removed — AdvisoryInboxService unit tests live in crate::mcp::tools::tests.
// Integration tests in tests/serve_cli.rs (P010) + tests/mcp_tools_cli.rs (P011) cover spawn-binary behavior.
```

**Lưu ý:**
- Remove the `AdvisoryInboxServer` struct + `impl ServerHandler` block + ALL imports that become unused (`Implementation`, `ServerCapabilities`, `ServerInfo`, `ServerHandler`). Worker verifies via `cargo check` no unused-import warnings.
- Remove the `#[cfg(test)] mod tests` block from P010 — those tests referenced `AdvisoryInboxServer.get_info()`. P011's macro-generated `get_info()` on `AdvisoryInboxService` is functionally equivalent (name + version from Cargo.toml), and `mcp/tools.rs` `#[cfg(test)] mod tests` plus integration tests cover the surface adequately.
- KEEP the runtime/transport/serve/waiting pipeline EXACTLY as P010 shipped. Only the server construction line changes.
- If P010 `tests/serve_cli.rs` references `AdvisoryInboxServer` Rust identifier (Anchor #18 verify), Worker updates that test to reference `AdvisoryInboxService` OR (preferred) since `serve_cli.rs` is a spawn-binary test, it likely only matches on JSON-RPC stdout strings — no Rust identifier coupling — so no change needed. Worker verify by `grep -n "AdvisoryInbox" tests/serve_cli.rs`.

---

### Task 7: Extract `execute(...)` helpers in `src/cli/{migrate_state,state_backfill,scan_and_append}.rs` (Strategy B per Architectural Decision #3)

**Files:** `src/cli/migrate_state.rs`, `src/cli/state_backfill.rs`, `src/cli/scan_and_append.rs`.

**For each file, refactor pattern:**

1. Identify the pure-logic block inside current `pub fn run(...) -> Result<()>` — everything between "args parsed / file path resolved" and "JSON serialized + printed to stdout".
2. Extract that block into `pub fn execute(<typed args>) -> Result<<TypedResult>, <TypedError>>`. The result struct fields match the MCP output struct fields from Task 3.
3. The remaining `run(...)` becomes:
   ```rust
   pub fn run(args: &XArgs) -> Result<()> {
       let result = execute(/* args mapped */)?;
       serde_json::to_writer(std::io::stdout(), &result)?;
       println!();
       Ok(())
   }
   ```

**Result struct shapes (suggested — Worker adjusts to existing types if already present):**

```rust
// src/cli/migrate_state.rs
#[derive(Debug, Serialize)]
pub struct MigrateResult {
    pub from: String,
    pub to: String,
    pub seen_count: usize,
}
pub fn execute(state_path: &Path, dry_run: bool) -> Result<MigrateResult, MigrateError> { /* P007 logic */ }

// src/cli/state_backfill.rs
#[derive(Debug, Serialize)]
pub struct StateBackfillResult {
    pub backfilled_count: usize,
}
pub fn execute(state_path: &Path, inbox_path: &Path, dry_run: bool) -> Result<StateBackfillResult, /* existing error */> { /* P008 logic */ }

// src/cli/scan_and_append.rs
#[derive(Debug, Serialize)]
pub struct ScanAndAppendResult {
    pub appended: usize,
    pub skipped_dedup: usize,
    pub total_open: usize,
}
pub fn execute(report_text: &str, inbox_path: &Path, state_path: &Path) -> Result<ScanAndAppendResult, /* existing error */> { /* P009 logic */ }
```

**Lưu ý:**
- DO NOT change `pub fn run(...) -> Result<()>` signature — existing CLI integration tests (`tests/migrate_state_cli.rs`, `tests/state_backfill_cli.rs`, `tests/scan_and_append_cli.rs`) keep passing.
- Result structs MUST derive `Serialize` so they serialize the same JSON as the current inline `serde_json::json!(...)` calls. Worker verifies field names match the existing JSON output exactly (existing integration tests assert specific JSON keys).
- The error types (`MigrateError`, etc.) MUST be `Display` + `Send + Sync + 'static` so `mcp_error("...", N, e)` from `mcp/tools.rs` can wrap them. They already are per P007/P008/P009 ship (anyhow + thiserror pattern).
- If `parse_report` / `dedup` / `append` ALSO need refactoring (Strategy B optional for these), Worker may extract their helpers too — but it's NOT required since `mcp/tools.rs` inlines those calls directly into the lib (Strategy A). Tầng 2 self-decide.
- Read CLAUDE.md DEFINITION OF DONE: `cargo test --all` MUST pass after refactor. The most likely regression vector is JSON field ordering or naming drift; Worker compares pre/post test output exactly.

---

### Task 8: Update `docs/ARCHITECTURE.md` + `docs/CHANGELOG.md` + `README.md` (Docs Gate Tầng 1)

**`docs/ARCHITECTURE.md` §5 — append P011 scaffold-status entry after the existing P010 entry (line ~255):**

```markdown
- P011: `src/mcp/{mod.rs, tools.rs}` shipped — `AdvisoryInboxService` struct (`#[derive(Debug, Clone)]`) with `tool_router: ToolRouter<Self>` field, holds 6 `#[tool]` methods via `#[tool_router]` macro. `#[tool_handler] impl ServerHandler for AdvisoryInboxService {}` auto-generates `get_info()` (reads name/version from `Cargo.toml`, enables tools capability). 6 tools: `parse_report` / `dedup` / `append` / `migrate_state` / `state_backfill` / `scan_and_append` — each with typed input + output struct (`#[derive(Deserialize/Serialize, JsonSchema)]`). Error mapping: `ErrorData { code: ErrorCode(-32000), message, data: { subcmd, exit_code } }` per ARCHITECTURE §6. `Cargo.toml`: rmcp features +macros +schemars; new dep `schemars = "1.0"` (or rmcp re-export per Worker decision). `src/cli/serve.rs` swapped: removed `AdvisoryInboxServer` unit struct + manual `impl ServerHandler`; now constructs `AdvisoryInboxService::new()`. Runtime/transport pipeline unchanged from P010. Strategy B refactor: `cli/{migrate_state,state_backfill,scan_and_append}.rs` extracted `pub fn execute(...)` from `pub fn run(...)` — CLI `run()` signatures unchanged; MCP tools + CLI subcmds share the same `execute` core. `AdvisoryRow` + `Status` + `Severity` (P002) gained `JsonSchema` derive (additive). 3 unit tests in `mcp/tools.rs` (`parse_report_happy_path`, `parse_report_missing_sentinel`, `dedup_with_mock_state`); 2 integration tests in `tests/mcp_tools_cli.rs` (`tools_list_returns_six_tools`, `tools_call_parse_report_round_trip`). Binary release size: <Worker fills delta> MB (vs P010 1.96 MB).
```

**`docs/ARCHITECTURE.md` §6 — update Status section (line ~262-266):**

**Tìm:**

```markdown
### Status

- **P010 (shipped 2026-05-28):** Handshake support — ...
- **P011 (planned):** 6 tools registered via `ToolRouter`. ServerCapabilities flips `.enable_tools()`. `src/mcp/` module introduced.
```

**Thay bằng:**

```markdown
### Status

- **P010 (shipped 2026-05-28):** Handshake support — `initialize` JSON-RPC returns valid `InitializeResult`; 0 tools; capabilities empty.
- **P011 (shipped <Worker date>):** 6 tools registered via `#[tool_router]` + `#[tool_handler]` macros on `AdvisoryInboxService`. `ServerCapabilities` auto-flipped to `enable_tools()` by macro. `src/mcp/{mod.rs, tools.rs}` shipped. Tool input/output schemas auto-derived via `schemars`. Tool failures surface as JSON-RPC `ErrorData { code: -32000, data: { subcmd, exit_code } }`; server keeps running.
```

**`docs/CHANGELOG.md` — prepend entry (newest at top):**

```markdown
## P011 — MCP tool dispatch (6 tools) — <Worker date>

**Feat:** Register 6 tools via rmcp `#[tool_router]` + `#[tool_handler]` macros on new `AdvisoryInboxService` struct in `src/mcp/tools.rs`. Each tool wraps existing P002-P009 lib functions with typed JSON-schema-derived input/output (via `schemars`). Tools: `parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`.

**Deps:** Added rmcp features `macros` + `schemars`; new dep `schemars = "1.0"` (or rmcp re-export — see Discovery Report).

**Refactor:** Extracted `pub fn execute(...)` from `cli/migrate_state.rs`, `cli/state_backfill.rs`, `cli/scan_and_append.rs` `run()` to share core logic between CLI subcmd + MCP tool — CLI `run()` public signatures unchanged.

**Removed:** `AdvisoryInboxServer` unit struct + manual `impl ServerHandler` from `cli/serve.rs` (superseded by macro-generated handler on `AdvisoryInboxService`).

**Tests:** +3 unit (mcp/tools.rs), +2 integration (tests/mcp_tools_cli.rs). `tools/list` returns 6 tool names; `tools/call parse_report` round-trips structured output.

**Capabilities:** ServerCapabilities now declares `tools` (P010 was empty).

**Error format:** Tool failures = JSON-RPC `{ code: -32000, message, data: { subcmd, exit_code } }` per ARCHITECTURE §6.

**Out-of-scope (deferred):** `.mcp.json` `mcpServers` registration (deploy step); `tokio::task::spawn_blocking` for IO-heavy tools (P012 polish); per-tool integration tests for the other 5 tools (P012).

**home:** docs/ARCHITECTURE.md §5 + §6 (durable); docs/discoveries/P011.md (operational)
```

**`README.md` — expand MCP quick-start section (add `tools/call` example):**

Worker locates the existing MCP section (P010 added it per Anchor #16 P010 ship) and appends:

```markdown
### Calling a tool via MCP

After `initialize` handshake, send a `tools/call` JSON-RPC request:

\`\`\`json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "parse_report",
    "arguments": {
      "report_text": "<agent report markdown with sentinel block>"
    }
  }
}
\`\`\`

Response includes structured `result.content` matching `ParseReportOutput`:

\`\`\`json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [...],
    "isError": false
  }
}
\`\`\`

6 tools available: `parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`. See `docs/ARCHITECTURE.md` §6 for full input/output schemas.

On failure, tools return JSON-RPC errors with `code: -32000` and `data: { subcmd, exit_code }` for client diagnostics.
```

**Lưu ý:**
- Replace backslash-fences with backtick fences when actually writing README.md (the example above escapes them for embedding in this phiếu).
- The exact response shape (`result.content` array vs typed inline) depends on rmcp 1.7.0 — Worker may need to refine the example after observing actual `tools/call` response from Test 5.
- DO NOT add MCP installation steps (e.g., `cargo install`, `.mcp.json` editing) — that's deploy territory + P013 tarot install phiếu.

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `Cargo.toml` | Task 1: rmcp features +macros +schemars; new dep `schemars = "1.0"` (or rely on rmcp re-export per Anchor #3) |
| `src/main.rs` | Task 5: add `mod mcp;` declaration |
| `src/cli/serve.rs` | Task 6: remove `AdvisoryInboxServer` + manual `impl ServerHandler`; construct `AdvisoryInboxService::new()`; clean unused imports |
| `src/mcp/mod.rs` | Task 2: NEW — module root, `pub mod tools;` + doc comment |
| `src/mcp/tools.rs` | Task 3: NEW — `AdvisoryInboxService` + 6 input/output structs + 6 `#[tool]` methods + `mcp_error` helper + 3 unit tests |
| `tests/mcp_tools_cli.rs` | Task 4: NEW — 2 integration tests (`tools/list` + `tools/call parse_report`) |
| `src/cli/migrate_state.rs` | Task 7: extract `pub fn execute(...) -> Result<MigrateResult, MigrateError>`; `run()` becomes thin wrapper |
| `src/cli/state_backfill.rs` | Task 7: extract `pub fn execute(...) -> Result<StateBackfillResult, ...>`; `run()` thin wrapper |
| `src/cli/scan_and_append.rs` | Task 7: extract `pub fn execute(...) -> Result<ScanAndAppendResult, ...>`; `run()` thin wrapper |
| `src/row.rs` | Anchor #7 conditional: add `JsonSchema` derive to `AdvisoryRow` + `Status` + `Severity` if absent. Pure additive. |
| `docs/ARCHITECTURE.md` | Task 8: §5 append P011 entry; §6 flip Status from "P011 planned" to "P011 shipped" + macro details |
| `docs/CHANGELOG.md` | Task 8: prepend P011 entry (newest at top) |
| `README.md` | Task 8: expand MCP quick-start with `tools/call` example |
| `docs/discoveries/P011.md` | Discovery Report — per-phiếu file per CLAUDE.md §DISCOVERY REPORT |
| `docs/DISCOVERIES.md` | Discovery Report — append 1-line index entry (newest at top) |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/sentinel.rs` | `sentinel::extract_block(&str) -> Result<String, SentinelError>` continues to work — tool `parse_report` calls it |
| `src/state.rs` | `state::read(&Path) -> Result<StateFile, StateReadError>` + `state::write_atomic(&Path, &StateFile) -> Result<(), StateWriteError>` continue to work |
| `src/inbox.rs` | `inbox::read_inbox(&Path) -> Result<String, InboxError>` + `insert_rows(&mut String, &[AdvisoryRow]) -> Result<(), InboxError>` + `write_atomic(&Path, &str) -> Result<(), InboxError>` + `parse_rows(&str) -> Result<Vec<AdvisoryRow>, InboxError>` continue to work |
| `src/cli/parse_report.rs` | `pub fn run(...)` UNCHANGED (Strategy A — `mcp/tools.rs::parse_report` calls sentinel + row::parse_row directly, NOT cli/parse_report.rs::run); existing test suite passes |
| `src/cli/dedup.rs` | Same — Strategy A; CLI run unchanged |
| `src/cli/append.rs` | Same — Strategy A; CLI run unchanged |
| `src/cli/init.rs` | No MCP equivalent; `cli::init::run` unchanged |
| `src/cli/mod.rs` | Existing `pub mod` registrations unchanged |
| `tests/sentinel_*` `tests/parse_report_cli.rs` `tests/dedup_cli.rs` `tests/append_cli.rs` `tests/migrate_state_cli.rs` `tests/state_backfill_cli.rs` `tests/scan_and_append_cli.rs` | All continue to pass after Strategy B refactor (Task 7); Worker confirms `cargo test --all` clean post-refactor before MCP tools added |
| `tests/serve_cli.rs` | P010 integration test continues to pass after `AdvisoryInboxServer → AdvisoryInboxService` swap (spawn-binary test asserts JSON-RPC stdout, not Rust identifier — verify Anchor #18) |
| `.mcp.json` | NOT modified — `mcpServers` `advisory-inbox` registration deferred to deploy step (P010 ship + P013 tarot install) |

---

## Luật chơi (Constraints)

1. **NO new dep besides `schemars = "1.0"` (or relying on rmcp re-export per Anchor #3).** Adding any other crate (e.g., `tokio-util`, `wait_timeout`) is Hard Stop.
2. **NO bump of rmcp version.** Stays 1.7.0. Only feature flag additions.
3. **NO `#[tokio::main]` on `src/main.rs`.** P001-P010 sync-main contract preserved. Tokio current_thread runtime stays inline in `cli::serve::run()`.
4. **NO new `unsafe { }` blocks.** Per CLAUDE.md §HARD STOPS #7.
5. **NO change to CLI subcmd `pub fn run(...) -> Result<()>` signatures.** Strategy B extracts helpers; CLI `run()` becomes thin wrapper but keeps signature.
6. **NO change to state schema, inbox format, sentinel marker, CLI exit codes.** MCP is parallel surface; lib + format invariants unchanged.
7. **NO async tool methods.** All 6 `#[tool]` methods sync per Architectural Decision #6. `spawn_blocking` deferred to P012.
8. **NO modification of `.mcp.json` `mcpServers`.** Deploy step separate.
9. **NO process::exit inside tool methods.** Tool errors return `Err(ErrorData)` — JSON-RPC error response; server keeps running.
10. **Token leak grep MUST pass.** New files (`src/mcp/`, `tests/mcp_tools_cli.rs`) + Cargo.toml diff grep clean for `ghp_|gho_|ghu_|ghs_|github_pat_`.
11. **Each tool name MUST exactly match ARCHITECTURE §6 table.** snake_case, no dashes. `parse_report` (NOT `parse-report` or `parseReport`).
12. **CLI tests MUST continue passing.** `cargo test --all` post-refactor (Task 7) before adding MCP tools — verify no JSON field drift.
13. **Macro-generated `get_info()` MUST return server name = `"advisory-inbox"`.** Verify via Test 4 integration — `tools/list` precedes `initialize`-response inspection, but the same response carries `serverInfo`.
14. **6 tools = 6, no more, no less.** No bonus tools (e.g., `health`, `version`) in P011. P012+ may add.
15. **Sub-mech A trigger smoke MUST verify-fire post-implementation.** Test 4 + Test 5 are the trigger assertions.
16. **Discovery Report MUST be written per CLAUDE.md §DISCOVERY REPORT.** `docs/discoveries/P011.md` + 1-line index in `docs/DISCOVERIES.md`. Includes: which Anchors needed Worker self-fix (especially Tier 2 decisions per Anchors #3, #5, #6, #13, #18, #23), whether `schemars` dep needed or rmcp re-export sufficed, binary size delta.
17. **Docs Gate Tầng 1 MUST pass.** ARCHITECTURE §5 + §6 + CHANGELOG + README updated. MCP surface = security boundary per RULES.md §11 = AUTO Tầng 1 (no Tầng 2 escape).

---

## Nghiệm thu

### Automated
- [ ] `cargo check` clean (0 warnings) — Sub-mech B
- [ ] `cargo test --all` ≥69 tests pass (P010 baseline ~64 + ≥5 new in P011) — Sub-mech B
- [ ] `cargo test --test mcp_tools_cli` ≥2 integration tests pass — Sub-mech A
- [ ] `cargo test mcp::tools::tests` ≥3 unit tests pass — Sub-mech B
- [ ] `cargo test --test serve_cli` P010 integration test still passes — regression
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo fmt --check` no diff
- [ ] `cargo build --release` exit 0, 0 warnings
- [ ] `cargo update --dry-run` no surprise major bump — Sub-mech E
- [ ] `target/release/advisory-inbox --help` shows `serve` subcmd (unchanged surface from P010)
- [ ] Sub-mech F token leak grep: `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/mcp/ tests/mcp_tools_cli.rs Cargo.toml` → 0 hits

### Manual Testing
- [ ] **Trigger smoke (Sub-mech A):** `echo -e '{"jsonrpc":"2.0","id":1,"method":"initialize",...}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run --quiet -- serve` → stdout contains all 6 tool names + `"id":1` + `"id":2` echoes; exit 0
- [ ] **Tool call round-trip:** same spawn + `tools/call` for `parse_report` with a fixture report_text → response contains parsed `advisory_id` value + `advisories_found` count
- [ ] **Tool error path (optional):** `tools/call parse_report` with empty report_text → JSON-RPC error response with `code: -32000` + `data.subcmd: "parse_report"`
- [ ] **Capabilities flip:** `initialize` response's `result.capabilities` includes `tools` (compared to P010 empty capabilities)
- [ ] **Binary size sanity:** `ls -la target/release/advisory-inbox` < 10 MB (P010 baseline 1.96 MB; schemars + macros delta acceptable)

### Regression
- [ ] All 6 CLI subcmds (parse-report, dedup, append, migrate-state, state-backfill, scan-and-append) produce identical stdout JSON pre/post Strategy B refactor — verify by running fixture inputs and diffing output bytes
- [ ] P010 `serve` integration test (`tests/serve_cli.rs`) passes — initialize handshake unchanged
- [ ] P001-P009 unit tests in `src/sentinel.rs`, `src/row.rs`, `src/state.rs`, `src/inbox.rs` continue passing — no lib API change
- [ ] `init` subcmd unaffected (no MCP equivalent shipped)
- [ ] No `unsafe` introduced anywhere — `grep -rn 'unsafe' src/` returns 0

### Docs Gate (AUTO Tầng 1 per RULES.md §11 — MCP surface = security boundary)
- [ ] `docs/ARCHITECTURE.md` §5 — P011 scaffold-status entry appended after P010
- [ ] `docs/ARCHITECTURE.md` §6 — Status flipped from "P011 planned" to "P011 shipped"; macro details added
- [ ] `docs/CHANGELOG.md` — P011 entry prepended (newest at top); includes `home:` line per RULES.md §6
- [ ] `README.md` — MCP quick-start section expanded with `tools/call` example for at least 1 tool
- [ ] `docs/RULES.md` — no changes needed (Sub-mech B existing entry covers schemars verification step)
- [ ] `docs/PROJECT.md` — no phase status change (Phase 3 closes with P011 ship → can flip to "Phase 4 next" if applicable; Worker self-decides whether the status delta merits an edit)

### Discovery Report
- [ ] Write `docs/discoveries/P011.md` covering:
  - Assumptions in phiếu — CORRECT / WRONG (with `file:line` citations from rmcp source if Anchor #5/#6/#13 needed cargo-doc lookup)
  - Whether `schemars` explicit dep was needed OR rmcp re-export sufficed (Anchor #3 resolution)
  - Strategy A vs B picks per subcmd (Task 7) — which subcmds got `execute()` extracted
  - Binary release size delta (P010 1.96 MB → P011 <X> MB)
  - `serde_json::Value` `JsonSchema` derive resolution (Anchor #23)
  - Any tool method that needed `spawn_blocking` deferred to P012 (NOT shipped per Decision #6)
  - Scope expansions (if any — note original vs shipped, with reason)
  - Tier escalations (write "None" if no 2→1)
- [ ] Append 1-line index entry to `docs/DISCOVERIES.md` (newest at top): `- P011 (<date>) — MCP tool dispatch + schemars dep; binary +<X> MB; Strategy B refactor for <N> subcmds → docs/discoveries/P011.md`
