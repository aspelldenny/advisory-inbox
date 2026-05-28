//! MCP tool dispatch — 6 tools per ARCHITECTURE §6.
//!
//! Each tool delegates to existing lib code (sentinel, row, state, inbox, cli/<subcmd>).
//! No duplicate logic — tools wrap the same fns the CLI subcmds call.
//!
//! Error mapping: any tool failure surfaces as JSON-RPC `ErrorData { code: -32000,
//! message: <error display>, data: { subcmd: <name>, exit_code: <N> } }` matching
//! ARCHITECTURE §6 + ARCHITECTURE §1 exit-code conventions.

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

use rmcp::{
    ErrorData, Json, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{ErrorCode, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cli::append as cli_append;
use crate::cli::scan_and_append as cli_scan;
use crate::inbox;
use crate::row::{self, AdvisoryRow};
use crate::sentinel;
use crate::state::{self, SCHEMA_VERSION, StateFile};

// ──────────────────────────────────────────────────────────────
// Error helper
// ──────────────────────────────────────────────────────────────

/// Build a JSON-RPC ErrorData matching ARCHITECTURE §6 error format.
///
/// `code` = -32000 (custom server error per JSON-RPC 2.0 reserved range).
/// `data` = `{ "subcmd": <name>, "exit_code": <N> }` for client diagnostic parity
/// with CLI exit-code semantics (1 = input error, 2 = processing error).
fn mcp_error(subcmd: &str, exit_code: i32, msg: &str) -> ErrorData {
    ErrorData::new(
        ErrorCode(-32000),
        msg.to_string(),
        Some(json!({ "subcmd": subcmd, "exit_code": exit_code })),
    )
}

// ──────────────────────────────────────────────────────────────
// Input + output types (one pair per tool)
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseReportInput {
    /// Agent report markdown containing `<!-- INBOX_APPEND_START -->` sentinel block.
    pub report_text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ParseReportOutput {
    /// Parsed advisory rows from the sentinel block.
    pub rows: Vec<AdvisoryRow>,
    /// Free-form stack metadata (currently always empty — future phiếu populates).
    pub stack_scanned: serde_json::Value,
    /// Number of advisories found (equals rows.len()).
    pub advisories_found: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DedupInput {
    /// Path to `.advisory-scan-state` JSON file.
    pub state_path: String,
    /// Advisory rows to filter (typically from `parse_report` output).
    pub rows: Vec<AdvisoryRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DedupOutput {
    /// Rows not yet in state — safe to append.
    pub kept: Vec<AdvisoryRow>,
    /// Rows already in state — skipped.
    pub skipped: Vec<AdvisoryRow>,
    /// All input row advisory_ids (kept + skipped) for downstream state union.
    pub observed_ids: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendInput {
    /// Path to the inbox markdown file.
    pub inbox_path: String,
    /// Advisory rows to insert at top of `## Rows` section.
    pub rows: Vec<AdvisoryRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendOutput {
    /// Number of rows inserted.
    pub appended_count: usize,
    /// Total open rows in inbox after insert.
    pub total_open: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MigrateStateInput {
    /// Path to the state file to migrate.
    pub state_path: String,
    /// If true, classify and report migration but do not write the file.
    pub dry_run: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MigrateStateOutput {
    /// Detected source format ("missing", "legacy", or "json-v1").
    pub from: String,
    /// Target format (always "json-v1").
    pub to: String,
    /// Number of seen_advisories in the resulting state.
    pub seen_count: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StateBackfillInput {
    /// Path to the state file to update.
    pub state_path: String,
    /// Path to the inbox markdown to extract IDs from.
    pub inbox_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StateBackfillOutput {
    /// Number of IDs added to state (net new from inbox).
    pub backfilled_count: usize,
    /// Total seen_advisories in state after backfill.
    pub total_seen_after: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanAndAppendInput {
    /// Agent report markdown containing the sentinel block.
    pub report_text: String,
    /// Path to the inbox markdown file.
    pub inbox_path: String,
    /// Path to the state JSON file.
    pub state_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScanAndAppendOutput {
    /// Number of new rows inserted into inbox.
    pub appended: usize,
    /// Number of rows skipped (already in state).
    pub skipped_dedup: usize,
    /// Total open rows in inbox after insert.
    pub total_open: usize,
}

// ──────────────────────────────────────────────────────────────
// Service struct + 6 #[tool] methods
// ──────────────────────────────────────────────────────────────

/// MCP service exposing advisory-inbox CLI logic as 6 structured tools.
///
/// `#[tool_router]` generates `Self::tool_router()` static fn used by `#[tool_handler]`.
/// `#[tool_handler] impl ServerHandler` is declared separately so `get_info()` can be
/// overridden to return the correct `advisory-inbox` name from `Cargo.toml` env vars
/// (rmcp's auto-generated `from_build_env()` reads its own crate name, not ours).
pub struct AdvisoryInboxService;

#[tool_router]
impl AdvisoryInboxService {
    /// Parse sentinel block from agent report markdown into structured advisory rows.
    ///
    /// Returns rows, stack metadata (currently empty), and advisory count.
    #[tool(
        name = "parse_report",
        description = "Parse sentinel block from agent report markdown into structured advisory rows."
    )]
    fn parse_report(
        &self,
        Parameters(p): Parameters<ParseReportInput>,
    ) -> Result<Json<ParseReportOutput>, ErrorData> {
        // Strategy A: call lib fns directly (sentinel + row).
        let raw_lines = sentinel::extract_block(&p.report_text)
            .map_err(|e| mcp_error("parse_report", 1, &e.to_string()))?;
        let rows: Vec<AdvisoryRow> = raw_lines
            .iter()
            .map(|line| row::parse_row(line))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| mcp_error("parse_report", 2, &e.to_string()))?;
        let advisories_found = rows.len();
        Ok(Json(ParseReportOutput {
            rows,
            stack_scanned: json!({}),
            advisories_found,
        }))
    }

    /// Filter advisory rows against state seen_advisories to remove duplicates.
    ///
    /// Returns kept (new), skipped (seen), and observed_ids (all input IDs).
    #[tool(
        name = "dedup",
        description = "Filter advisory rows against state seen_advisories to remove duplicates."
    )]
    fn dedup(&self, Parameters(p): Parameters<DedupInput>) -> Result<Json<DedupOutput>, ErrorData> {
        // Strategy A: call state::read + partition logic directly.
        let state_path = PathBuf::from(&p.state_path);
        let st = state::read(&state_path).map_err(|e| mcp_error("dedup", 1, &e.to_string()))?;
        let seen: HashSet<String> = st.seen_advisories.into_iter().collect();
        let mut kept = Vec::new();
        let mut skipped = Vec::new();
        let mut observed_ids = Vec::with_capacity(p.rows.len());
        for r in p.rows {
            observed_ids.push(r.advisory_id.clone());
            if seen.contains(&r.advisory_id) {
                skipped.push(r);
            } else {
                kept.push(r);
            }
        }
        Ok(Json(DedupOutput {
            kept,
            skipped,
            observed_ids,
        }))
    }

    /// Insert advisory rows into the inbox markdown at top of `## Rows` section.
    ///
    /// Atomic write per INV-LOCAL-002. Returns count inserted and total open rows.
    #[tool(
        name = "append",
        description = "Insert advisory rows into the inbox markdown at top of ## Rows section."
    )]
    fn append(
        &self,
        Parameters(p): Parameters<AppendInput>,
    ) -> Result<Json<AppendOutput>, ErrorData> {
        // Strategy B: delegate to cli::append::execute().
        let inbox_path = PathBuf::from(&p.inbox_path);
        let result = cli_append::execute(&inbox_path, &p.rows)
            .map_err(|e| mcp_error("append", 2, &e.to_string()))?;
        Ok(Json(AppendOutput {
            appended_count: result.appended_count,
            total_open: result.total_open,
        }))
    }

    /// Migrate state file from legacy single-line ISO-8601 or missing to JSON v1 schema.
    ///
    /// Idempotent for JSON v1 input. Use dry_run=true to inspect without writing.
    #[tool(
        name = "migrate_state",
        description = "Migrate state file from legacy format to JSON v1 schema. Idempotent for v1 input."
    )]
    fn migrate_state(
        &self,
        Parameters(p): Parameters<MigrateStateInput>,
    ) -> Result<Json<MigrateStateOutput>, ErrorData> {
        // Strategy A: replicate migrate-state logic directly (no stdin — pure path+flag).
        use chrono::Utc;
        let state_path = PathBuf::from(&p.state_path);
        let raw = match std::fs::read_to_string(&state_path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(mcp_error(
                    "migrate_state",
                    2,
                    &format!("reading state file: {e}"),
                ));
            }
        };
        let (from_label, new_state) = match raw {
            None => (
                "missing",
                StateFile {
                    schema_version: SCHEMA_VERSION,
                    last_scan_at: Utc::now(),
                    seen_advisories: Vec::new(),
                    agent_version: String::new(),
                },
            ),
            Some(content) => match serde_json::from_str::<StateFile>(&content) {
                Ok(parsed) => {
                    if parsed.schema_version != SCHEMA_VERSION {
                        return Err(mcp_error(
                            "migrate_state",
                            1,
                            &format!(
                                "unsupported schema_version {} (expected {})",
                                parsed.schema_version, SCHEMA_VERSION
                            ),
                        ));
                    }
                    ("json-v1", parsed)
                }
                Err(_) => {
                    let trimmed = content.trim();
                    match chrono::DateTime::parse_from_rfc3339(trimmed) {
                        Ok(dt) => (
                            "legacy",
                            StateFile {
                                schema_version: SCHEMA_VERSION,
                                last_scan_at: dt.with_timezone(&Utc),
                                seen_advisories: Vec::new(),
                                agent_version: String::new(),
                            },
                        ),
                        Err(_) => {
                            return Err(mcp_error(
                                "migrate_state",
                                1,
                                "state file format unrecognised (not JSON v1, not ISO-8601)",
                            ));
                        }
                    }
                }
            },
        };
        let seen_count = new_state.seen_advisories.len();
        if !p.dry_run {
            state::write_atomic(&state_path, &new_state)
                .map_err(|e| mcp_error("migrate_state", 2, &e.to_string()))?;
        }
        Ok(Json(MigrateStateOutput {
            from: from_label.to_string(),
            to: "json-v1".to_string(),
            seen_count,
        }))
    }

    /// Extract processed/dismissed advisory IDs from inbox and union into state seen_advisories.
    ///
    /// Recovery path when state was lost but inbox retains review decisions.
    #[tool(
        name = "state_backfill",
        description = "Extract processed/dismissed advisory IDs from inbox into state seen_advisories."
    )]
    fn state_backfill(
        &self,
        Parameters(p): Parameters<StateBackfillInput>,
    ) -> Result<Json<StateBackfillOutput>, ErrorData> {
        // Strategy A: call state::read + inbox::read_inbox + inbox::parse_rows directly.
        use crate::row::Status;
        let state_path = PathBuf::from(&p.state_path);
        let inbox_path = PathBuf::from(&p.inbox_path);
        let existing =
            state::read(&state_path).map_err(|e| mcp_error("state_backfill", 1, &e.to_string()))?;
        let inbox_content = inbox::read_inbox(&inbox_path)
            .map_err(|e| mcp_error("state_backfill", 1, &e.to_string()))?;
        let rows = inbox::parse_rows(&inbox_content)
            .map_err(|e| mcp_error("state_backfill", 2, &e.to_string()))?;
        let extracted: BTreeSet<String> = rows
            .iter()
            .filter(|r| matches!(r.status, Status::Processed | Status::Dismissed))
            .map(|r| r.advisory_id.clone())
            .collect();
        let mut union: BTreeSet<String> = existing.seen_advisories.iter().cloned().collect();
        let pre_count = union.len();
        union.extend(extracted);
        let post_count = union.len();
        let backfilled_count = post_count - pre_count;
        let updated = StateFile {
            schema_version: existing.schema_version,
            last_scan_at: existing.last_scan_at,
            seen_advisories: union.into_iter().collect(),
            agent_version: existing.agent_version,
        };
        state::write_atomic(&state_path, &updated)
            .map_err(|e| mcp_error("state_backfill", 2, &e.to_string()))?;
        Ok(Json(StateBackfillOutput {
            backfilled_count,
            total_seen_after: post_count,
        }))
    }

    /// Composite: parse report → dedup → append inbox → update state in one call.
    ///
    /// Writes inbox FIRST then state (P009 atomicity order). On state write failure,
    /// recovery = call state_backfill.
    #[tool(
        name = "scan_and_append",
        description = "Composite: parse report sentinel block, dedup, append inbox, update state."
    )]
    fn scan_and_append(
        &self,
        Parameters(p): Parameters<ScanAndAppendInput>,
    ) -> Result<Json<ScanAndAppendOutput>, ErrorData> {
        // Strategy B: delegate to cli::scan_and_append::execute().
        let inbox_path = PathBuf::from(&p.inbox_path);
        let state_path = PathBuf::from(&p.state_path);
        let result = cli_scan::execute(&p.report_text, &inbox_path, &state_path)
            .map_err(|e| mcp_error("scan_and_append", 2, &e.to_string()))?;
        Ok(Json(ScanAndAppendOutput {
            appended: result.appended,
            skipped_dedup: result.skipped_dedup,
            total_open: result.total_open,
        }))
    }
}

