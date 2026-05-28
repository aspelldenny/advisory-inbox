//! Inbox markdown parser + writer for advisory-inbox.
//!
//! See `docs/ARCHITECTURE.md` §3 for the inbox markdown format,
//! and §7 + `docs/security/INVARIANTS.md` §3 INV-LOCAL-002 for the
//! atomic-write protocol (this module is the first concrete user).

use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;
use thiserror::Error;

use crate::row::AdvisoryRow;

/// Errors raised by inbox read/parse/write operations.
///
/// Exit-code mapping (caller's responsibility in `main.rs`):
/// - [`InboxError::MissingRowsHeading`] → exit code 1 (per ARCHITECTURE §1 append).
/// - [`InboxError::Io`] → exit code 2.
/// - [`InboxError::ParseRow`] → exit code 1 (per ARCHITECTURE §1 state-backfill).
#[derive(Error, Debug)]
pub enum InboxError {
    #[error("inbox `{path}` is missing `## Rows` heading — cannot determine insert position")]
    MissingRowsHeading { path: PathBuf },
    #[error("inbox `{path}` I/O failure: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Row parse failure during `parse_rows`. `line_number` is 1-based line index
    /// in the full inbox content (NOT relative to `## Rows` section start).
    #[error("inbox `{path}` row parse failed at line {line_number}: {source}")]
    ParseRow {
        path: PathBuf,
        line_number: usize,
        #[source]
        source: crate::row::RowParseError,
    },
}

/// Read the inbox markdown file at `path` into a String.
///
/// # Errors
/// Returns [`InboxError::Io`] if the file cannot be read.
pub fn read_inbox(path: &Path) -> Result<String, InboxError> {
    std::fs::read_to_string(path).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Insert `rows` after the `## Rows` heading line. Returns the new content
/// and the total count of `status == open` rows in the resulting inbox.
///
/// Order: `rows[0]` ends up TOPMOST. Existing rows are preserved in their
/// original relative order (shifted down). Caller emits rows newest-first
/// to maintain the inbox's "newest at top" invariant.
///
/// Errors with [`InboxError::MissingRowsHeading`] if no line equal to
/// `## Rows` (after `trim_end`) is found in `content`.
///
/// Empty `rows` slice → no-op: returns original content + recounts `total_open`.
///
/// The `path` parameter is embedded in the error for user-facing messages.
pub fn insert_rows(
    content: &str,
    rows: &[AdvisoryRow],
    path: &Path,
) -> Result<(String, usize), InboxError> {
    // 1. Split content into lines.
    let lines: Vec<&str> = content.lines().collect();

    // 2. Find `## Rows` heading (line equals "## Rows" after trailing whitespace trim).
    let heading_idx = lines
        .iter()
        .position(|l| l.trim_end() == "## Rows")
        .ok_or_else(|| InboxError::MissingRowsHeading {
            path: path.to_path_buf(),
        })?;

    // 3. Build output: lines[0..=heading_idx] + new rows (forward order — rows[0] topmost)
    //    + lines[heading_idx+1..].
    let mut out = String::with_capacity(content.len() + rows.len() * 200);
    for line in &lines[..=heading_idx] {
        out.push_str(line);
        out.push('\n');
    }
    for row in rows {
        // `impl Display for AdvisoryRow` emits the pipe-delim 8-col line per ARCHITECTURE §3.
        out.push_str(&row.to_string());
        out.push('\n');
    }
    for line in &lines[heading_idx + 1..] {
        out.push_str(line);
        out.push('\n');
    }

    // Preserve trailing-newline behavior: `lines()` strips the final newline.
    // If the original did NOT end with '\n', the loop above added one extra — remove it.
    if !content.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    // 4. Count total_open: scan output, count pipe-delim rows with status=open outside HTML comments.
    let total_open = count_open_rows(&out);

    Ok((out, total_open))
}

/// Count rows with `status == open` in the inbox content. Skips HTML comment
/// blocks (`<!-- ... -->`) per ARCHITECTURE §3 parser rules.
///
/// Implementation: line-by-line scan with a simple `in_comment_block` flag.
/// A line counts as an "open row" if (a) we are NOT in a comment block,
/// and (b) the line contains the substring `| open |` (pipe-delim heuristic).
fn count_open_rows(content: &str) -> usize {
    let mut in_comment = false;
    let mut count = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<!--") && !trimmed.contains("-->") {
            in_comment = true;
            continue;
        }
        if in_comment {
            if trimmed.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        // Substring match: pipe-delim line with ` | open | ` as status column.
        if line.contains("| open |") {
            count += 1;
        }
    }
    count
}

