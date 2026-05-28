//! `advisory-inbox dedup` — filter parse-report rows against state.seen_advisories.
//!
//! See ARCHITECTURE.md §1 subcmd `dedup` for the I/O contract.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::row::AdvisoryRow;
use crate::state;

/// JSON envelope emitted by `parse-report` (subset — extra fields ignored).
///
/// Accepts shape `{ "rows": [...], ... }`. Flat-array input is NOT supported
/// (per Constraint #4 — Tầng 1 contract: ONE input shape).
#[derive(Deserialize)]
struct RowsEnvelope {
    rows: Vec<AdvisoryRow>,
}

/// Read state + rows JSON, partition rows by `state.seen_advisories` membership,
/// emit `{ "kept": [...], "skipped": [...], "observed_ids": [...] }` to stdout.
///
/// `observed_ids` carries every input row's `advisory_id` regardless of kept/skipped —
/// downstream consumers (e.g., `scan-and-append`) union this into state for the
/// next scan's seen set.
///
/// # Errors
/// - [`state::StateReadError`] (mapped to exit 1 in `main.rs`) — file unreadable,
///   malformed JSON, or schema_version mismatch.
/// - I/O + JSON errors on `rows_json` propagate via anyhow (mapped to exit 2 in main.rs).
pub fn run(state_path: PathBuf, rows_json: PathBuf) -> Result<()> {
    // 1. Read + validate state.
    let state = state::read(&state_path)?;

    // 2. Read rows envelope.
    let rows_text = std::fs::read_to_string(&rows_json)
        .with_context(|| format!("read rows file {}", rows_json.display()))?;
    let envelope: RowsEnvelope = serde_json::from_str(&rows_text)
        .with_context(|| format!("parse rows JSON from {}", rows_json.display()))?;

    // 3. Partition rows into kept / skipped; collect all advisory_ids.
    let mut kept: Vec<AdvisoryRow> = Vec::new();
    let mut skipped: Vec<AdvisoryRow> = Vec::new();
    let mut observed_ids: Vec<String> = Vec::with_capacity(envelope.rows.len());

    for row in envelope.rows {
        observed_ids.push(row.advisory_id.clone());
        if state.seen_advisories.contains(&row.advisory_id) {
            skipped.push(row);
        } else {
            kept.push(row);
        }
    }

    // 4. Emit output JSON + trailing newline (serde_json::json! alphabetises keys:
    //    actual stdout order → "kept" → "observed_ids" → "skipped").
    let out = serde_json::json!({
        "kept": kept,
        "skipped": skipped,
        "observed_ids": observed_ids,
    });
    serde_json::to_writer(std::io::stdout().lock(), &out).context("write stdout JSON")?;
    println!();
    Ok(())
}
