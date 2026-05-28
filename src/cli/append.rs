//! `advisory-inbox append` — insert filtered rows into the inbox markdown at
//! the top of `## Rows`, atomic-write.
//!
//! See `docs/ARCHITECTURE.md` §1 subcmd `append` for the I/O contract,
//! §3 for the inbox markdown format, and `docs/security/INVARIANTS.md` §3
//! INV-LOCAL-002 for the atomic-write protocol (delegated to `inbox::write_atomic`).

use std::path::PathBuf;

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

    // 2. Read inbox.
    let content = inbox::read_inbox(&inbox_path)?;

    // 3. Insert (newest at top — rows[0] topmost).
    let (new_content, total_open) = inbox::insert_rows(&content, &envelope.rows, &inbox_path)?;

    // 4. Atomic write per INV-LOCAL-002.
    inbox::write_atomic(&inbox_path, &new_content)?;

    // 5. Emit JSON stdout + trailing newline.
    let appended_count = envelope.rows.len();
    let out = serde_json::json!({
        "appended_count": appended_count,
        "total_open": total_open,
    });
    serde_json::to_writer(std::io::stdout().lock(), &out).context("write stdout JSON")?;
    println!();
    Ok(())
}
