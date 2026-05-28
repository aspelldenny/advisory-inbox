//! Stub for `scan-and-append` composite subcommand. Real logic wired in P009.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(report: Option<PathBuf>, inbox: PathBuf, state: PathBuf) -> Result<()> {
    println!(
        "TODO: scan-and-append (report={:?}, inbox={:?}, state={:?}) — wired in P009",
        report, inbox, state
    );
    Ok(())
}
