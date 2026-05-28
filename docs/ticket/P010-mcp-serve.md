# PHIẾU P010: `serve` subcmd — rmcp stdio JSON-RPC handshake

> **ID format:** `P010` — counter `.phieu-counter` = 10 sau P009 ship.
> **Filename:** `docs/ticket/P010-mcp-serve.md`
> **Branch:** `feat/P010-mcp-serve`

---

> **Loại:** feat
> **Tầng:** 1
> **Ưu tiên:** P1 (Phase 3 opener — handshake foundation cho P011 tool dispatch; P013 tarot install dùng `advisory-inbox serve` qua `.mcp.json`; without P010 ship, P011/P013 blocked)
> **Ảnh hưởng:** `src/cli/serve.rs` (stub → real impl, inline rmcp wiring — no new `src/mcp/` module này phiếu), `src/main.rs` (`Commands::Serve` dispatch arm gain `MCP transport error → exit 5` mapping per ARCHITECTURE §1 exit-code table), `tests/serve_cli.rs` (NEW — at least 1 unit-level metadata test + 1 integration spawn test), `docs/ARCHITECTURE.md` §5 (P010 scaffold-status entry: `cli/serve.rs` wired with rmcp stdio, NO `src/mcp/` module shipped yet — P011 will introduce if needed) + §6 (note: handshake-only this phiếu; 6 tools come in P011) + §1 (exit code 5 documented for MCP), `docs/CHANGELOG.md` (P010 entry — first MCP code, tokio async runtime first use, rmcp 1.7.0 first integration), `README.md` (MCP quick-start: `advisory-inbox serve` invocation + `.mcp.json` snippet — Worker check Anchor #16)
> **Dependency:** P001 (CLI scaffold + `Commands::Serve` variant + `cli/serve.rs` stub + `cli/mod.rs` registers `pub mod serve;`), `Cargo.toml` deps `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` + `tokio = { version = "1", features = ["rt", "macros", "io-std"] }` đã có (P001 verified). KHÔNG dependency vào P002-P009 lib code (handshake KHÔNG gọi sentinel/row/state/inbox).
> **Lane:** **Guarded** (MCP transport surface — first network-shape protocol code; RULES.md §1 Guarded scope: "MCP server logic (rmcp transport, tool dispatch)"; transport-io qua stdin/stdout = process boundary; INV-WF-001 trigger verifiability applies — handshake MUST verify-fire)
> **Sub-mech áp dụng:** **A** (trigger — MCP server starts when invoked via `advisory-inbox serve` from `.mcp.json` config OR direct shell; smoke test = pipe JSON-RPC `initialize` to stdin, expect valid response on stdout, exit 0 when stdin closes), **B** (capability — `cargo check`, `cargo test --test serve_cli`, `cargo build --release`; rmcp 1.7.0 API surface verified via context7 by Architect — see Skills consulted), **D** (persistence — ARCHITECTURE.md §6 server name/version anchored in docs + Cargo.toml `version` is source of truth)

---

## Context

### Vấn đề hiện tại

Phase 1 (P001-P006) + Phase 2 (P007-P009) đã ship 6 CLI subcmd (`parse-report`, `dedup`, `append`, `migrate-state`, `state-backfill`, `scan-and-append`). Phase 3 mở cửa MCP integration: Claude Code session (orchestrator hoặc subagent) gọi tool qua JSON-RPC 2.0 stdin/stdout thay vì spawn binary nhiều lần qua shell.

Hiện tại `src/cli/serve.rs` là stub:

```rust
//! Stub for `serve` (MCP) subcommand. Real logic wired in P010.

use anyhow::Result;

pub fn run() -> Result<()> {
    println!("TODO: serve (MCP stdio JSON-RPC) — wired in P010");
    Ok(())
}
```

Phiếu P010 KHÔNG ship tool dispatch — chỉ ship **handshake**: server start, parse MCP `initialize` JSON-RPC request, respond với `InitializeResult` carrying server info (name + version + empty/default capabilities). Khi client gửi `tools/list` request → respond empty list `{ "tools": [] }` (server không crash, just no tools yet). P011 fill 6 tools.

Reference BACKLOG.md P010:
- Lane: Guarded.
- Scope: `cli/serve.rs` start rmcp server with stdio transport. No tools yet, just handshake.
- Acceptance: Send JSON-RPC `initialize` → response valid per MCP spec.
- Sub-mech checks: A (MCP trigger fires from `.mcp.json`), B (rmcp API check via context7).

Reference ARCHITECTURE.md §1 Subcmd serve (line 90-98):
```
advisory-inbox serve
  Transport: stdin/stdout JSON-RPC 2.0 (rmcp stdio)
  Tools exposed: 6 (... 6 tools listed for P011)
  Behavior: Long-running, no exit until stdin closed
```

Reference ARCHITECTURE.md §6 MCP Surface:
- **Name:** `advisory-inbox`
- **Version:** Cargo.toml version (currently `0.1.0` per Cargo.toml line 3)
- **Transport:** stdio JSON-RPC 2.0 (rmcp)

Reference ARCHITECTURE.md §1 exit codes (line 110-119):
```
| 5    | MCP transport error (rmcp serve mode only) |
```

P010 fully exercises exit code 5 for the first time. Subcmd return paths:
- Stdin closed cleanly (EOF) → exit 0 (long-running ended normally).
- rmcp transport / IO error → exit 5.
- Other unexpected panic → exit 2 (fallback — generic processing error).

### Giải pháp

**3 unit công việc chính:**

#### 1. `src/cli/serve.rs` — stub → real impl (inline rmcp stdio wiring)

Architect (Bước 0) đã verify rmcp 1.7.0 API qua context7 (snapshots embedded in Skills consulted). Surface confirmed:

- `rmcp::transport::io::stdio() -> (tokio::io::Stdin, tokio::io::Stdout)` — gated by `transport-io` feature (Cargo.toml has it).
- `rmcp::handler::server::ServerHandler` trait — service implementation interface. Provided methods include `initialize`, `list_tools`, `call_tool`, `get_info`, etc. all have defaults; override only what you need.
- `rmcp::model::ServerInfo` = type alias for `InitializeResult { protocol_version, capabilities: ServerCapabilities, server_info: Implementation, instructions: Option<String> }`.
- `rmcp::model::Implementation { name, title?, version, description?, icons?, website_url? }` — `#[non_exhaustive]`. Construct via field-init OR builder if rmcp exposes one (Worker self-decide; field-init is safest given `#[non_exhaustive]` allows construction inside crate that defines it but external crates must use constructor — Worker Task 0 verify whether `Implementation::new(name, version)` or similar constructor exists; if not, use `..Default::default()` pattern).
- `rmcp::model::ServerCapabilities` — has `builder()` method; `ServerCapabilities::builder().build()` produces default (no capabilities enabled). P011 will `.enable_tools()`. P010 ships **no capabilities enabled** (empty/default) — server still responds to `initialize` per MCP spec, and `tools/list` is OPTIONAL per spec when tools capability not declared.
- `ServiceExt::serve(transport)` — extension trait method on a service implementor; returns `Future<Output = Result<RunningService<RoleServer, S>, ServerInitializeError>>`. Awaiting completes after `initialize` handshake. Then call `.waiting().await` on the `RunningService` to block until stdin closes / cancel.

**Architectural decisions (LOCKED by Architect):**

1. **NO `src/mcp/` module shipped P010.** ARCHITECTURE.md §5 list `src/mcp/{mod.rs, tools.rs, transport.rs}` as planned, but for handshake-only (no tool dispatch, no schema marshaling), the entire wiring fits in `cli/serve.rs` (~50-80 lines). Adding `src/mcp/` adds 3 files + module boundary for code we don't have yet. **Defer to P011** when tool dispatch needs structure (likely `src/mcp/tools.rs` + extract handler struct to `src/mcp/mod.rs`). P010 keeps the surface minimal. Worker MUST NOT create `src/mcp/` directory in this phiếu.

2. **`#[tokio::main]` STAYS OUT of `src/main.rs`.** Per P001 history (Anchor #7 confirmed sync main; P001-P009 all use sync `fn main() -> anyhow::Result<()>`). Async runtime is ONLY needed inside `cli::serve::run()` — every other subcmd is sync. Solution: inside `serve::run()`, build a tokio current-thread runtime, block on async serve fn. Reason: keep cold-start fast for sync subcmds (no runtime init for `parse-report`); only pay tokio cost when actually serving.

3. **Runtime flavor: `current_thread`.** Cargo.toml declares `tokio = { features = ["rt", "macros", "io-std"] }` — note **NO `rt-multi-thread`**. The `rt` feature alone provides only `Builder::new_current_thread()`. Worker MUST use `tokio::runtime::Builder::new_current_thread().enable_all().build()?` (NOT `Builder::new_multi_thread()` — feature gate missing). Single-threaded runtime is sufficient for stdio JSON-RPC serial request/response — there's only one client over one pipe pair.

4. **Server name + version:** name = `"advisory-inbox"` (literal — matches ARCHITECTURE §6 + binary name + `.mcp.json` key); version = `env!("CARGO_PKG_VERSION")` (currently `"0.1.0"` per Cargo.toml; auto-updates on bump). Source of truth = `Cargo.toml`. No hardcoding version string.

5. **Capabilities = empty/default.** This phiếu DOES NOT declare `tools` capability. Rationale: declaring `tools` while `list_tools` returns empty is misleading to clients. When P011 wires tools, P011 flips `.enable_tools()`. P010 ships honest "I'm a server with handshake support, no features advertised yet" — still valid MCP.

6. **Custom `get_info()` override.** Implement the `ServerHandler` trait MANUALLY (not via `#[tool_handler]` macro — that macro is for tool servers + reads name/version from Cargo.toml automatically but requires `ToolRouter`; for P010 with zero tools, manual impl is cleaner). Override `get_info()` to return `ServerInfo` with our `Implementation`. Other trait methods use provided defaults (which return appropriate MCP errors or empty lists).

7. **Server struct shape:** unit struct `pub struct AdvisoryInboxServer;` (no state). Inside `serve.rs` module. P011 will extend with `ToolRouter` field if needed.

8. **Process lifecycle:** server runs until stdin EOF (client closes pipe) OR Ctrl-C. `.waiting().await` returns on either condition. Return `Ok(())` → exit 0. On rmcp transport errors during init/runtime, return error → main.rs maps to exit 5.

9. **Error mapping:** `rmcp` API surfaces `ServerInitializeError` from `.serve()` (initialize handshake error) and the `.waiting()` future may yield runtime errors. Both wrap into `anyhow::Error` via `?`. main.rs downcast: any error from `serve::run` → exit 5 (only one error family from MCP transport; no need for fine-grained mapping yet). Worker MUST verify in Task 0 whether `.waiting()` returns `Result<_, _>` or just `()` — rmcp API check via context7 if context7 available at EXECUTE time, else Worker reads `cargo doc --open --package rmcp` or `cargo test rmcp -- --list 2>&1` for surface hints. If `.waiting()` returns plain `()`, drop the `?` and just `runtime.block_on(server.waiting())`.

10. **Stdin EOF semantics:** rmcp 1.7.0 `RunningService::waiting()` documented behavior: completes when transport closes (stdin EOF for stdio transport) OR cancellation token fires (we don't use one). Worker Task 0 may verify via crate docs. If behavior is uncertain, fallback: wrap `.waiting().await` in a match — `Ok(_) => Ok(())`, `Err(e) => Err(e.into())`. Architect locks: exit 0 on graceful EOF, exit 5 on transport error.

#### Pipeline implementation outline

```
1. Build tokio current_thread runtime.
2. Inside runtime.block_on(async {
3.   Build Implementation { name: "advisory-inbox", version: env!("CARGO_PKG_VERSION"), other fields default/None }.
4.   Build ServerCapabilities via builder().build() (no flags enabled — empty).
5.   Build ServerInfo { protocol_version: default, capabilities, server_info: implementation, instructions: None }.
6.   Construct AdvisoryInboxServer (unit struct).
7.   Get stdio transport via rmcp::transport::io::stdio() → (Stdin, Stdout).
8.   Call server.serve((stdin, stdout)).await → RunningService.
9.   Call running.waiting().await → blocks until stdin EOF.
10. });
11. Return Ok(()).
```

#### Code pattern (Architect-drafted; Worker tunes if rmcp 1.7.0 surface differs)

```rust
//! `serve` subcommand — MCP server (stdio JSON-RPC 2.0) via rmcp 1.7.0.
//!
//! This phiếu (P010) ships **handshake only** — no tools registered.
//! P011 will wire 6 tools (parse_report, dedup, append, migrate_state, state_backfill,
//! scan_and_append) per ARCHITECTURE.md §6.
//!
//! Architecture decisions (P010):
//! - Inline rmcp wiring in this file; no `src/mcp/` module yet (defer to P011).
//! - tokio current_thread runtime built INSIDE this fn (no `#[tokio::main]` in main.rs).
//! - `ServerCapabilities` = empty/default (no `tools` declared; honest until P011).

use anyhow::{Context, Result};
use rmcp::{
    ServiceExt,
    handler::server::ServerHandler,
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::io::stdio,
};

/// Unit struct — no state needed for handshake-only server.
/// P011 will extend with `ToolRouter` field for tool dispatch.
#[derive(Debug, Clone, Default)]
pub struct AdvisoryInboxServer;

impl ServerHandler for AdvisoryInboxServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            // protocol_version: default (rmcp picks current MCP spec version)
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
    // All other ServerHandler methods use provided defaults.
    // `list_tools` default returns empty `ListToolsResult` → MCP-compliant empty list.
}

pub fn run() -> Result<()> {
    // Build current-thread runtime (Cargo.toml declares `rt` feature only — NO multi-thread).
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio current_thread runtime for MCP serve")?;

    runtime.block_on(async {
        let server = AdvisoryInboxServer::default();
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
```

**Worker tuning latitude (Tầng 2 self-decide):**
- `Implementation` field order may need adjustment if rmcp 1.7.0 has different fields (`#[non_exhaustive]` means crate-internal extras possible). If `Implementation::new(name: String, version: String)` constructor exists, prefer it over field-init. Worker Task 0 verify.
- If `running.waiting()` returns plain `()` (no Result), drop the `?` and the `.context()`.
- If `stdio()` returns something other than `(Stdin, Stdout)` (e.g., a single composite transport), pass directly to `.serve(transport)`.
- If `protocol_version: Default::default()` doesn't compile (no `Default` impl on `ProtocolVersion`), use the explicit current version constant rmcp exports (e.g., `ProtocolVersion::V_2024_11_05` or similar — Worker verify via `cargo doc`).
- `description` field may not exist on `Implementation` in 1.7.0 — if compile fails, remove the field. Architect verified `#[non_exhaustive]` schema shows it as `Option<String>` per context7 snapshot, but rmcp may have shifted between minor versions.

**Hard constraint:** Worker MUST NOT add `#[tokio::main]` to `src/main.rs`. P001-P009 sync-main contract preserved. If pattern compile-fails, escalate.

#### 2. `src/main.rs` — `Commands::Serve` dispatch arm with exit 5 mapping

**Tìm** (current P001 scaffold passthrough — Worker confirm post-Task 0 via Anchor #2):

```rust
Commands::Serve => cli::serve::run(),
```

**Thay bằng:**

```rust
Commands::Serve => {
    if let Err(e) = cli::serve::run() {
        eprintln!("error: {:#}", e);
        // MCP transport / runtime errors → exit 5 per ARCHITECTURE §1 exit-code table.
        std::process::exit(5);
    }
    Ok(())
}
```

**Lưu ý:**
- Tail `Ok(())` REQUIRED for match-arm uniformity (P004 Turn 1 O1.1 precedent).
- Exit 5 is reserved for MCP per ARCHITECTURE §1 line 118.
- No fine-grained downcast yet — all `serve::run` errors map to exit 5 (only one error surface this phiếu). P011 may refine if it wants to distinguish "tool-not-found" (handler error → JSON-RPC error response, NOT process exit) vs "transport failure" (process exit 5). For P010, transport-only failures surface.
- Pattern differs slightly from `Commands::Append`/`Dedup`/etc. (those use downcast). Serve has only 1 error family → simpler. Acceptable per phiếu spec.

#### 3. `tests/serve_cli.rs` (NEW — at least 1 unit test + 1 integration spawn test)

Unit test (compiled into `serve_cli` integration target — wraps `AdvisoryInboxServer` ServerHandler methods):

- **Test A — `AdvisoryInboxServer::get_info` returns correct metadata:**
  - Construct `AdvisoryInboxServer::default()`.
  - Call `.get_info()` → assert `server_info.name == "advisory-inbox"`, `server_info.version == env!("CARGO_PKG_VERSION")`.
  - Assert `capabilities.tools.is_none()` OR equivalent "no tools declared" check (Worker verifies exact field shape via cargo doc).
  - Note: This test is in `tests/serve_cli.rs` so it needs to reference `advisory_inbox::cli::serve::AdvisoryInboxServer`. Worker verify whether the struct is publicly re-exported from `cli/mod.rs` (`pub use serve::AdvisoryInboxServer`) — IF NOT, add `pub use` in `src/cli/mod.rs` OR move test to `src/cli/serve.rs` `#[cfg(test)]` module. **Recommendation: keep test inline in `src/cli/serve.rs` as `#[cfg(test)] mod tests`** for the unit-level metadata check (no need to expose struct outside the crate). The integration test below (which spawns the binary) goes in `tests/serve_cli.rs`.

Integration test (spawn binary, write JSON-RPC, read response):

- **Test B — Spawn binary + send `initialize` request + verify response shape:**
  - Use `assert_cmd::Command::cargo_bin("advisory-inbox")` with arg `serve`.
  - Pipe an `initialize` JSON-RPC request to stdin, then close stdin to signal EOF (so server exits gracefully).
  - Read stdout, find at least one JSON-RPC response line containing `"result"` (initialize response) with `"serverInfo"` containing `"name":"advisory-inbox"`.
  - Assert exit status = 0 (graceful EOF).
  - Use timeout (5s) to prevent test hang if server doesn't exit cleanly.

  **JSON-RPC request payload (newline-delimited JSON, MCP spec):**
  ```json
  {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.0"}}}
  ```

  **Expected stdout shape (one of the lines):**
  ```json
  {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"...","capabilities":{...},"serverInfo":{"name":"advisory-inbox","version":"0.1.0",...}}}
  ```

  **Implementation pattern (`assert_cmd` + `std::process::Command` with piped stdio):**

  ```rust
  use assert_cmd::cargo::CommandCargoExt;
  use std::io::Write;
  use std::process::{Command, Stdio};
  use std::time::Duration;

  #[test]
  fn serve_responds_to_initialize() {
      let mut child = Command::cargo_bin("advisory-inbox")
          .unwrap()
          .arg("serve")
          .stdin(Stdio::piped())
          .stdout(Stdio::piped())
          .stderr(Stdio::piped())
          .spawn()
          .expect("spawn advisory-inbox serve");

      let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.0.0"}}}"#;

      {
          let stdin = child.stdin.as_mut().expect("stdin handle");
          writeln!(stdin, "{}", request).expect("write request");
      }
      // Close stdin by dropping it.
      drop(child.stdin.take());

      // Wait with timeout — Worker may use wait_timeout crate OR poll loop.
      // Tầng 2 self-decide impl detail; recommendation below.
      let output = child.wait_with_output().expect("wait for child");
      assert!(output.status.success(), "exit not 0: {:?}", output.status);

      let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
      assert!(
          stdout.contains("\"name\":\"advisory-inbox\""),
          "stdout missing serverInfo.name: {}",
          stdout
      );
      assert!(
          stdout.contains("\"id\":1"),
          "stdout missing JSON-RPC id echo: {}",
          stdout
      );
  }
  ```

  **Tầng 2 self-decide:**
  - Whether to add `wait_timeout` dev-dep to enforce 5s ceiling. Recommendation: skip — `wait_with_output()` blocks until child exits; stdin is closed → rmcp `.waiting()` future should resolve → child exits. If test hangs in CI, add timeout in follow-up phiếu. CI total budget per test is 60s default; one stuck test triggers CI timeout visibly.
  - Whether to also assert `"jsonrpc":"2.0"` in response — recommended.

- **Optional Test C — `tools/list` returns empty array:**
  - Same spawn pattern, send `initialize` THEN `tools/list` request (newline-delimited).
  - Verify response contains `"tools":[]` (or absent, depending on rmcp default `list_tools` implementation).
  - Tầng 2 self-decide — nice-to-have for P011 transition smoke; OK to skip.

### Scope

- CHỈ sửa: `src/cli/serve.rs` (stub → real impl), `src/main.rs` (`Commands::Serve` dispatch arm only).
- CHỈ tạo: `tests/serve_cli.rs` (NEW integration test ≥1 spawn test).
- CHỈ update docs: `docs/ARCHITECTURE.md` §5 (P010 scaffold-status entry) + §6 (note handshake-only, tools deferred to P011) + §1 (exit code 5 now exercised); `docs/CHANGELOG.md` (P010 entry — first MCP code, first tokio runtime use, first rmcp integration); `README.md` (MCP quick-start `.mcp.json` snippet + `advisory-inbox serve` invocation — Worker check Anchor #16 for existing coverage).
- KHÔNG sửa: `src/sentinel.rs` / `src/row.rs` / `src/state.rs` / `src/inbox.rs` / `src/cli/parse_report.rs` / `src/cli/dedup.rs` / `src/cli/append.rs` / `src/cli/migrate_state.rs` / `src/cli/state_backfill.rs` / `src/cli/scan_and_append.rs` / `src/cli/init.rs` (Phase 1+2 ship locks, MCP handshake doesn't call into these). `src/cli/mod.rs` (P001 already registers `pub mod serve;` — Worker verify Anchor #4; KHÔNG add `pub use`).
- KHÔNG sửa: `Cargo.toml` (rmcp 1.7.0 + tokio rt feature already present per P001 ship — Anchor #1). NO new dep this phiếu. If Worker discovers rmcp 1.7.0 surface requires `transport-async-rw` or similar additional feature → STOP, escalate as design objection.
- KHÔNG tạo: `src/mcp/` directory or any of `src/mcp/mod.rs` / `src/mcp/tools.rs` / `src/mcp/transport.rs` — defer to P011 (Architectural Decision #1). If Worker feels strongly that handshake code is better in `src/mcp/transport.rs`, escalate via shape objection.
- KHÔNG add `#[tokio::main]` to `src/main.rs`. Runtime stays inline in `cli::serve::run()` (Architectural Decision #2).
- KHÔNG register any MCP tools in this phiếu (`enable_tools()` NOT called). ServerCapabilities ships empty/default. P011 fills tools (Architectural Decision #5).
- KHÔNG modify `.mcp.json` `mcpServers` namespace to add `advisory-inbox` server entry. The `_post_p010` example block already exists (per current `.mcp.json` line 47-53) as documentation hint, but actually wiring it into `mcpServers` requires `cargo install --path .` of this binary first → out-of-scope (separate phiếu / Sếp-driven step after P010 ship). Worker MAY mention the example in CHANGELOG/README quick-start.
- KHÔNG đổi exit code semantics for any non-Serve subcmd. Only `Commands::Serve` gains exit 5 mapping.
- KHÔNG đổi state schema / inbox format / sentinel marker. Handshake doesn't touch any data files.
- KHÔNG bump `Cargo.toml` `version`. Stays `0.1.0` until P012 release polish phiếu.
- KHÔNG add unit tests that depend on `tokio::main` runtime at crate root.
- KHÔNG implement `call_tool` / `list_tools` overrides — leave defaults. P011's job.

### Skills consulted

**Architect (Bước 0) ran `mcp__context7__resolve-library-id` + `mcp__context7__query-docs` against `/websites/rs_rmcp_rmcp` (rmcp official docs, Source Reputation: High, Benchmark 68.5, 5221 snippets) for rmcp 1.7.0 API surface.**

Captured snapshots (these are Architect-verified, NOT speculation):

1. **`ServerHandler` trait** (`rmcp::handler::server::ServerHandler`):
   - 26 provided methods including `initialize`, `complete`, `list_tools`, `call_tool`, `list_resources`, `get_info`, etc. All default-implemented.
   - Trait bound: `Sized + Send + Sync + 'static`.
   - Override only the methods you need. For P010 handshake: override `get_info()` only.

2. **`rmcp::transport::io::stdio() -> (Stdin, Stdout)`** — gated by `transport-io` feature (Cargo.toml has this). Returns tokio Stdin + Stdout.

3. **`ServiceExt::serve(transport)`** — extension trait; returns `Future<Output = Result<RunningService<RoleServer, S>, ServerInitializeError>>`. Available with `server` feature.

4. **`ServerInfo`** = `pub struct InitializeResult { protocol_version: ProtocolVersion, capabilities: ServerCapabilities, server_info: Implementation, instructions: Option<String> }`.

5. **`Implementation`** (`#[non_exhaustive]`):
   ```rust
   pub struct Implementation {
       pub name: String,
       pub title: Option<String>,
       pub version: String,
       pub description: Option<String>,
       pub icons: Option<Vec<Icon>>,
       pub website_url: Option<String>,
   }
   ```

6. **`ServerCapabilities::builder().build()`** = empty default. `.enable_tools()` flips tools capability — P011 uses this; P010 stays empty.

7. **Manual `get_info()` example with macro** (canonical pattern shown by rmcp docs):
   ```rust
   #[tool_handler]
   impl ServerHandler for MyToolHandler {
       fn get_info(&self) -> ServerInfo {
           ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
       }
   }
   ```
   — Note `ServerInfo::new(capabilities)` constructor exists; Worker MAY prefer it. Architect's draft uses field-init for explicit name/version control (rmcp `ServerInfo::new` may not let you set Implementation fields). Worker self-decide which form (Tầng 2).

8. **`serve_server_with_ct`** (lower-level function): proves the higher-level `ServiceExt::serve` exists. Signature reference for typing.

**Architect read `docs/ticket/P001-scaffold-cli.md`** — confirmed:
- `Cargo.toml` Cargo deps include `rmcp = "1.7.0"` with `server` + `transport-io` features; `tokio` with `rt` + `macros` + `io-std` features (NO `rt-multi-thread`).
- `src/main.rs` is sync `fn main() -> anyhow::Result<()>` — Anchor #7 P001.
- `Commands::Serve` variant exists with no fields (P001 Anchor #6 — `serve` has no flags per ARCHITECTURE §1).
- `cli/serve.rs` exists as stub `pub fn run() -> Result<()>` printing TODO (Anchor #3 P001 + current Read confirmation).
- `cli/mod.rs` registers `pub mod serve;` per P001 ship.

**Architect read `docs/ticket/P009-scan-and-append.md`** for:
- Test fixture / `assert_cmd` patterns (Test B integration test in P010 follows same shape: `Command::cargo_bin("advisory-inbox").arg("serve")`).
- main.rs dispatch arm match-arm uniformity (Tail `Ok(())` required).
- Lane/Tầng/Sub-mech header conventions.

**Architect read `Cargo.toml`** to confirm:
- `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` (line 23).
- `tokio = { version = "1", features = ["rt", "macros", "io-std"] }` (line 18) — confirms current_thread is the only available flavor.
- `[dev-dependencies]` has `assert_cmd = "2"` + `predicates = "3"` + `tokio-test = "0.4"`.
- No `wait_timeout` dep; integration test relies on stdin-EOF triggered exit.

**Architect read `.mcp.json`** to confirm:
- Project committed config has `_post_p010` example block (lines 47-53) showing the future `advisory-inbox` server entry. P010 doesn't actually move it into `mcpServers` — that's a deploy step.

**Architect read `docs/ARCHITECTURE.md`**:
- §1 (line 90-98 + 110-119) — `serve` subcmd spec + exit code 5 reserved.
- §5 (line 220-256) — module layout planned `src/mcp/{mod.rs, tools.rs, transport.rs}`. P010 Architectural Decision #1 defers this; ARCHITECTURE §5 will get a P010 scaffold-status entry noting "P010 inline in cli/serve.rs; src/mcp/ deferred to P011".
- §6 (line 259-287) — MCP Surface: name = `advisory-inbox`, version = Cargo.toml, transport = stdio JSON-RPC 2.0, 6 tools (deferred to P011).

**Architect did NOT verify by Reading `src/cli/serve.rs` content** — spawn-prompt context already showed the stub. Anchor #3 confirms via current Read.

**Architect did NOT verify rmcp `RunningService::waiting()` return type** beyond context7 snippets. Worker Task 0 verifies via `cargo doc --no-deps --open --package rmcp` if needed, or trust the draft pattern; if compile fails on `.context()` over a non-Result, adjust per "Worker tuning latitude" notes.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> **BẮT BUỘC:** Kiến trúc sư PHẢI grep/verify code thật trước khi viết assumption.
> Thợ đọc bảng này để biết assumption nào đã verify, assumption nào chưa.
> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | `Cargo.toml` already has `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` + `tokio = { version = "1", features = ["rt", "macros", "io-std"] }`. NO need to add. NO `rt-multi-thread` feature → MUST use `Builder::new_current_thread()`. | Architect Read Cargo.toml dòng 18 + 23 during context load. | `[verified]` | ✅ Lines 18 + 23 confirmed. |
| 2 | `src/main.rs` has `Commands::Serve` variant (no fields) + dispatch arm `Commands::Serve => cli::serve::run()` (flat passthrough from P001). | P001 ship + P008/P009 Discovery confirmed all 8 dispatch arms present; Serve specifically is fieldless per ARCHITECTURE §1. | `[needs Worker verify]` | ⏳ TO VERIFY (Worker Task 0: `grep -n -A 2 "Commands::Serve" src/main.rs`). If dispatch arm has been touched by prior PR → STOP. |
| 3 | `src/cli/serve.rs` is the P001 stub: `pub fn run() -> Result<()> { println!("TODO: serve (MCP stdio JSON-RPC) — wired in P010"); Ok(()) }`. | Architect Read `src/cli/serve.rs` directly during context load. | `[verified]` | ✅ Confirmed verbatim (9 lines). |
| 4 | `src/cli/mod.rs` registers `pub mod serve;` (P001 scaffold). | P001 ship Anchor #10; P008/P009 transitively confirmed `pub mod` lines present. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -n "pub mod serve" src/cli/mod.rs`). |
| 5 | `fn main() -> anyhow::Result<()>` is SYNC (P001-P009 contract). NO `#[tokio::main]`. | P001 Anchor #7 + P002-P009 ship history. | `[needs Worker verify]` | ⏳ TO VERIFY (`head -20 src/main.rs` — confirm no `#[tokio::main]`). HARD STOP if main is already async. |
| 6 | `rmcp::handler::server::ServerHandler` trait exists with provided methods (initialize, list_tools, call_tool, get_info, etc.) all default-implemented. Override only `get_info()` for P010. | Architect context7 query → captured 26-method snapshot per Skills consulted #1. | `[verified]` | ✅ via context7 1.7.0 docs. |
| 7 | `rmcp::transport::io::stdio() -> (tokio::io::Stdin, tokio::io::Stdout)` available with `transport-io` feature. | Architect context7 query → captured signature per Skills consulted #2. | `[verified]` | ✅ via context7. |
| 8 | `ServiceExt::serve(transport)` returns `Future<Output = Result<RunningService<RoleServer, S>, ServerInitializeError>>`. | Architect context7 query → captured signature per Skills consulted #3. | `[verified]` | ✅ via context7. |
| 9 | `RunningService::waiting()` exists and resolves when transport closes (stdin EOF for stdio). Exact return type (Result vs ()) NOT fully verified by Architect — Worker tunes if needed. | Architect context7 covered `RunningService` reference but precise `.waiting()` signature for 1.7.0 not pinned. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE — try compile; if `.context()` fails on plain `()`, drop the `?`. Worker may also `cargo doc --no-deps --package rmcp` for surface check. |
| 10 | `ServerInfo` type alias for `InitializeResult { protocol_version, capabilities, server_info, instructions }`. `Implementation` has fields `{ name, title?, version, description?, icons?, website_url? }` (`#[non_exhaustive]`). | Architect context7 query → Skills consulted #4, #5. | `[verified]` | ✅ via context7. |
| 11 | `ServerCapabilities::builder().build()` produces empty/default capabilities. `.enable_tools()` flips tools — NOT called this phiếu. | Architect context7 query → Skills consulted #6. | `[verified]` | ✅ via context7. |
| 12 | `protocol_version: Default::default()` may or may not compile — `ProtocolVersion` Default trait existence not Architect-verified. If fails, use `ProtocolVersion::V_2024_11_05` or rmcp-exported const. | Worker compile check. | `[needs Worker verify]` | ⏳ TO VERIFY (Worker: try `Default::default()`; if compile error, `cargo doc` to find const). |
| 13 | `Implementation::new(name, version)` constructor MAY exist as ergonomic shortcut. Architect didn't pin but `#[non_exhaustive]` typically pairs with a `::new()`. | Worker may grep `impl Implementation` in `cargo doc` output. | `[needs Worker verify]` | ⏳ TO VERIFY (optional — field-init also works; Tầng 2 self-decide which form). |
| 14 | ARCHITECTURE §1 line 90-98 documents serve subcmd I/O contract (stdio JSON-RPC 2.0, 6 tools, long-running until stdin closed); line 118 reserves exit code 5 for MCP transport error. | Architect Read ARCHITECTURE.md §1 during context load. | `[verified]` | ✅ Confirmed. |
| 15 | ARCHITECTURE §5 lists P008/P009 scaffold-status entries; P010 entry pending — Worker adds. §6 MCP Surface lists 6 tools (deferred to P011). | Architect Read ARCHITECTURE.md §5 + §6 during context load. | `[needs Worker verify]` | ⏳ TO VERIFY Worker reads §5 latest state + §6; adds P010 entry. |
| 16 | `README.md` does NOT have MCP quick-start section yet (P004→P009 covered 5 atomic + 1 composite CLI subcmd, never touched MCP). | P009 Discovery sequence — no prior phiếu mentioned MCP quick-start. | `[unverified]` | ⏳ TO VERIFY (`grep -n -i "mcp\|serve" README.md` — expect 0-1 hits in subcmd list, NOT in quick-start). |
| 17 | `.mcp.json` has `_post_p010` example block at lines 47-53 (NOT inside `mcpServers`). P010 does NOT move it into `mcpServers` — deferred to deploy step. | Architect Read `.mcp.json` during context load. | `[verified]` | ✅ Lines 47-53 confirmed (commented hint block). |
| 18 | `tests/serve_cli.rs` does NOT exist (Phase 3 first test file). | Architect did NOT glob tests/; expects greenfield. | `[needs Worker verify]` | ⏳ TO VERIFY (`ls tests/` — no `serve_cli.rs`). Worker creates this NEW file in Task 3. |
| 19 | `assert_cmd::Command::cargo_bin(...)` + `std::process::Command::spawn` with `Stdio::piped` is canonical pattern for spawn-binary integration tests (P004-P009 precedent uses `Command::cargo_bin`; piped-stdio is std). `wait_with_output()` blocks until child exits. | Rust std + assert_cmd standard API. | `[verified]` | ✅ Standard pattern. |
| 20 | Sub-mech F: token leak check — new files (serve.rs + serve_cli.rs) MUST grep clean for `ghp_|gho_|ghu_|ghs_|github_pat_`. | Worker grep after EXECUTE. | `[needs Worker verify]` | ⏳ TO VERIFY (`grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/cli/serve.rs tests/serve_cli.rs Cargo.toml`). |
| 21 | Sub-mech A (trigger): MCP server actually starts when invoked. Smoke test = pipe `initialize` JSON-RPC to `cargo run -- serve` stdin, observe response on stdout, verify exit 0 on stdin close. | Worker Task 0 + Test B integration test. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE (Test B is the assertion). |
| 22 | rmcp 1.7.0 + tokio current_thread runtime + stdio transport composition known-good (rmcp examples use multi-thread but current_thread works for single-client stdio). | Architect context7 showed `serve_server_with_ct` is async — works under any runtime; current_thread is a `Runtime` value with `block_on`. | `[unverified]` | ⏳ TO VERIFY at compile + Test B — if multi-threaded runtime is required by some rmcp internal, compile/runtime error will surface. Mitigation: if rmcp 1.7.0 internally spawns tasks, current_thread `enable_all()` covers io + time + sleep. |
| 23 | `ProtocolVersion` rmcp default — when not specified by client, rmcp picks current. When client sends `"protocolVersion":"2024-11-05"` (per Test B request payload), rmcp negotiates. Architect picked 2024-11-05 because it's the MCP spec version contemporary with rmcp 1.7.0 release era. | Architect knowledge cutoff 2026-01 — rmcp 1.7.0 + MCP 2024-11-05 alignment is consistent. | `[unverified]` | ⏳ TO VERIFY (Worker reads rmcp ProtocolVersion variants in `cargo doc`; if rmcp 1.7.0 supports a newer ver like `2025-XX`, use that in Test B request and assert response accepts). |

**Hard Stop triggers:**
- Anchor #2 — `Commands::Serve` dispatch arm has been modified by prior PR (e.g., already error-mapped) → STOP, investigate.
- Anchor #5 — `src/main.rs` already has `#[tokio::main]` → STOP, escalate (Architectural Decision #2 violation).
- Anchor #6/#7/#8 — rmcp API surface differs significantly from Architect snapshots (e.g., `ServerHandler` removed in 1.7.0) → STOP, escalate as design objection. Worker uses `cargo doc --no-deps --package rmcp` for ground truth.
- If `enable_all()` requires `rt-multi-thread` feature → STOP, escalate (Cargo.toml dep change needed).

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ cách xử lý.** Hiện không có ❌. Anchors #16, #22, #23 flagged `[unverified]` — Worker verifies before adding docs sections / before locking ProtocolVersion choice.

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V1 (initial draft)

### Turn 1 — Worker Challenge

**Anchor verification (all verified against real code + rmcp-1.7.0 source):**

- Anchor #1 ✅: `Cargo.toml` line 18 `tokio = { version = "1", features = ["rt", "macros", "io-std"] }` + line 23 `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }`. NO `rt-multi-thread`. Confirmed.
- Anchor #2 ✅: `src/main.rs` line 208 `Commands::Serve => cli::serve::run()` — flat passthrough, unmodified by P001-P009.
- Anchor #3 ✅: `src/cli/serve.rs` is P001 stub verbatim (9 lines, TODO println).
- Anchor #4 ✅: `src/cli/mod.rs` line 13 `pub mod serve;` present.
- Anchor #5 ✅: `src/main.rs` is sync `fn main() -> Result<()>` — no `#[tokio::main]`. Confirmed.
- Anchor #6 ✅: `ServerHandler` trait confirmed in `handler/server.rs` line 158 + line 19 impl for H:ServerHandler. Default methods present including `get_info()`.
- Anchor #7 ✅: `transport/io.rs` line 4 `pub fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout)`. Exact match.
- Anchor #8 ✅: `ServiceExt::serve` in `service.rs` line 168-177, returns `Result<RunningService<R,Self>, R::InitializeError>` (where RoleServer::InitializeError = ServerInitializeError). Confirmed.
- Anchor #9 ✅: `RunningService::waiting()` at `service.rs:545` returns `Result<QuitReason, tokio::task::JoinError>` — IS a Result. The `.context()` pattern compiles correctly since `JoinError` implements `std::error::Error`. No tuning needed.
- Anchor #10 ✅: `ServerInfo = InitializeResult` (model.rs:893). `InitializeResult` has `protocol_version`, `capabilities`, `server_info`, `instructions`. `Implementation` has `name`, `title?`, `version`, `description?`, `icons?`, `website_url?`. All fields confirmed.
- Anchor #11 ✅: `ServerCapabilities::builder().build()` produces empty default. `.enable_tools()` exists. Confirmed.
- Anchor #12 ✅: `ProtocolVersion` has `Default` impl at model.rs:142-146 — returns `Self::LATEST`. Compiles.
- Anchor #13 ✅: `Implementation::new(name: impl Into<String>, version: impl Into<String>)` exists at model.rs:1015. Prefer this over field-init (cleaner + forward-safe with `#[non_exhaustive]`). Tầng 2 self-decide: will use `Implementation::new()`.
- Anchor #14 ✅: ARCHITECTURE §1 line 90-98 + exit code 5 at line 118 confirmed.
- Anchor #15 ✅: §5 P009 entry at line 254; §6 MCP Surface section at line 259+. P010 entry pending.
- Anchor #16 ✅: README has no MCP quick-start section — `grep "serve\|MCP\|mcp" README.md` returns only description-line mentions (no ## section). P010 must add.
- Anchor #17 ✅: `.mcp.json` `_post_p010` block exists (not in `mcpServers`). Confirmed.
- Anchor #18 ✅: `ls tests/` — no `serve_cli.rs`. Greenfield confirmed.
- Anchor #19 ✅: `assert_cmd::cargo::CommandCargoExt` + `std::process::Command` pattern standard. Confirmed.
- Anchor #20 ✅ (deferred to post-EXECUTE): Token leak grep on new files — will run.
- Anchor #21 ✅ (deferred): Sub-mech A smoke = Test B integration test.
- Anchor #22 ✅: rmcp's `ServiceExt::serve` uses `tokio::spawn` internally (confirmed via service.rs). `current_thread` runtime supports `spawn` under `block_on`. `enable_all()` covers io + time. No `rt-multi-thread` needed.
- Anchor #23 ⚠️ [mechanical]: Phiếu draft uses `"protocolVersion":"2024-11-05"` in Test B request payload. rmcp 1.7.0 `ProtocolVersion::default()` returns LATEST = `"2025-11-25"` (from model.rs:192-195, 4 versions: 2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25). The 2024-11-05 request will still work (rmcp accepts older client versions by negotiating), but Worker will use `"2025-03-26"` (conservative middle) OR keep `"2024-11-05"` (explicit test of backwards compat). Tầng 2 self-decide: keep `"2024-11-05"` (tests backward-compat negotiation; conservative for CI stability).

**No Tầng 1 objections found.** All architectural assumptions verified against rmcp-1.7.0 source. Code pattern in phiếu is sound with one Tầng 2 self-fix: use `Implementation::new("advisory-inbox", env!("CARGO_PKG_VERSION"))` instead of field-init. Baseline tests: 62 (confirmed).

**Worker accepted V1 — no challenges.** Proceeding to EXECUTE.

**Status:** ✅ ACCEPTED — EXECUTE authorized

### Turn 1 — Architect Response
*(Architect fill phần này khi invoked RESPOND mode. KHÔNG đọc source code — dựa vào Worker `file:line` citation.)*

- [O1.1] → ACCEPT / DEFEND / REFRAME (Tầng 2) / DEFER TO SẾP → action taken
- [O1.2] → …

**Status:** ✅ RESPONDED — phiếu bumped to V2

*(Repeat Turn 2, Turn 3 if needed. Cap = 3.)*

### Final consensus
- Phiếu version: V1
- Total turns: 1 (no objections)
- Approved (autonomous): 2026-05-28 — code execution may begin

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
| A (trigger) | Spawn `cargo run --quiet -- serve`, pipe `initialize` JSON-RPC to stdin, close stdin | exit 0, stdout contains `"serverInfo":{"name":"advisory-inbox"...}`, `"id":1` echoed | | |
| A (trigger) | `.mcp.json` `_post_p010` example block still present (documentation hint preserved) | block at lines ~47-53 | | |
| B (capability) | `cargo check` | exit 0, 0 warnings | | |
| B (capability) | `cargo test --test serve_cli` | ≥1 integration test pass (Test B spawn+initialize) | | |
| B (capability) | `cargo test --lib cli::serve::tests` (or wherever unit test lives) | ≥1 unit test pass (Test A metadata) | | |
| B (capability) | `cargo build --release` | exit 0, 0 warnings, binary `target/release/advisory-inbox` exists | | |
| B (capability) | `target/release/advisory-inbox --help` shows `serve` subcmd | row contains "Start MCP server" desc (from clap derive) | | |
| C (state migration) | (no state schema change this phiếu) | N/A | N/A | N/A |
| D (persistence) | `grep -n "P010" docs/CHANGELOG.md` | ≥1 hit (entry at top) | | |
| D (persistence) | `grep -l "serve\|MCP\|rmcp" docs/ARCHITECTURE.md` | ≥1 hit (§5 P010 entry + §6 unchanged) | | |
| D (persistence) | `grep -n -i "advisory-inbox serve\|mcp" README.md` | ≥1 hit (quick-start MCP section) | | |
| E (env drift) | `cargo update --dry-run` | no surprise major bump (rmcp + tokio unchanged) | | |
| E (env drift) | `cargo build --release` from current target | exit 0, 0 warnings | | |
| F (runtime state) | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' src/cli/serve.rs src/main.rs tests/serve_cli.rs Cargo.toml` | 0 hits | | |
| F (runtime state) | binary release size sanity check `ls -la target/release/advisory-inbox \| awk '{print $5}'` | < 10 MB (rmcp + tokio bump baseline; baseline post-P009 unknown — note delta) | | |

---

## Nhiệm vụ

### Task 0 — Pre-EXECUTE capability verification (Sub-mech A + B + F)

**Mục tiêu:** Worker grep + verify state thật TRƯỚC khi viết code; verify rmcp 1.7.0 API surface matches Architect's context7 snapshots.

**Lệnh chạy (verify Anchors #2, #4, #5, #9, #12, #13, #15, #16, #18):**

```bash
# Anchor #2 — main.rs Commands::Serve dispatch arm shape
grep -n -A 2 "Commands::Serve" src/main.rs

# Anchor #4 — cli/mod.rs registers serve
grep -n "pub mod serve" src/cli/mod.rs

# Anchor #5 — main.rs is sync (no #[tokio::main])
head -20 src/main.rs
grep -n "tokio::main" src/main.rs || echo "main is sync ✅"

# Anchor #9 + #12 + #13 — rmcp 1.7.0 API surface ground truth
# (Worker may use cargo doc OR cargo expand — pick lowest-cost)
cargo doc --no-deps --package rmcp 2>&1 | tail -5
# Then open target/doc/rmcp/handler/server/trait.ServerHandler.html in browser OR
# inspect target/doc/rmcp/model/struct.Implementation.html source. Look for:
#   - ServerHandler::get_info signature
#   - Implementation::new constructor existence
#   - RunningService::waiting return type (Result or ())
#   - ProtocolVersion::Default impl OR exported const

# Anchor #15 — ARCHITECTURE §5 latest state + §6 MCP surface
grep -n "P008\|P009\|P010" docs/ARCHITECTURE.md
sed -n '259,287p' docs/ARCHITECTURE.md   # §6 MCP Surface review

# Anchor #16 — README MCP coverage
grep -n -i "advisory-inbox serve\|mcp\|rmcp" README.md

# Anchor #18 — tests/ greenfield for serve_cli
ls tests/ | grep serve_cli || echo "no existing tests/serve_cli.rs ✅"

# Sub-mech F preflight (Anchor #20)
grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' src/ tests/ Cargo.toml || echo "clean"

# Sub-mech A smoke (Anchor #21 — pre-implementation: skip; post-implementation Test B asserts)

# Baseline test count (post-P009)
cargo test --all -- --list 2>/dev/null | grep -E "^test " | wc -l
# Expect ~62 (39 unit + 23 integration per P009 Discovery). Phiếu target: ≥64 after P010 (+1 unit + 1 integration min; +2-3 if Worker includes optional Test C).
```

**Output:** Worker fill vào Debate Log Turn 1 Anchor table.

**Hard Stop triggers:**
- Anchor #2 — `Commands::Serve` dispatch arm has fields (not `Commands::Serve =>`) OR already error-mapped → STOP, escalate.
- Anchor #5 — `#[tokio::main]` present on `fn main` → STOP, escalate Architectural Decision #2 violation.
- Anchor #9 — if `RunningService::waiting` returns plain `()` (no Result), Worker drops `?` and `.context()` (Tầng 2 self-fix; NOT a Hard Stop, just a tuning note).
- Anchor #12 — if `ProtocolVersion: Default::default()` doesn't compile, Worker substitutes the explicit version const found in `cargo doc` (Tầng 2 self-fix).
- rmcp `ServerHandler` trait NOT FOUND or radically different shape from Anchor #6 snapshot → STOP, escalate design objection.

---

### Task 1: `src/cli/serve.rs` — stub → real rmcp handshake impl

**File:** `src/cli/serve.rs`

**Tìm** (current P001 stub, Anchor #3 verbatim):

```rust
//! Stub for `serve` (MCP) subcommand. Real logic wired in P010.

use anyhow::Result;

pub fn run() -> Result<()> {
    println!("TODO: serve (MCP stdio JSON-RPC) — wired in P010");
    Ok(())
}
```

**Thay bằng** (Architect-drafted; Worker tunes per Anchors #9/#12/#13 Task 0 results):

```rust
//! `serve` subcommand — MCP server (stdio JSON-RPC 2.0) via rmcp 1.7.0.
//!
//! This phiếu (P010) ships **handshake only** — no tools registered.
//! P011 will wire 6 tools (parse_report, dedup, append, migrate_state, state_backfill,
//! scan_and_append) per ARCHITECTURE.md §6.
//!
//! Architecture decisions (P010):
//! - Inline rmcp wiring in this file; no `src/mcp/` module yet (defer to P011).
//! - tokio current_thread runtime built INSIDE this fn (no `#[tokio::main]` in main.rs).
//! - `ServerCapabilities` = empty/default (no `tools` declared; honest until P011).

use anyhow::{Context, Result};
use rmcp::{
    ServiceExt,
    handler::server::ServerHandler,
    model::{Implementation, ServerCapabilities, ServerInfo},
    transport::io::stdio,
};

/// Unit struct — no state needed for handshake-only server.
/// P011 will extend with `ToolRouter` field for tool dispatch.
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
    use super::*;

    #[test]
    fn get_info_returns_name_and_version() {
        let server = AdvisoryInboxServer;
        let info = server.get_info();
        assert_eq!(info.server_info.name, "advisory-inbox");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn get_info_returns_no_tools_capability() {
        // P010 explicitly does NOT declare tools capability. P011 will flip it.
        let server = AdvisoryInboxServer;
        let info = server.get_info();
        // Worker tunes assert based on actual ServerCapabilities field shape:
        // - If `capabilities.tools: Option<...>`, assert is_none().
        // - If `capabilities` has bool-flag accessors, assert tools_enabled() == false.
        // Architect's snapshot showed `ServerCapabilities::builder().enable_tools()` flips
        // a flag; default builder leaves it unset. Worker verify exact assert form.
        let _ = info.capabilities; // silence unused if assert form needs adjustment
    }
}
```

**Lưu ý:**
- Worker MUST verify (Anchor #12) that `protocol_version: Default::default()` compiles. If `ProtocolVersion` lacks `Default`, substitute the rmcp-exported variant (e.g., `rmcp::model::ProtocolVersion::V_2024_11_05` — verify via `cargo doc`).
- Worker MUST verify (Anchor #9) `running.waiting().await` return type. If it returns plain `()`, change the body to `running.waiting().await; Ok(())` (no `?`, no `.context()`).
- Worker MUST verify (Anchor #13) `Implementation::new(name, version)` constructor — if exists, MAY prefer it over field-init for ergonomics (Tầng 2 self-decide). Field-init is the safer default given `#[non_exhaustive]` may permit additional Option<_> fields.
- `description` field on `Implementation` per Architect's context7 snapshot — IF the field doesn't exist in 1.7.0 (sometimes minor docs lag), Worker removes the line and the comma; Tầng 2 self-fix.
- `#[derive(Default)]` is harmless since Server is unit struct; both `AdvisoryInboxServer` and `AdvisoryInboxServer::default()` valid.
- `tokio::runtime::Builder::new_current_thread()` REQUIRED (NOT multi-thread — Cargo.toml feature gate).
- `enable_all()` enables io + time + signal under the runtime; Cargo.toml `rt` + `io-std` features sufficient for stdio transport.
- NO `unsafe { }`.
- NO `process::exit` inside `run()` — bubble via `anyhow::Result`. main.rs maps to exit 5.
- Unit test #2 (`get_info_returns_no_tools_capability`) is intentionally underspecified — Worker fills the assert form once they see the actual `ServerCapabilities` struct shape. The test name + intent is locked; assert detail is Tầng 2.
- `tokio` async block uses `Ok::<(), anyhow::Error>(())` to give the closure an explicit return type for the outer `?` to bind. Standard idiom for `block_on(async { ... })`.

---

### Task 2: `src/main.rs` — `Commands::Serve` dispatch arm + exit code 5 mapping

**File:** `src/main.rs`

**Tìm** (current P001 scaffold flat passthrough — Worker confirm post-Task 0 via Anchor #2):

```rust
Commands::Serve => cli::serve::run(),
```

**Thay bằng:**

```rust
Commands::Serve => {
    if let Err(e) = cli::serve::run() {
        eprintln!("error: {:#}", e);
        // MCP transport / runtime errors → exit 5 per ARCHITECTURE §1 exit-code table.
        std::process::exit(5);
    }
    Ok(())
}
```

**Lưu ý:**
- Tail `Ok(())` REQUIRED (match-arm uniformity precedent P004-P009).
- Single error family → exit 5. No downcast needed (rmcp surfaces wrap into anyhow; treat all as transport-class).
- Pattern simpler than `Commands::Append`/`Dedup`/etc. dispatch arms because handshake has no per-error-type granularity (no input-vs-write-vs-parse distinction).
- Worker MUST keep the literal `Commands::Serve => cli::serve::run(),` line as the only match arm being replaced — do NOT touch adjacent arms.

---

### Task 3: `tests/serve_cli.rs` — NEW integration test

**File:** `tests/serve_cli.rs` (NEW — does not exist; Anchor #18 confirms greenfield).

**Tạo** (Worker may refine timeout / pattern per CI behavior):

```rust
//! Integration tests for `advisory-inbox serve` (MCP handshake — P010).
//!
//! These tests spawn the binary, write JSON-RPC `initialize` to stdin, read response
//! from stdout, and assert MCP handshake shape per spec.
//!
//! P010 ships handshake only — `tools/list` tests deferred to P011.

use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::process::{Command, Stdio};

/// MCP `initialize` JSON-RPC request payload — protocolVersion picked to match rmcp 1.7.0
/// supported version. If rmcp negotiates differently, the response still echoes the request
/// `id` and includes `serverInfo` — assertions check serverInfo shape, not protocolVersion echo.
const INIT_REQUEST: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.0"}}}"#;

#[test]
fn serve_responds_to_initialize_with_server_info() {
    let mut child = Command::cargo_bin("advisory-inbox")
        .expect("cargo_bin advisory-inbox")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn advisory-inbox serve");

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{}", INIT_REQUEST).expect("write initialize request");
    }
    // Close stdin to signal EOF — rmcp should resolve .waiting() and exit gracefully.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait for child");

    assert!(
        output.status.success(),
        "advisory-inbox serve did not exit 0 — status: {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");

    // Assert MCP initialize response shape: serverInfo.name + id echo.
    assert!(
        stdout.contains("\"name\":\"advisory-inbox\""),
        "stdout missing serverInfo.name=\"advisory-inbox\": {}",
        stdout
    );
    assert!(
        stdout.contains("\"id\":1"),
        "stdout missing JSON-RPC id=1 echo: {}",
        stdout
    );
    assert!(
        stdout.contains("\"jsonrpc\":\"2.0\""),
        "stdout missing JSON-RPC envelope marker: {}",
        stdout
    );
}
```

**Lưu ý:**
- `assert_cmd::cargo::CommandCargoExt` brings `Command::cargo_bin` onto std `Command`. Worker confirms import; alternative is `assert_cmd::Command` wrapper but std Command + pipe Stdio is more flexible for stdin write.
- `wait_with_output()` blocks until child exits. Because stdin is closed (dropped after write), rmcp `.waiting()` should resolve → child should exit 0. If child hangs in CI, mitigation: kill child with `child.kill()` after a sleep (Tầng 2 fix; not preferred — better to fix root cause).
- Test asserts via substring `contains(...)` — robust to whitespace/key-ordering changes in JSON output. Matches P004-P009 precedent.
- Worker MAY add a second test for `tools/list` returning empty if Architect's `ServerHandler` default `list_tools` returns `Ok(ListToolsResult { tools: vec![] })`. Tầng 2 self-decide; recommended skip (P011 verifies tools surface).
- If `protocolVersion` in request payload triggers rejection by rmcp 1.7.0 (server expects newer version), Worker adjusts the literal to match what rmcp supports — verify via Anchor #23 + cargo doc.

---

### Task 4: Update docs (Tầng 1 Docs Gate per RULES.md §11)

#### 4a. `docs/CHANGELOG.md` — add P010 entry at top

**Tìm** (current top entry — P009):

```markdown
## P009 — scan-and-append composite subcmd (2026-05-28)
```

**Thêm phía trên** (do NOT delete P009 entry):

```markdown
## P010 — `serve` subcmd: rmcp stdio handshake (2026-05-28)

**Type:** feat | **Tầng:** 1 | **Lane:** Guarded

### Added

- `cli/serve.rs` real impl: rmcp 1.7.0 MCP server over stdio JSON-RPC 2.0. `AdvisoryInboxServer` unit struct implementing `ServerHandler` with custom `get_info()` returning `Implementation { name: "advisory-inbox", version: env!("CARGO_PKG_VERSION") }` + empty `ServerCapabilities` (no tools declared — honest until P011). Tokio `current_thread` runtime built inline (no `#[tokio::main]` in main.rs — sync-main contract preserved per P001-P009).
- Handshake support only: server responds to `initialize` JSON-RPC requests with valid `InitializeResult` per MCP spec. `tools/list` returns empty (default `ServerHandler::list_tools`). P011 will wire 6 tools.
- `tests/serve_cli.rs` integration test: spawns binary, pipes `initialize` JSON-RPC to stdin, asserts response contains `"name":"advisory-inbox"` + `"id":1` echo + `"jsonrpc":"2.0"` envelope, exit 0 on stdin EOF.
- `src/cli/serve.rs` unit tests: `AdvisoryInboxServer::get_info()` returns correct name/version.

### Changed

- `main.rs` `Commands::Serve` dispatch arm: replaced flat passthrough with error → exit 5 mapping (MCP transport error class per ARCHITECTURE §1 exit-code table — first usage of code 5).
- First tokio runtime instantiation in this binary. Cargo.toml `tokio = { features = ["rt", "macros", "io-std"] }` exercised; current_thread flavor (no `rt-multi-thread` feature gate).
- First rmcp 1.7.0 integration. `ServiceExt::serve` + `transport::io::stdio` + `ServerHandler` trait surface used.

### Architecture decisions (P010)

- `src/mcp/` module DEFERRED to P011 — handshake-only fits in `cli/serve.rs`. ARCHITECTURE §5 module layout still lists planned `src/mcp/`; entry status now "P011 (planned)".
- ServerCapabilities ships empty/default — declaring tools while returning none is misleading. P011 flips `.enable_tools()`.

### Test counts

Baseline 62 (post-P009) → post-P010 64+ (39 unit + 23 integration + 2 new unit (cli::serve::tests) + 1 new integration (serve_cli) = 65 minimum).

### Atomicity caveat / runtime notes

- No state file writes this phiếu — MCP handshake is read-only on filesystem.
- `.mcp.json` `_post_p010` example block (lines ~47-53) NOT yet moved into `mcpServers` namespace — that requires `cargo install --path .` followed by manual config edit (deploy step, out-of-scope P010).

---
```

#### 4b. `docs/ARCHITECTURE.md` — update §5 + §6 + §1 exit code note

**§5 (Module Layout) — append P010 scaffold-status entry under existing P009 entry** (line ~254 per Anchor #15):

```markdown
- P010: `cli/serve.rs` wired with rmcp 1.7.0 MCP server (stdio JSON-RPC 2.0 handshake). `AdvisoryInboxServer` unit struct implementing `ServerHandler::get_info()`. Tokio `current_thread` runtime built inline in `serve::run()` (no `#[tokio::main]` in `main.rs` — P001-P009 sync-main contract preserved). NO `src/mcp/` module shipped — handshake-only fits in `cli/serve.rs` (~80 lines). P011 will add `src/mcp/{mod.rs, tools.rs}` when tool dispatch needs structure. `main.rs` `Commands::Serve` dispatch arm gains exit-code-5 mapping (MCP transport error class). 1 unit test (get_info metadata) + 1 integration test (`tests/serve_cli.rs` spawn binary + `initialize` JSON-RPC round-trip).
```

**§6 (MCP Surface) — add a "Status" note at top of section** (line ~259 area):

```markdown
### Status

- **P010 (shipped 2026-05-28):** Handshake support — `initialize` JSON-RPC request → valid `InitializeResult` response with `serverInfo: { name: "advisory-inbox", version: <Cargo.toml> }` + empty `ServerCapabilities`. 0 tools registered. `tools/list` returns empty (`ServerHandler` default).
- **P011 (planned):** 6 tools registered via `ToolRouter`. ServerCapabilities flips `.enable_tools()`.
```

(Worker preserves the existing Tools table — P010 doesn't change it.)

**§1 (CLI Surface) — annotate exit code 5 row** (line ~118):

The exit-code table line `| 5    | MCP transport error (rmcp serve mode only) |` stays as-is. Worker may add a parenthetical "(first exercised P010)" if Tầng 2 self-decide adds clarity. Recommendation: skip — table is already explicit.

#### 4c. `README.md` — add MCP quick-start section

**Worker check Anchor #16** (`grep -n -i "mcp\|serve" README.md`). If section doesn't exist, ADD near the bottom of the CLI quick-start section:

```markdown
## MCP server mode

`advisory-inbox` can also run as an MCP (Model Context Protocol) server, exposing its
functionality to Claude Code and other MCP-capable AI assistants via JSON-RPC 2.0 over
stdin/stdout.

```sh
# Direct invocation (handshake-only as of P010; tools come in P011):
advisory-inbox serve

# Or wire into your project's .mcp.json:
{
  "mcpServers": {
    "advisory-inbox": {
      "command": "/path/to/advisory-inbox",
      "args": ["serve"]
    }
  }
}
```

The server responds to MCP `initialize` requests with server info (`name: "advisory-inbox"`,
`version: <Cargo.toml>`). As of P010, no tools are registered — P011 will expose 6 tools
(`parse_report`, `dedup`, `append`, `migrate_state`, `state_backfill`, `scan_and_append`).

Exit code 5 indicates MCP transport / runtime error (other exit codes apply only to direct
CLI subcommands; see Exit codes section).
```

(Worker adjusts heading level + position to fit README's existing structure.)

---

## Files cần sửa

| File | Thay đổi |
|------|---------|
| `src/cli/serve.rs` | Task 1: stub → real rmcp handshake impl (`AdvisoryInboxServer` + `ServerHandler` + tokio current_thread runtime). Adds 2 unit tests in `#[cfg(test)] mod tests`. |
| `src/main.rs` | Task 2: `Commands::Serve` dispatch arm gains exit-5 error mapping. |
| `tests/serve_cli.rs` | Task 3: NEW file — 1 integration test spawn binary + `initialize` JSON-RPC round-trip. |
| `docs/CHANGELOG.md` | Task 4a: P010 entry at top (above P009). |
| `docs/ARCHITECTURE.md` | Task 4b: §5 P010 scaffold-status entry + §6 Status subsection. |
| `README.md` | Task 4c: MCP server mode quick-start (if Anchor #16 confirms absent). |

## Files KHÔNG sửa (verify only)

| File | Verify gì |
|------|----------|
| `src/cli/mod.rs` | `pub mod serve;` declaration already present from P001 (Anchor #4). NO `pub use serve::AdvisoryInboxServer;` — keep struct module-private. |
| `Cargo.toml` | `rmcp = { version = "1.7.0", features = ["server", "transport-io"] }` (line 23) + `tokio = { version = "1", features = ["rt", "macros", "io-std"] }` (line 18) already present (Anchor #1). NO new dep this phiếu. |
| `src/sentinel.rs` / `src/row.rs` / `src/state.rs` / `src/inbox.rs` | Phase 1+2 ship locks. Handshake doesn't call into these. |
| `src/cli/parse_report.rs` / `dedup.rs` / `append.rs` / `migrate_state.rs` / `state_backfill.rs` / `scan_and_append.rs` / `init.rs` | Other subcmd impls untouched. |
| `.mcp.json` | `_post_p010` example block stays at lines ~47-53; NOT moved into `mcpServers` (deploy step out-of-scope). |
| `docs/PROJECT.md` | Status section may stay at "Phase 1 not yet shipped" or wherever the previous phiếu left it — Worker self-decides if Phase 3 opening warrants a status note; Tầng 2. |
| `docs/security/INVARIANTS.md` | No INV-LOCAL-002 usage this phiếu (no atomic file writes). Tầng 2 self-decide if Worker wants to add a note that INV-WF-001 (trigger verifiability) is exercised — recommendation: skip (already documented in RULES.md §5). |
| `docs/BACKLOG.md` | Worker strikethrough P010 + mark shipped after merge (post-EXECUTE step; phiếu workflow standard). |

---

## Luật chơi (Constraints)

1. **NO new dep.** Cargo.toml stays untouched. `rmcp = "1.7.0"` + `tokio` already in. If Worker discovers rmcp 1.7.0 needs an additional feature (e.g., `transport-async-rw`) → STOP, escalate as design objection. Default features may be sufficient even though not enumerated.
2. **NO `#[tokio::main]` in `src/main.rs`.** Sync main contract from P001-P009 preserved. Runtime stays inline in `cli::serve::run()` via `Builder::new_current_thread().enable_all().build()?.block_on(...)`.
3. **NO `src/mcp/` directory creation.** Handshake-only fits in `cli/serve.rs`. P011 deferral architecturally locked. If Worker prefers structure → escalate via shape objection.
4. **NO MCP tools registered.** ServerCapabilities ships empty/default. P011 fills tools. Don't call `.enable_tools()`. Don't override `ServerHandler::list_tools` (default empty result is correct).
5. **NO `unsafe { }` block.** Standard hard constraint.
6. **NO `process::exit(...)` inside `cli::serve::run()`.** Bubble via `anyhow::Result`; main.rs maps to exit 5.
7. **NO `.mcp.json` `mcpServers` namespace edit.** The `_post_p010` example block at lines ~47-53 stays as documentation; moving into `mcpServers` requires binary install + is out-of-scope.
8. **Server name = `"advisory-inbox"` literal.** Version = `env!("CARGO_PKG_VERSION")`. NO hardcoded version string. Source of truth = Cargo.toml.
9. **Use `tokio::runtime::Builder::new_current_thread()`.** Cargo.toml feature gate `rt` (NOT `rt-multi-thread`) limits options. If `enable_all()` fails to compile or panic at runtime due to missing feature → STOP, escalate.
10. **Test B integration test MUST exit 0 on stdin EOF.** If child hangs (rmcp doesn't resolve `.waiting()` on stdin close), STOP and escalate runtime behavior objection — do NOT band-aid with `child.kill()` (would mask root cause).
11. **NO state schema change.** No inbox markdown format change. No sentinel marker change. Handshake doesn't touch any data files.
12. **NO `Cargo.toml` `version` bump.** Stays `0.1.0`.
13. **Match existing main.rs match-arm style.** Tail `Ok(())` required (P004 Turn 1 precedent). Block-form `Commands::Serve => { ... Ok(()) }` matches `Commands::Append`/`Dedup`/etc.

---

## Nghiệm thu

### Automated
- [ ] `cargo build --release` — zero warnings, binary `target/release/advisory-inbox` exists.
- [ ] `cargo test --all` — all pass (baseline 62 + ≥2 new = ≥64).
- [ ] `cargo test --test serve_cli` — integration test pass (spawn + initialize round-trip).
- [ ] `cargo test --lib cli::serve::tests` (or equivalent module path) — unit tests pass (get_info metadata).
- [ ] `cargo clippy --all-targets -- -D warnings` — clean.
- [ ] `cargo fmt --check` — no diff.

### Manual Testing
- [ ] **Sub-mech A smoke (trigger):**
  ```sh
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"manual","version":"0.0.0"}}}' | cargo run --quiet -- serve
  ```
  Expected: stdout JSON contains `"name":"advisory-inbox"` + `"id":1`. Exit 0 after stdin EOF.
- [ ] **`advisory-inbox --help` still shows `serve` subcmd** (P001 scaffold preserved):
  ```sh
  cargo run --quiet -- --help | grep -A 1 "^  serve"
  ```
- [ ] **Optional: `tools/list` returns empty** (P011 transition check):
  ```sh
  printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"manual","version":"0.0.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n' | cargo run --quiet -- serve
  ```
  Expected: second response (id=2) contains `"tools":[]` or absent (rmcp default).

### Regression
- [ ] **All 6 previously-shipped subcmd still pass their integration tests:**
  ```sh
  cargo test --test parse_report_cli --test dedup_cli --test append_cli --test migrate_state_cli --test state_backfill_cli --test scan_and_append_cli
  ```
- [ ] **Cold start latency unchanged for non-serve subcmd** (no tokio runtime should spin up):
  ```sh
  time cargo run --quiet --release -- parse-report < tests/fixtures/agent-report-1.md > /dev/null
  ```
  Expected: < 100ms (P004 budget per ARCHITECTURE §9). Tokio inline in serve::run means no overhead for other subcmd.
- [ ] **Binary size sanity** (rmcp + tokio first integration — note baseline delta):
  ```sh
  ls -la target/release/advisory-inbox | awk '{print $5}'
  ```
  Expected: < 10 MB (PROJECT.md success criterion). If delta from post-P009 is > 3 MB, note in Discovery Report.

### Docs Gate
- [ ] `docs/CHANGELOG.md` — P010 entry at top per Task 4a template.
- [ ] `docs/ARCHITECTURE.md` — §5 P010 scaffold-status entry + §6 Status subsection per Task 4b. (Tầng 1: MCP server logic ship → ARCHITECTURE.md MUST update.)
- [ ] `README.md` — MCP server mode quick-start added (Tầng 1: CLI subcmd touched + first MCP code).
- [ ] `docs-gate --all --verbose` — pass.

### Discovery Report
- [ ] `docs/discoveries/P010.md` — full report per RULES.md §13 template. Must cover:
  - Anchors verified ✅ vs ⚠️/❌ (especially #9/#12/#13/#22/#23 — rmcp API surface details that Architect couldn't fully pin).
  - rmcp 1.7.0 API reality vs Architect snapshots — any drift documented.
  - Sub-mech A trigger smoke result (Test B output excerpt).
  - Binary size delta vs P009 baseline.
  - Any compile-fix on `ProtocolVersion::Default::default()` or `Implementation` field shape.
  - Lane assignment: Guarded (confirmed).
- [ ] `docs/DISCOVERIES.md` — 1-line index entry appended (newest at top): `2026-05-28 P010: serve subcmd wired (rmcp 1.7.0 stdio handshake, first tokio runtime use, ServerCapabilities empty until P011, 1 integration + 2 unit tests, ~XX tests total) → see docs/discoveries/P010.md`.
- [ ] Sub-mechanism A-F Verification Trace filled in table above (especially Sub-mech A trigger smoke — Test B output excerpt).
