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
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::inbox;
use crate::row;
use crate::sentinel;
use crate::state::{self, StateFile};

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

    // 2. Extract sentinel block → raw row lines. Bubble SentinelError → main.rs maps to exit 1.
    let raw_lines = sentinel::extract_block(&report_text)?;

    // 3. Parse each row. Bubble RowParseError → main.rs maps to exit 2.
    let parsed_rows: Vec<_> = raw_lines
        .iter()
        .map(|line| row::parse_row(line))
        .collect::<Result<Vec<_>, _>>()?;

    // 4. Read existing state. Bubble StateReadError → main.rs maps to exit 1.
    let pre_state = state::read(&state_path)
        .with_context(|| format!("reading state file `{}`", state_path.display()))?;

    // 5. Partition into (kept, skipped) vs pre_state.seen_advisories.
    //    observed_ids = ALL row.advisory_id (kept + skipped), used downstream for state update.
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

    // 6. Read inbox markdown. Bubble InboxError::Io → main.rs maps to exit 1.
    let inbox_content = inbox::read_inbox(&inbox_path)?;

    // 7. Insert kept rows into inbox content (newest at top of `## Rows`).
    //    Bubble InboxError::MissingRowsHeading → exit 1.
    //    O1.1 mechanical fix: insert_rows takes 3 params (content, rows, path).
    let (new_content, total_open) = inbox::insert_rows(&inbox_content, &kept, &inbox_path)?;

    // 8. Write inbox FIRST (locked order per Architecture Decision #3).
    //    InboxError::Io on write → main.rs maps to exit 2.
    inbox::write_atomic(&inbox_path, &new_content)
        .with_context(|| format!("writing inbox to `{}`", inbox_path.display()))?;

    // 9. Build updated state: union seen_advisories with observed_ids, bump last_scan_at.
    //    Sub-mech C: BTreeSet union guarantees post.seen_advisories ⊇ pre.seen_advisories.
    let mut union: BTreeSet<String> = pre_state.seen_advisories.iter().cloned().collect();
    union.extend(observed_ids);
    let updated = StateFile {
        schema_version: pre_state.schema_version,
        last_scan_at: chrono::Utc::now(), // scan event — UPDATE timestamp
        seen_advisories: union.into_iter().collect(), // BTreeSet → sorted Vec
        agent_version: pre_state.agent_version, // PRESERVED per Architecture Decision #5
    };

    // 10. Write state SECOND. StateWriteError::Io → main.rs maps to exit 2.
    state::write_atomic(&state_path, &updated)
        .with_context(|| format!("writing state to `{}`", state_path.display()))?;

    // 11. Emit summary JSON to stdout.
    let summary = json!({
        "appended": kept.len(),
        "skipped_dedup": skipped.len(),
        "total_open": total_open,
    });
    println!("{}", summary);

    Ok(())
}