/// Parse all rows under the `## Rows` heading from inbox markdown content.
///
/// Behavior:
/// - Returns empty `Vec` if `## Rows` heading is absent (tolerate-empty per spec).
/// - Skips blank lines, HTML-comment blocks (`<!-- ... -->`), column-header row
///   (`| Date | Advisory ID | ... |`), and separator row (`|---...|`).
/// - Stops at next `## ` heading after `## Rows` (preserves future schema extension).
/// - Returns [`InboxError::ParseRow`] with `path: PathBuf::new()` placeholder on row
///   parse failure — CALLER (e.g., `cli/state_backfill.rs`) MUST re-wrap with real
///   path before bubbling to `main.rs`.
///
/// First consumer: P008 `state-backfill` subcmd.
pub fn parse_rows(content: &str) -> Result<Vec<crate::row::AdvisoryRow>, InboxError> {
    let lines: Vec<&str> = content.lines().collect();
    let heading_idx = match lines.iter().position(|l| l.trim_end() == "## Rows") {
        Some(idx) => idx,
        None => return Ok(Vec::new()), // tolerate-empty per locked decision
    };

    let mut rows = Vec::new();
    let mut in_comment = false;

    for (offset, &line) in lines.iter().enumerate().skip(heading_idx + 1) {
        let line_number = offset + 1; // 1-based line index in full content

        // Stop at next `## ` or `# ` heading (start of new section).
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("## ") || trimmed_start.starts_with("# ") {
            break;
        }

        // HTML comment block toggle (multi-line or same-line open+close).
        let starts_comment = line.contains("<!--");
        let ends_comment = line.contains("-->");
        if in_comment {
            if ends_comment {
                in_comment = false;
            }
            continue;
        }
        if starts_comment {
            if !ends_comment {
                in_comment = true;
            }
            // Both same-line open+close AND multi-line open: skip this line.
            continue;
        }

        // Skip blank lines.
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip separator row `|---...|`.
        if trimmed.starts_with("|---") || trimmed.starts_with("| ---") {
            continue;
        }

        // Skip column-header row `| Date | Advisory ID | ... |`.
        if trimmed.starts_with("| Date |") || trimmed.starts_with("|Date|") {
            continue;
        }

        // Treat as pipe-row; parse via `row::parse_row`.
        match crate::row::parse_row(line) {
            Ok(row) => rows.push(row),
            Err(source) => {
                return Err(InboxError::ParseRow {
                    path: PathBuf::new(), // placeholder; caller fills with real path
                    line_number,
                    source,
                });
            }
        }
    }

    Ok(rows)
}

