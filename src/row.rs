//! AdvisoryRow type — 1 row in inbox markdown table (ARCHITECTURE.md §3).
//!
//! Serialized as JSON between subcommands (parse-report → dedup → append).
//! Status/Severity enums lock the wire format per upstream advisory convention.

use std::fmt;
use std::str::FromStr;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Status of an advisory row — Sếp gates `open` → `processed`/`dismissed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Processed,
    Dismissed,
}

/// Severity per upstream advisory (Critical/High/Medium/Low only — RULES.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// One row in the inbox markdown table — 8 columns per ARCHITECTURE.md §3.
///
/// Column order (markdown): Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note.
/// JSON field order matches struct field order (serde default).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AdvisoryRow {
    /// Date the advisory was observed (YYYY-MM-DD).
    pub date: NaiveDate,
    /// Advisory ID (CVE-YYYY-NNNNN, GHSA-xxxx-yyyy, RUSTSEC-YYYY-NNNN, etc.).
    pub advisory_id: String,
    /// Upstream advisory URL.
    pub source_url: String,
    /// Affected package spec (e.g., `next@<15.5.17`).
    pub package: String,
    /// Code location (`path/to/file.ext:line` or `indirect` for transitive).
    pub file_line: String,
    /// Severity per upstream.
    pub severity: Severity,
    /// Current status (open until Sếp gates).
    pub status: Status,
    /// Free-form note (`-` placeholder when empty).
    pub note: String,
}

/// Errors returned by [`parse_row`] when an inbox row line cannot be decoded.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum RowParseError {
    #[error("empty row line")]
    EmptyLine,
    #[error("expected {expected} cells, got {actual}")]
    WrongCellCount { expected: usize, actual: usize },
    #[error("invalid date `{0}` (expected YYYY-MM-DD)")]
    InvalidDate(String),
    #[error("invalid severity `{0}` (expected Critical/High/Medium/Low)")]
    InvalidSeverity(String),
    #[error("invalid status `{0}` (expected open/processed/dismissed)")]
    InvalidStatus(String),
}

impl FromStr for Status {
    type Err = RowParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "open" => Ok(Status::Open),
            "processed" => Ok(Status::Processed),
            "dismissed" => Ok(Status::Dismissed),
            other => Err(RowParseError::InvalidStatus(other.to_string())),
        }
    }
}

impl FromStr for Severity {
    type Err = RowParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Critical" => Ok(Severity::Critical),
            "High" => Ok(Severity::High),
            "Medium" => Ok(Severity::Medium),
            "Low" => Ok(Severity::Low),
            other => Err(RowParseError::InvalidSeverity(other.to_string())),
        }
    }
}

/// Parse one pipe-delimited inbox row line into an [`AdvisoryRow`].
///
/// Expects exactly 8 cells in order:
/// `Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note`.
///
/// Whitespace around each cell is trimmed. Leading/trailing `|` are stripped.
///
/// # Errors
/// - [`RowParseError::EmptyLine`] if line is empty after trimming.
/// - [`RowParseError::WrongCellCount`] if cell count != 8.
/// - [`RowParseError::InvalidDate`] if Date cell does not match `YYYY-MM-DD`.
/// - [`RowParseError::InvalidSeverity`] / [`RowParseError::InvalidStatus`] for unknown enum values.
pub fn parse_row(line: &str) -> Result<AdvisoryRow, RowParseError> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(RowParseError::EmptyLine);
    }
    let inner = trimmed.trim_start_matches('|').trim_end_matches('|');
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    if cells.len() != 8 {
        return Err(RowParseError::WrongCellCount {
            expected: 8,
            actual: cells.len(),
        });
    }
    let date = NaiveDate::parse_from_str(cells[0], "%Y-%m-%d")
        .map_err(|_| RowParseError::InvalidDate(cells[0].to_string()))?;
    let advisory_id = cells[1].to_string();
    let source_url = cells[2].to_string();
    let package = cells[3].to_string();
    let file_line = cells[4].to_string();
    let severity = Severity::from_str(cells[5])?;
    let status = Status::from_str(cells[6])?;
    let note = cells[7].to_string();
    Ok(AdvisoryRow {
        date,
        advisory_id,
        source_url,
        package,
        file_line,
        severity,
        status,
        note,
    })
}

impl fmt::Display for Status {
    /// Render status as lowercase variant name per ARCHITECTURE §3 and serde `rename_all = "lowercase"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Open => f.write_str("open"),
            Status::Processed => f.write_str("processed"),
            Status::Dismissed => f.write_str("dismissed"),
        }
    }
}

impl fmt::Display for Severity {
    /// Render severity as PascalCase variant name per ARCHITECTURE §3 and serde `rename_all = "PascalCase"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Critical => f.write_str("Critical"),
            Severity::High => f.write_str("High"),
            Severity::Medium => f.write_str("Medium"),
            Severity::Low => f.write_str("Low"),
        }
    }
}

