# DISCOVERIES — advisory-inbox

> Operational evidence log. Soft cap < 1000 dòng. Rotate khi vượt → `docs/Archive/DISCOVERIES_ARCHIVE.md`.
>
> Per-phiếu detail: `docs/discoveries/P<NNN>.md`. This file is 1-line index, newest at top.

---

<!-- Entries appended here, newest at top. Format:
- YYYY-MM-DD P<NNN>: <one-line summary>, <key finding> → see docs/discoveries/P<NNN>.md
-->

- 2026-05-28 P013: tarot install complete — /advisory-scan 142→27 lines, binary v0.1.0 installed via cargo install --path (clean build ~51s), state file Path C jq prestep schema_version added (2 CVEs preserved), smoke test Approach B exit 0 appended=1 dedup-rerun skipped=1, Sub-mech A/B/C/D/E/F all green; V1→V2 debate caught 3rd state format (O1.1 shape) + --report stdin idiom (O1.2 mechanical); discovery: 10-col tarot inbox vs 8-col binary parse (advisory-watch emits 8-col per handbook — not a blocker); tarot PR https://github.com/aspelldenny/tarot/pull/579 → see docs/discoveries/P013.md
- 2026-05-28 P012: release polish shipped (README 243→158 lines, ARCHITECTURE §5 consolidated + ASCII flow diagram, cargo publish --dry-run exit 0 (116 files 1.3 MiB / 414.8 KiB compressed), rust-version=1.85 added to Cargo.toml, no code change, 69 tests preserved) → see docs/discoveries/P012.md
- 2026-05-28 P011: MCP tool dispatch shipped (6 tools via #[tool_router]+#[tool_handler], schemars=1.0 dep added, rmcp::from_build_env() reads rmcp crate name not ours → manual get_info() required, Parameters/Json at handler::server::wrapper not router::tool, Strategy B extract for append+scan_and_append, 69 tests total, binary ~2.16 MB) → see docs/discoveries/P011.md
- 2026-05-28 P010: serve subcmd wired (rmcp 1.7.0 stdio handshake, first tokio current_thread runtime, Implementation::new() + ServerInfo builder used, waiting() returns Result<QuitReason,JoinError> not (), ServerCapabilities empty until P011, 65 tests total, binary ~1.96 MB) → see docs/discoveries/P010.md
- 2026-05-28 P009: scan-and-append composite wired (INV-LOCAL-002 4th caller — inbox::write_atomic + state::write_atomic in same invocation), NOT cross-file atomic (inbox-first write order; recovery = state-backfill), 3 integration tests pass, 62 tests total → see docs/discoveries/P009.md
- 2026-05-28 P008: state-backfill wired (INV-LOCAL-002 third user — state::write_atomic), inbox::parse_rows added + InboxError::ParseRow variant, 4 IDs union from fixture (Sub-mech C pass), dry-run byte-identity verified (Sub-mech F), 59 tests total → see docs/discoveries/P008.md
- 2026-05-28 P007: migrate-state wired (INV-LOCAL-002 second user — state::write_atomic), legacy ISO→JSON v1 with timestamp preserved (Sub-mech C), 2 mechanical deviations (predicates trait import + clippy io_other_error), 49 tests total → see docs/discoveries/P007.md
- 2026-05-28 P006: append wired (INV-LOCAL-002 atomic write first user), insert_rows signature adds path param for cleaner errors, ARCHITECTURE §7 flush→sync_all doc drift fixed, 41 tests total → see docs/discoveries/P006.md
- 2026-05-28 P005: dedup wired (state + rows JSON → kept/skipped/observed_ids), state::read enforces schema_version==1, #![allow(dead_code)] removed from state.rs, anyhow downcast maps exit codes (StateReadError→1, else→2) → see docs/discoveries/P005.md
- 2026-05-28 P004: parse-report wired (stdin/--input → sentinel → row → JSON), #![allow(dead_code)] removed from row.rs, anyhow downcast maps exit codes (SentinelError→1, other→2), clippy if_same_then_else collapsed → see docs/discoveries/P004.md
- 2026-05-28 P003: sentinel.rs shipped (6 tests pass, str::find for literal markers), #![allow(dead_code)] per P002 pattern, eprintln! multiple-START warn is intentional operational stderr (DoD item 5 exempt) → see docs/discoveries/P003.md
- 2026-05-28 P002: row.rs + state.rs scaffold types shipped (8 tests pass), dead_code lint on unused pub types requires #![allow(dead_code)] until P004+ wire-in → see docs/discoveries/P002.md
- 2026-05-28 P001: scaffold CLI surface shipped (8 subcmd, clap derive, sync main), rustfmt wraps 3-field match arms multi-line → see docs/discoveries/P001.md
