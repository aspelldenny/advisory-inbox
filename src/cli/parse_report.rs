//! `advisory-inbox parse-report` — extract sentinel block from agent report markdown,
//! parse each row into [`AdvisoryRow`], emit JSON to stdout.
//!
//! See ARCHITECTURE.md §1 subcmd `parse-report` for the I/O contract.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::row::{self, AdvisoryRow};
use crate::sentinel;

/// Read agent report (stdin or `--input <FILE>`), parse rows, emit JSON.
///
/// Output JSON shape:
/// ```text
/// { "rows": [...], "stack_scanned": {}, "advisories_found": N }
/// ```
///
/// `stack_scanned` is currently always `{}` — future phiếu will populate from
/// the report `**Stack scanned:**` section.
///
/// # Errors
/// - [`sentinel::SentinelError`] (mapped to exit 1 in `main.rs`) if markers missing.
/// - [`row::RowParseError`] (mapped to exit 2) if any row line fails to parse.
/// - I/O errors (read stdin/file) → anyhow bubble.
pub fn run(input: Option<PathBuf>) -> Result<()> {
    // 1. Read source text.
    let text = match input {
        Some(path) => std::fs::read_to_string(&path)
            .with_context(|| format!("read input file {}", path.display()))?,
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .lock()
                .read_to_string(&mut buf)
                .context("read stdin")?;
            buf
        }
    };

    // 2. Extract sentinel block (SentinelError bubble → main maps to exit 1).
    let raw_lines = sentinel::extract_block(&text)?;

    // 3. Parse each line into AdvisoryRow (RowParseError bubble → main maps to exit 2).
    let mut rows: Vec<AdvisoryRow> = Vec::with_capacity(raw_lines.len());
    for line in &raw_lines {
        let row = row::parse_row(line)?;
        rows.push(row);
    }

    // 4. Build + emit output JSON (compact, single line + trailing newline).
    let out = serde_json::json!({
        "rows": rows,
        "stack_scanned": {},
        "advisories_found": rows.len(),
    });
    serde_json::to_writer(std::io::stdout().lock(), &out).context("write stdout JSON")?;
    println!();
    Ok(())
}
