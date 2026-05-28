//! `state-backfill` subcommand — extract advisory IDs from inbox rows with
//! status `processed`/`dismissed` and union into state.seen_advisories.
//!
//! Recovery path for users whose state file was lost/corrupted but whose
//! inbox markdown retains review decisions. See `docs/ARCHITECTURE.md` §1
//! (CLI surface) for the I/O contract.
//!
//! Sub-mech C invariant: post.seen_advisories ⊇ pre.seen_advisories.
//! `last_scan_at` and `agent_version` are PRESERVED (backfill is not a scan event).

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::inbox::{self, InboxError};
use crate::row::Status;
use crate::state::{self, StateFile};

pub fn run(state_path: PathBuf, inbox_path: PathBuf, dry_run: bool) -> Result<()> {
    // 1. Read existing state (bubble StateReadError → main.rs maps to exit 1).
    let existing = state::read(&state_path)
        .with_context(|| format!("reading state file `{}`", state_path.display()))?;

    // 2. Read inbox markdown (bubble InboxError::Io → main.rs maps to exit 1).
    let inbox_content = inbox::read_inbox(&inbox_path)?;

    // 3. Parse rows; re-wrap ParseRow error with real inbox path.
    let rows = match inbox::parse_rows(&inbox_content) {
        Ok(rs) => rs,
        Err(InboxError::ParseRow {
            line_number,
            source,
            ..
        }) => {
            return Err(InboxError::ParseRow {
                path: inbox_path.clone(),
                line_number,
                source,
            }
            .into());
        }
        Err(e) => return Err(e.into()),
    };

    // 4. Extract IDs from rows with status processed/dismissed only.
    //    Status::Open rows MUST NOT contribute (constraint #7).
    let extracted: BTreeSet<String> = rows
        .iter()
        .filter(|r| matches!(r.status, Status::Processed | Status::Dismissed))
        .map(|r| r.advisory_id.clone())
        .collect();

    // 5. Union with pre-existing seen_advisories.
    //    BTreeSet semantics ensure monotonic non-shrink (Sub-mech C).
    let mut union: BTreeSet<String> = existing.seen_advisories.iter().cloned().collect();
    let pre_count = union.len();
    union.extend(extracted);
    let post_count = union.len();
    let backfilled_count = post_count - pre_count;

    // 6. Build updated state (PRESERVE last_scan_at + agent_version + schema_version).
    //    Backfill is a RECOVERY operation, not a scan event — do NOT set last_scan_at = now().
    let updated = StateFile {
        schema_version: existing.schema_version,
        last_scan_at: existing.last_scan_at,
        seen_advisories: union.into_iter().collect(), // BTreeSet → sorted Vec
        agent_version: existing.agent_version,
    };

    // 7. Write (unless dry-run). Always write to canonicalize sort + JSON format.
    if !dry_run {
        state::write_atomic(&state_path, &updated)
            .with_context(|| format!("writing backfilled state to `{}`", state_path.display()))?;
    }

    // 8. Emit summary JSON to stdout.
    println!(
        "{}",
        json!({
            "backfilled_count": backfilled_count,
            "total_seen_after": post_count,
        })
    );

    Ok(())
}
