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
        ServerInfo::new(ServerCapabilities::builder().build()).with_server_info(
            Implementation::new("advisory-inbox", env!("CARGO_PKG_VERSION")),
        )
    }
    // All other ServerHandler methods use provided defaults.
    // `list_tools` default returns empty list — MCP-compliant for server with no tools capability.
}

pub fn run() -> Result<()> {
    // Build current-thread runtime (Cargo.toml declares `rt` feature only — NO multi-thread).
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
        // ServerCapabilities::builder().build() leaves tools as None.
        assert!(
            info.capabilities.tools.is_none(),
            "P010 must not declare tools capability — P011 responsibility"
        );
    }
}
