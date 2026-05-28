//! `scan-and-append` composite subcommand — compose parse → dedup → append + state update
//! into a single invocation.
//!
//! See `docs/ARCHITECTURE.md` §1 (CLI surface) for the I/O contract.
//!
//! **Atomicity note:** This composite writes TWO files (`inbox` markdown + `state` JSON).
//! Each write is individually atomic via INV-LOCAL-002 (temp+fsync+rename), but the PAIR
//! is NOT cross-file transactional. Write order is inbox FIRST, then state — if state
//! write fails after inbox succeeded, inbox has new rows but state lacks IDs; recovery
//! path is `advisory-inbox state-backfill`. See phiếu P009 Constraint #3.
//!
//! Sub-mech C invariant: post.seen_advisories ⊇ pre.seen_advisories (BTreeSet union).
//! `last_scan_at` UPDATED to Utc::now() (scan event); `agent_version` PRESERVED.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;

use crate::inbox;
use crate::row;
use crate::sentinel;
use crate::state::{self, StateFile};

/// Result of a successful scan-and-append operation (used by MCP tool dispatch).
pub struct ScanAndAppendResult {
    /// Number of new rows inserted into the inbox.
    pub appended: usize,
    /// Number of rows skipped because they were already in state.
    pub skipped_dedup: usize,
    /// Total number of open rows in the inbox after insert.
    pub total_open: usize,
}

/// Pure composite logic: parse report → dedup → append inbox → update state.
///
/// Called by both `run()` (CLI path) and `mcp::tools` (MCP path) to avoid duplication.
/// See `docs/ARCHITECTURE.md` §1 and P009 atomicity notes for write-order semantics.
///
/// # Errors
/// - [`sentinel::SentinelError`] → exit 1 in `main.rs`.
/// - [`row::RowParseError`] → exit 2.
/// - [`state::StateReadError`] → exit 1.
/// - [`inbox::InboxError`] → exit 1 (missing heading) or exit 2 (IO).
pub fn execute(
    report_text: &str,
    inbox_path: &Path,
    state_path: &Path,
) -> Result<ScanAndAppendResult> {
    // 1. Extract sentinel block → raw row lines.
    let raw_lines = sentinel::extract_block(report_text)?;

    // 2. Parse each row.
    let parsed_rows: Vec<_> = raw_lines
        .iter()
        .map(|line| row::parse_row(line))
        .collect::<Result<Vec<_>, _>>()?;

    // 3. Read existing state.
    let pre_state = state::read(state_path)
        .with_context(|| format!("reading state file `{}`", state_path.display()))?;

    // 4. Partition into (kept, skipped) vs pre_state.seen_advisories.
    let seen: std::collections::HashSet<String> =
        pre_state.seen_advisories.iter().cloned().collect();
    let (kept, skipped): (Vec<_>, Vec<_>) = parsed_rows
        .into_iter()
        .partition(|r| !seen.contains(&r.advisory_id));
    let observed_ids: BTreeSet<String> = kept
        .iter()
        .chain(skipped.iter())
        .map(|r| r.advisory_id.clone())
        .collect();

    // 5. Read inbox markdown.
    let inbox_content = inbox::read_inbox(inbox_path)?;

    // 6. Insert kept rows into inbox content (newest at top of `## Rows`).
    let (new_content, total_open) = inbox::insert_rows(&inbox_content, &kept, inbox_path)?;

    // 7. Write inbox FIRST (locked order per Architecture Decision #3).
    inbox::write_atomic(inbox_path, &new_content)
        .with_context(|| format!("writing inbox to `{}`", inbox_path.display()))?;

    // 8. Build updated state: union seen_advisories with observed_ids, bump last_scan_at.
    let mut union: BTreeSet<String> = pre_state.seen_advisories.iter().cloned().collect();
    union.extend(observed_ids);
    let updated = StateFile {
        schema_version: pre_state.schema_version,
        last_scan_at: chrono::Utc::now(),
        seen_advisories: union.into_iter().collect(),
        agent_version: pre_state.agent_version,
    };

    // 9. Write state SECOND.
    state::write_atomic(state_path, &updated)
        .with_context(|| format!("writing state to `{}`", state_path.display()))?;

    Ok(ScanAndAppendResult {
        appended: kept.len(),
        skipped_dedup: skipped.len(),
        total_open,
    })
}

pub fn run(report: Option<PathBuf>, inbox_path: PathBuf, state_path: PathBuf) -> Result<()> {
    // 1. Read report text (stdin if report is None, else file).
    let report_text = match report {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("reading report file `{}`", path.display()))?,
        None => {
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .context("reading report from stdin")?;
            s
        }
    };

    // 2. Execute pure logic.
    let result = execute(&report_text, &inbox_path, &state_path)?;

    // 3. Emit summary JSON to stdout.
    let summary = json!({
        "appended": result.appended,
        "skipped_dedup": result.skipped_dedup,
        "total_open": result.total_open,
    });
    println!("{}", summary);

    Ok(())
}
