//! Sentinel marker block extractor.
//!
//! Agent advisory-watch emits report markdown containing a block delimited by
//! `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->`. This module
//! locates the first such pair and extracts the raw row lines between them,
//! skipping blank and HTML-comment lines.
//!
//! See ARCHITECTURE.md §4 for the format contract.

// Exported items are scaffold-declared for P004+ consumers. Allow dead_code
// until cli/parse_report.rs wire-in phiếu (P004) imports extract_block.
#![allow(dead_code)]

use thiserror::Error;

/// Sentinel marker opening the appendable block.
pub const SENTINEL_START: &str = "<!-- INBOX_APPEND_START -->";

/// Sentinel marker closing the appendable block.
pub const SENTINEL_END: &str = "<!-- INBOX_APPEND_END -->";

/// Errors returned by [`extract_block`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum SentinelError {
    #[error("missing sentinel start marker `<!-- INBOX_APPEND_START -->` in report")]
    MissingStartMarker,
    #[error("missing sentinel end marker `<!-- INBOX_APPEND_END -->` after start")]
    MissingEndMarker,
}

/// Extract raw row lines from the first sentinel block in `report_text`.
///
/// Returns each non-blank, non-comment line between the first
/// `<!-- INBOX_APPEND_START -->` and the next `<!-- INBOX_APPEND_END -->`.
/// If multiple START markers exist beyond the first pair, a warning is emitted
/// to stderr and only the first pair is used.
///
/// # Errors
/// - [`SentinelError::MissingStartMarker`] if START not found.
/// - [`SentinelError::MissingEndMarker`] if START found but no END after it.
pub fn extract_block(report_text: &str) -> Result<Vec<String>, SentinelError> {
    // 1. Locate first START marker.
    let start_idx = report_text
        .find(SENTINEL_START)
        .ok_or(SentinelError::MissingStartMarker)?;
    let after_start = start_idx + SENTINEL_START.len();

    // 2. Locate END marker AFTER first START.
    let end_offset = report_text[after_start..]
        .find(SENTINEL_END)
        .ok_or(SentinelError::MissingEndMarker)?;
    let end_idx = after_start + end_offset;

    // 3. Warn if extra START markers exist beyond the first pair.
    //    This is intentional operational stderr output (not debug cruft) — see
    //    ARCHITECTURE.md §4 and P003 Discovery Report O1.2 ACK.
    let remainder = &report_text[end_idx + SENTINEL_END.len()..];
    let extra_starts = remainder.matches(SENTINEL_START).count();
    if extra_starts > 0 {
        eprintln!(
            "warn: multiple INBOX_APPEND_START markers found ({} extra after first pair); using first pair only",
            extra_starts
        );
    }

    // 4. Slice between markers + filter lines.
    let block = &report_text[after_start..end_idx];
    let rows: Vec<String> = block
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| {
            let trimmed = l.trim_start();
            !trimmed.is_empty() && !trimmed.starts_with("<!--")
        })
        .map(|l| l.to_string())
        .collect();

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_two_rows() {
        let report = "\
some preamble\n\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-2026-0001 | url1 | pkg1 | f:1 | High | open | - |\n\
| 2026-05-28 | CVE-2026-0002 | url2 | pkg2 | f:2 | Medium | open | - |\n\
<!-- INBOX_APPEND_END -->\n\
trailing\n";
        let rows = extract_block(report).expect("should parse");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("CVE-2026-0001"));
        assert!(rows[1].contains("CVE-2026-0002"));
    }

    #[test]
    fn empty_block_returns_empty_vec() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("empty block is valid");
        assert!(rows.is_empty());
    }

    #[test]
    fn missing_start_marker_errors() {
        let report = "no markers here at all, just prose";
        let err = extract_block(report).unwrap_err();
        assert_eq!(err, SentinelError::MissingStartMarker);
    }

    #[test]
    fn missing_end_marker_errors() {
        let report = "<!-- INBOX_APPEND_START -->\n| row | ... |\n";
        let err = extract_block(report).unwrap_err();
        assert_eq!(err, SentinelError::MissingEndMarker);
    }

    #[test]
    fn multiple_start_uses_first_pair() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-FIRST | u | p | f:1 | High | open | - |\n\
<!-- INBOX_APPEND_END -->\n\
between\n\
<!-- INBOX_APPEND_START -->\n\
| 2026-05-28 | CVE-SECOND | u | p | f:2 | Low | open | - |\n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("first pair valid");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("CVE-FIRST"));
        assert!(!rows[0].contains("CVE-SECOND"));
    }

    #[test]
    fn block_with_blank_and_comment_lines_skipped() {
        let report = "\
<!-- INBOX_APPEND_START -->\n\
\n\
| 2026-05-28 | CVE-REAL | u | p | f:1 | High | open | - |\n\
<!-- placeholder example -->\n\
   \n\
<!-- INBOX_APPEND_END -->\n";
        let rows = extract_block(report).expect("ok");
        assert_eq!(rows.len(), 1, "only real row kept");
        assert!(rows[0].contains("CVE-REAL"));
    }
}
