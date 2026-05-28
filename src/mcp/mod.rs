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
