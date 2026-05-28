//! Stub for `init` subcommand. Real logic wired in a Phase 1 follow-up phiếu.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(inbox_path: Option<PathBuf>, state_path: Option<PathBuf>) -> Result<()> {
    println!(
        "TODO: init (inbox_path={:?}, state_path={:?}) — wired in Phase 1 follow-up",
        inbox_path, state_path
    );
    Ok(())
}
