//! Stub for `migrate-state` subcommand. Real logic wired in P007.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, dry_run: bool) -> Result<()> {
    println!(
        "TODO: migrate-state (state={:?}, dry_run={}) — wired in P007",
        state, dry_run
    );
    Ok(())
}
