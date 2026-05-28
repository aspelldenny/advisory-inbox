//! AdvisoryRow type — 1 row in inbox markdown table (ARCHITECTURE.md §3).
//!
//! Serialized as JSON between subcommands (parse-report → dedup → append).
//! Status/Severity enums lock the wire format per upstream advisory convention.

// Types are scaffold-declared for P003+ consumers. Allow dead_code until
// subcmd wire-in phiếu (P004+) imports them.
#![allow(dead_code)]

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Status of an advisory row — Sếp gates `open` → `processed`/`dismissed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Processed,
    Dismissed,
}

/// Severity per upstream advisory (Critical/High/Medium/Low only — RULES.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}