/// Atomically write `content` to `path` per INV-LOCAL-002 protocol:
/// temp file in SAME parent directory → fsync data+metadata → atomic rename.
///
/// # Protocol (INV-LOCAL-002 — do NOT deviate)
/// 1. `NamedTempFile::new_in(parent)` — same filesystem ensures rename is atomic.
/// 2. `write_all` — buffer entire content into temp file.
/// 3. `as_file().sync_all()` — fsync data + metadata; kernel cannot reorder write+rename.
/// 4. `persist(target)` — atomic rename replacing target.
///
/// # Forbidden alternatives (INV-LOCAL-002)
/// - `OpenOptions::append(true)` against target
/// - `std::fs::write` direct (no temp+rename)
/// - `std::fs::rename` outside `tempfile::persist`
///
/// # Errors
/// Returns [`InboxError::Io`] on any I/O failure (temp creation, write, fsync, rename).
pub fn write_atomic(path: &Path, content: &str) -> Result<(), InboxError> {
    let parent = path.parent().ok_or_else(|| InboxError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "target path has no parent directory",
        ),
    })?;
    let mut temp = NamedTempFile::new_in(parent).map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.write_all(content.as_bytes())
        .map_err(|source| InboxError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    // Flush buffered writes before rename.
    temp.flush().map_err(|source| InboxError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    temp.persist(path).map_err(|e| InboxError::Io {
        path: path.to_path_buf(),
        source: e.error,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    use crate::row::{Severity, Status};

    fn sample_row(advisory_id: &str, status: Status) -> AdvisoryRow {
        AdvisoryRow {
            date: NaiveDate::from_ymd_opt(2026, 5, 28).unwrap(),
            advisory_id: advisory_id.to_string(),
            source_url: format!("https://example.test/{advisory_id}"),
            package: "pkg@<1.0".to_string(),
            file_line: "src/foo.rs:1".to_string(),
            severity: Severity::High,
            status,
            note: "-".to_string(),
        }
    }

    /// Baseline inbox with 2 existing rows (1 open, 1 processed).
    const BASELINE: &str = "# Advisory Inbox\n\n## Rows\n\n\
| 2026-05-20 | CVE-OLD-1 | https://example.test/CVE-OLD-1 | pkg@<0.9 | src/old.rs:1 | High | open | - |\n\
| 2026-05-19 | CVE-OLD-2 | https://example.test/CVE-OLD-2 | pkg@<0.8 | src/old.rs:2 | Medium | processed | - |\n";

    #[test]
    fn insert_rows_happy_path() {
        let rows = vec![
            sample_row("CVE-NEW-1", Status::Open),
            sample_row("CVE-NEW-2", Status::Open),
        ];
        let path = Path::new("inbox.md");
        let (out, total_open) = insert_rows(BASELINE, &rows, path).expect("insert ok");

        // Both new rows present.
        assert!(out.contains("CVE-NEW-1"));
        assert!(out.contains("CVE-NEW-2"));
        // Old rows still present.
        assert!(out.contains("CVE-OLD-1"));
        assert!(out.contains("CVE-OLD-2"));
        // Order: CVE-NEW-1 (rows[0]) appears BEFORE CVE-OLD-1 in output.
        let pos_new1 = out.find("CVE-NEW-1").unwrap();
        let pos_old1 = out.find("CVE-OLD-1").unwrap();
        assert!(pos_new1 < pos_old1, "rows[0] should be topmost");
        // total_open: CVE-OLD-1 open + CVE-NEW-1 open + CVE-NEW-2 open = 3.
        // CVE-OLD-2 is processed — not counted.
        assert_eq!(total_open, 3);
    }

    #[test]
    fn insert_rows_missing_heading_errors() {
        let no_heading = "# Advisory Inbox\n\nSome text but no Rows heading.\n";
        let rows = vec![sample_row("CVE-X", Status::Open)];
        let path = Path::new("inbox.md");
        let err = insert_rows(no_heading, &rows, path).unwrap_err();
        assert!(matches!(err, InboxError::MissingRowsHeading { .. }));
    }

    #[test]
    fn insert_rows_empty_rows_noop() {
        let path = Path::new("inbox.md");
        let (out, total_open) = insert_rows(BASELINE, &[], path).expect("insert ok empty");
        // Content structurally preserved (all original lines present).
        for old_line in BASELINE.lines() {
            assert!(out.contains(old_line), "old line missing: {old_line}");
        }
        // total_open = 1 (only CVE-OLD-1 is open).
        assert_eq!(total_open, 1);
    }

    #[test]
    fn write_atomic_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("inbox.md");
        let content = "# Advisory Inbox\n\n## Rows\n\n| 2026-05-28 | CVE-X | u | p | f:1 | High | open | - |\n";
        write_atomic(&target, content).expect("write atomic ok");
        // File exists with expected content.
        let read_back = std::fs::read_to_string(&target).expect("read back");
        assert_eq!(read_back, content);
        // No leftover temp files in parent dir after persist.
        let leftover_count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".tmp")
            })
            .count();
        assert_eq!(leftover_count, 0, "leftover .tmp files found in parent dir");
    }

    #[test]
    fn count_open_skips_html_comment_block() {
        // Placeholder row inside HTML comment block must NOT be counted.
        let with_comment = "## Rows\n\n\
| 2026-05-28 | CVE-A | u | p | f:1 | High | open | - |\n\n\
<!--\n\
| 2026-05-23 | CVE-PLACEHOLDER | u | p | f:1 | Medium | open | - |\n\
-->\n";
        // Only CVE-A counted — the placeholder is inside the comment block.
        assert_eq!(count_open_rows(with_comment), 1);
    }

    // --- parse_rows tests (P008) ---

    #[test]
    fn parse_rows_happy_3_rows() {
        let content = "# Inbox\n\
## Rows\n\
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |\n\
|------|-------------|-----------|---------|-----------|----------|--------|------|\n\
| 2026-05-28 | CVE-2026-1 | https://x.com/1 | pkg1@<1 | f.rs:1 | High | open | - |\n\
| 2026-05-28 | CVE-2026-2 | https://x.com/2 | pkg2@<2 | f.rs:2 | Medium | processed | reviewed |\n\
| 2026-05-28 | CVE-2026-3 | https://x.com/3 | pkg3@<3 | f.rs:3 | Low | dismissed | n/a |\n";
        let rows = parse_rows(content).expect("parse rows");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].advisory_id, "CVE-2026-1");
        assert_eq!(rows[2].advisory_id, "CVE-2026-3");
    }

    #[test]
    fn parse_rows_empty_section() {
        // `## Rows` present but no data rows — only header + separator.
        let content = "# Inbox\n\
## Rows\n\
\n\
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |\n\
|------|-------------|-----------|---------|-----------|----------|--------|------|\n";
        let rows = parse_rows(content).expect("parse rows empty");
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn parse_rows_skips_html_comment() {
        let content = "# Inbox\n\
## Rows\n\
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |\n\
|------|-------------|-----------|---------|-----------|----------|--------|------|\n\
| 2026-05-28 | CVE-2026-1 | https://x.com/1 | pkg1@<1 | f.rs:1 | High | open | - |\n\
<!--\n\
| 2026-05-23 | GHSA-skip | https://x.com/skip | pkg@<x | indirect | Medium | open | - |\n\
-->\n\
| 2026-05-28 | CVE-2026-2 | https://x.com/2 | pkg2@<2 | f.rs:2 | Medium | processed | reviewed |\n";
        let rows = parse_rows(content).expect("parse rows with comment");
        assert_eq!(rows.len(), 2, "comment block row must be skipped");
        assert!(rows.iter().all(|r| r.advisory_id != "GHSA-skip"));
    }

    #[test]
    fn parse_rows_bad_row_returns_parse_row_error() {
        // 5 columns instead of 8 — row::parse_row should fail.
        let content = "## Rows\n\
| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |\n\
|------|-------------|-----------|---------|-----------|----------|--------|------|\n\
| 2026-05-28 | CVE-2026-1 | bad-row-only-5-cols | High | open |\n";
        let err = parse_rows(content).expect_err("should error on malformed row");
        assert!(
            matches!(err, InboxError::ParseRow { .. }),
            "expected ParseRow variant, got {:?}",
            err
        );
    }

    #[test]
    fn parse_rows_no_heading_returns_empty() {
        // No `## Rows` heading — tolerate-empty per locked decision.
        let content = "# Inbox\n\nNo rows section here.\n";
        let rows = parse_rows(content).expect("tolerate missing heading");
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn parse_rows_stops_at_next_heading() {
        // Content after next `## ` heading must NOT be parsed as rows.
        let content = "## Rows\n\
| 2026-05-28 | CVE-2026-1 | https://x.com/1 | pkg1@<1 | f.rs:1 | High | open | - |\n\
| 2026-05-28 | CVE-2026-2 | https://x.com/2 | pkg2@<2 | f.rs:2 | Medium | processed | reviewed |\n\
## Archive\n\
| 2026-05-28 | CVE-2026-3 | https://x.com/3 | pkg3@<3 | f.rs:3 | Low | dismissed | n/a |\n";
        let rows = parse_rows(content).expect("parse rows stops at heading");
        // Only 2 rows before `## Archive`; CVE-2026-3 must NOT be included.
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.advisory_id != "CVE-2026-3"));
    }
}