// ──────────────────────────────────────────────────────────────
// ServerHandler impl — custom get_info so name/version read from
// advisory-inbox Cargo.toml, not from rmcp's own build env.
// ──────────────────────────────────────────────────────────────

#[tool_handler]
impl ServerHandler for AdvisoryInboxService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        )
    }
}

// ──────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn service() -> AdvisoryInboxService {
        AdvisoryInboxService
    }

    /// Fixture: minimal valid sentinel report with 2 rows.
    fn report_fixture() -> String {
        "# Advisory Report\n\
         <!-- INBOX_APPEND_START -->\n\
         | 2026-05-28 | CVE-2026-0001 | https://example.com/cve1 | crate-a@1.0 | src/lib.rs:10 | High | open | - |\n\
         | 2026-05-28 | CVE-2026-0002 | https://example.com/cve2 | crate-b@2.0 | src/main.rs:5 | Medium | open | - |\n\
         <!-- INBOX_APPEND_END -->\n"
            .to_string()
    }

    /// Fixture: minimal valid state JSON.
    fn state_fixture(seen: &[&str]) -> String {
        let ids: Vec<String> = seen.iter().map(|s| format!("\"{s}\"")).collect();
        format!(
            r#"{{"schema_version":1,"last_scan_at":"2026-05-28T00:00:00Z","seen_advisories":[{}],"agent_version":""}}"#,
            ids.join(",")
        )
    }

    #[test]
    fn parse_report_happy_path() {
        let svc = service();
        let input = ParseReportInput {
            report_text: report_fixture(),
        };
        let result = svc.parse_report(Parameters(input)).expect("should succeed");
        let Json(out) = result;
        assert_eq!(out.advisories_found, 2);
        assert_eq!(out.rows.len(), 2);
        assert_eq!(out.rows[0].advisory_id, "CVE-2026-0001");
        assert_eq!(out.rows[1].advisory_id, "CVE-2026-0002");
    }

    #[test]
    fn parse_report_missing_sentinel_returns_error() {
        let svc = service();
        let input = ParseReportInput {
            report_text: "# No sentinel here".to_string(),
        };
        let result = svc.parse_report(Parameters(input));
        assert!(result.is_err(), "should fail on missing sentinel");
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert_eq!(err.code, ErrorCode(-32000));
        let data = err.data.expect("should have data object");
        assert_eq!(data["subcmd"], "parse_report");
        assert_eq!(data["exit_code"], 1);
    }

    #[test]
    fn mcp_error_shape_matches_architecture_spec() {
        // ARCHITECTURE §6 error format: code=-32000, data.subcmd, data.exit_code.
        let err = mcp_error("parse_report", 1, "missing sentinel");
        assert_eq!(err.code, ErrorCode(-32000));
        assert!(err.message.contains("missing sentinel"));
        let data = err.data.expect("data must be present");
        assert_eq!(data["subcmd"], "parse_report");
        assert_eq!(data["exit_code"], 1);
    }

    #[test]
    fn dedup_with_mock_state_partitions_correctly() {
        let svc = service();

        // Write a state file with 1 seen ID.
        let mut state_file = NamedTempFile::new().expect("tempfile");
        std::io::Write::write_all(
            &mut state_file,
            state_fixture(&["CVE-2026-0001"]).as_bytes(),
        )
        .expect("write state");

        // 3 rows: 1 already seen (CVE-2026-0001), 2 new.
        let rows = vec![
            row::parse_row(
                "| 2026-05-28 | CVE-2026-0001 | https://x.com | pkg@1 | f:1 | High | open | - |",
            )
            .unwrap(),
            row::parse_row(
                "| 2026-05-28 | CVE-2026-0002 | https://x.com | pkg@2 | f:2 | High | open | - |",
            )
            .unwrap(),
            row::parse_row(
                "| 2026-05-28 | CVE-2026-0003 | https://x.com | pkg@3 | f:3 | High | open | - |",
            )
            .unwrap(),
        ];
        let input = DedupInput {
            state_path: state_file.path().to_string_lossy().to_string(),
            rows,
        };
        let result = svc.dedup(Parameters(input)).expect("should succeed");
        let Json(out) = result;
        assert_eq!(out.kept.len(), 2, "CVE-0002 and CVE-0003 are new");
        assert_eq!(out.skipped.len(), 1, "CVE-0001 already seen");
        assert_eq!(out.observed_ids.len(), 3, "all 3 ids tracked");
    }
}