impl fmt::Display for AdvisoryRow {
    /// Render as pipe-delimited 8-col line per ARCHITECTURE §3.
    ///
    /// Format: `| {date} | {advisory_id} | {source_url} | {package} | {file_line} | {severity} | {status} | {note} |`
    ///
    /// - `date`: ISO calendar date `YYYY-MM-DD` (chrono NaiveDate default Display).
    /// - `severity`: PascalCase variant name (`Critical`/`High`/`Medium`/`Low`).
    /// - `status`: lowercase variant name (`open`/`processed`/`dismissed`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "| {date} | {advisory_id} | {source_url} | {package} | {file_line} | {severity} | {status} | {note} |",
            date = self.date,
            advisory_id = self.advisory_id,
            source_url = self.source_url,
            package = self.package,
            file_line = self.file_line,
            severity = self.severity,
            status = self.status,
            note = self.note,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> AdvisoryRow {
        AdvisoryRow {
            date: NaiveDate::from_ymd_opt(2026, 5, 28).expect("valid date"),
            advisory_id: "CVE-2026-9999".to_string(),
            source_url: "https://nvd.nist.gov/vuln/detail/CVE-2026-9999".to_string(),
            package: "next@<15.5.17".to_string(),
            file_line: "src/middleware.ts:42".to_string(),
            severity: Severity::High,
            status: Status::Open,
            note: "-".to_string(),
        }
    }

    #[test]
    fn row_roundtrip_json() {
        let row = sample_row();
        let json = serde_json::to_string(&row).expect("serialize");
        let back: AdvisoryRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(row, back);
    }

    #[test]
    fn status_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Status::Open).unwrap(), "\"open\"");
        assert_eq!(
            serde_json::to_string(&Status::Processed).unwrap(),
            "\"processed\""
        );
        assert_eq!(
            serde_json::to_string(&Status::Dismissed).unwrap(),
            "\"dismissed\""
        );
    }

    #[test]
    fn severity_serializes_pascalcase() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"Critical\""
        );
        assert_eq!(serde_json::to_string(&Severity::High).unwrap(), "\"High\"");
        assert_eq!(
            serde_json::to_string(&Severity::Medium).unwrap(),
            "\"Medium\""
        );
        assert_eq!(serde_json::to_string(&Severity::Low).unwrap(), "\"Low\"");
    }

    #[test]
    fn row_parses_known_json() {
        // ARCHITECTURE.md §3 example row as JSON (field order = struct order).
        let json = r#"{
            "date": "2026-05-28",
            "advisory_id": "CVE-2026-9999",
            "source_url": "https://nvd.nist.gov/vuln/detail/CVE-2026-9999",
            "package": "next@<15.5.17",
            "file_line": "src/middleware.ts:42",
            "severity": "High",
            "status": "open",
            "note": "-"
        }"#;
        let row: AdvisoryRow = serde_json::from_str(json).expect("parse known JSON");
        assert_eq!(row.date, NaiveDate::from_ymd_opt(2026, 5, 28).unwrap());
        assert_eq!(row.severity, Severity::High);
        assert_eq!(row.status, Status::Open);
    }

    #[test]
    fn parse_row_happy_path() {
        let line = "| 2026-05-28 | CVE-2026-0001 | https://example.com/cve | next@<15.5.17 | src/middleware.ts:42 | High | open | - |";
        let row = parse_row(line).expect("happy path parses");
        assert_eq!(row.advisory_id, "CVE-2026-0001");
        assert_eq!(row.severity, Severity::High);
        assert_eq!(row.status, Status::Open);
        assert_eq!(row.note, "-");
    }

    #[test]
    fn parse_row_bad_date_errors() {
        let line = "| not-a-date | CVE-X | u | p | f:1 | High | open | - |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(err, RowParseError::InvalidDate(_)));
    }

    #[test]
    fn parse_row_bad_severity_errors() {
        let line = "| 2026-05-28 | CVE-X | u | p | f:1 | Critic | open | - |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(err, RowParseError::InvalidSeverity(s) if s == "Critic"));
    }

    #[test]
    fn parse_row_wrong_cell_count() {
        let line = "| 2026-05-28 | CVE-X | only-three |";
        let err = parse_row(line).unwrap_err();
        assert!(matches!(
            err,
            RowParseError::WrongCellCount {
                expected: 8,
                actual: 3
            }
        ));
    }

    #[test]
    fn status_from_str_roundtrip() {
        assert_eq!("open".parse::<Status>().unwrap(), Status::Open);
        assert_eq!("dismissed".parse::<Status>().unwrap(), Status::Dismissed);
        assert!("OPEN".parse::<Status>().is_err());
    }

    #[test]
    fn severity_from_str_canonical() {
        assert_eq!("Critical".parse::<Severity>().unwrap(), Severity::Critical);
        assert_eq!("Low".parse::<Severity>().unwrap(), Severity::Low);
        // Case-sensitive PascalCase — lowercase must fail.
        assert!("critical".parse::<Severity>().is_err());
    }

    #[test]
    fn advisory_row_display_pipe_delim() {
        let row = AdvisoryRow {
            date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
            advisory_id: "CVE-2026-9999".to_string(),
            source_url: "https://nvd.nist.gov/vuln/detail/CVE-2026-9999".to_string(),
            package: "next@<15.5.17".to_string(),
            file_line: "src/middleware.ts:42".to_string(),
            severity: Severity::High,
            status: Status::Open,
            note: "-".to_string(),
        };
        let rendered = format!("{row}");
        assert_eq!(
            rendered,
            "| 2026-05-28 | CVE-2026-9999 | https://nvd.nist.gov/vuln/detail/CVE-2026-9999 | next@<15.5.17 | src/middleware.ts:42 | High | open | - |"
        );
    }
}
