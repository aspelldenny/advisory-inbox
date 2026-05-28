//! `advisory-inbox append` — insert filtered rows into the inbox markdown at
//! the top of `## Rows`, atomic-write.
//!
//! See `docs/ARCHITECTURE.md` §1 subcmd `append` for the I/O contract,
//! §3 for the inbox markdown format, and `docs/security/INVARIANTS.md` §3
//! INV-LOCAL-002 for the atomic-write protocol (delegated to `inbox::write_atomic`).

use std::path::{Path, PathBuf};

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

/// Result of a successful append operation (used by MCP tool dispatch).
pub struct AppendResult {
    /// Number of rows inserted into the inbox.
    pub appended_count: usize,
    /// Total number of open rows in the inbox after insert.
    pub total_open: usize,
}

/// Pure append logic: insert `rows` into inbox at `inbox_path`, atomic-write.
///
/// Called by both `run()` (CLI path) and `mcp::tools` (MCP path) to avoid duplication.
///
/// # Errors
/// - [`inbox::InboxError::MissingRowsHeading`] (exit 1 in `main.rs`).
/// - [`inbox::InboxError::Io`] (exit 2 in `main.rs`).
pub fn execute(inbox_path: &Path, rows: &[AdvisoryRow]) -> Result<AppendResult> {
    // 1. Read inbox.
    let content = inbox::read_inbox(inbox_path)?;

    // 2. Insert (newest at top — rows[0] topmost).
    let (new_content, total_open) = inbox::insert_rows(&content, rows, inbox_path)?;

    // 3. Atomic write per INV-LOCAL-002.
    inbox::write_atomic(inbox_path, &new_content)?;

    Ok(AppendResult {
        appended_count: rows.len(),
        total_open,
    })
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

    // 2. Execute pure logic.
    let result = execute(&inbox_path, &envelope.rows)?;

    // 3. Emit JSON stdout + trailing newline.
    let out = serde_json::json!({
        "appended_count": result.appended_count,
        "total_open": result.total_open,
    });
    serde_json::to_writer(std::io::stdout().lock(), &out).context("write stdout JSON")?;
    println!();
    Ok(())
}
