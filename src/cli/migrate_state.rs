//! `migrate-state` subcommand — detect legacy state file formats and
//! convert to JSON v1 schema. Idempotent for JSON v1 input.
//!
//! See `docs/ARCHITECTURE.md` §1 (CLI surface) and §2 (state schema)
//! for the full I/O contract.

use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use thiserror::Error;

use crate::state::{self, SCHEMA_VERSION, StateFile};

/// CLI-level errors specific to migrate-state semantics.
///
/// Both variants map to exit code 1 (per ARCHITECTURE §1 migrate-state
/// "format unknown" semantic).
#[derive(Error, Debug)]
pub enum MigrateError {
    /// File content is not parseable as JSON v1 or single-line ISO-8601.
    #[error(
        "state file `{path}` format unrecognised \
         (not JSON v1, not single-line ISO-8601 timestamp)"
    )]
    FormatUnknown { path: PathBuf },
    /// File is valid JSON but `schema_version` is not 1.
    #[error(
        "state file `{path}` has unsupported schema_version {found} \
         (expected {expected})"
    )]
    UnsupportedSchema {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
}

pub fn run(state_path: PathBuf, dry_run: bool) -> Result<()> {
    // 1. Detect file existence. On NotFound → MISSING branch.
    // Any other I/O error (permission denied, etc.) is wrapped as
    // StateWriteError::Io so the dispatch arm can downcast → exit 2.
    let raw = match std::fs::read_to_string(&state_path) {
        Ok(s) => Some(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err(state::StateWriteError::Io {
                path: state_path.clone(),
                source: e,
            }
            .into());
        }
    };

    // 2. Classify + build target state.
    let (from_label, new_state) = match raw {
        None => {
            // MISSING: write fresh JSON v1 with Utc::now() timestamp.
            (
                "missing",
                StateFile {
                    schema_version: SCHEMA_VERSION,
                    last_scan_at: Utc::now(),
                    seen_advisories: Vec::new(),
                    agent_version: String::new(),
                },
            )
        }
        Some(content) => {
            // Try JSON parse first (more specific failure mode).
            match serde_json::from_str::<StateFile>(&content) {
                Ok(parsed) => {
                    if parsed.schema_version != SCHEMA_VERSION {
                        return Err(MigrateError::UnsupportedSchema {
                            path: state_path.clone(),
                            found: parsed.schema_version,
                            expected: SCHEMA_VERSION,
                        }
                        .into());
                    }
                    // JSON v1: idempotent re-write (normalises pretty-print format).
                    ("json-v1", parsed)
                }
                Err(_json_err) => {
                    // Fall through to legacy single-line ISO-8601 parse.
                    let trimmed = content.trim();
                    match DateTime::parse_from_rfc3339(trimmed) {
                        Ok(parsed_dt) => {
                            let utc = parsed_dt.with_timezone(&Utc);
                            (
                                "legacy",
                                StateFile {
                                    schema_version: SCHEMA_VERSION,
                                    last_scan_at: utc,
                                    // Legacy format had no seen IDs — empty is correct,
                                    // not data loss (Sub-mech C: timestamp preserved).
                                    seen_advisories: Vec::new(),
                                    agent_version: String::new(),
                                },
                            )
                        }
                        Err(_iso_err) => {
                            return Err(MigrateError::FormatUnknown {
                                path: state_path.clone(),
                            }
                            .into());
                        }
                    }
                }
            }
        }
    };

    // 3. Write (unless --dry-run). File is NEVER touched on dry-run.
    if !dry_run {
        state::write_atomic(&state_path, &new_state)?;
    }

    // 4. Emit summary JSON to stdout (regardless of dry_run).
    let summary = json!({
        "from": from_label,
        "to": "json-v1",
        "seen_count": new_state.seen_advisories.len(),
    });
    println!("{summary}");

    Ok(())
}
