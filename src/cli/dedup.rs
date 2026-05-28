//! Stub for `dedup` subcommand. Real logic wired in P005.

use anyhow::Result;
use std::path::PathBuf;

pub fn run(state: PathBuf, rows_json: PathBuf) -> Result<()> {
    println!(
        "TODO: dedup (state={:?}, rows_json={:?}) — wired in P005",
        state, rows_json
    );
    Ok(())
}
