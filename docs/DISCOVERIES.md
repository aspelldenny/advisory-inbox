# DISCOVERIES — advisory-inbox

> Operational evidence log. Soft cap < 1000 dòng. Rotate khi vượt → `docs/Archive/DISCOVERIES_ARCHIVE.md`.
>
> Per-phiếu detail: `docs/discoveries/P<NNN>.md`. This file is 1-line index, newest at top.

---

<!-- Entries appended here, newest at top. Format:
- YYYY-MM-DD P<NNN>: <one-line summary>, <key finding> → see docs/discoveries/P<NNN>.md
-->

- 2026-05-28 P008: state-backfill wired (INV-LOCAL-002 third user — state::write_atomic), inbox::parse_rows added + InboxError::ParseRow variant, 4 IDs union from fixture (Sub-mech C pass), dry-run byte-identity verified (Sub-mech F), 59 tests total → see docs/discoveries/P008.md
- 2026-05-28 P007: migrate-state wired (INV-LOCAL-002 second user — state::write_atomic), legacy ISO→JSON v1 with timestamp preserved (Sub-mech C), 2 mechanical deviations (predicates trait import + clippy io_other_error), 49 tests total → see docs/discoveries/P007.md
- 2026-05-28 P006: append wired (INV-LOCAL-002 atomic write first user), insert_rows signature adds path param for cleaner errors, ARCHITECTURE §7 flush→sync_all doc drift fixed, 41 tests total → see docs/discoveries/P006.md
- 2026-05-28 P005: dedup wired (state + rows JSON → kept/skipped/observed_ids), state::read enforces schema_version==1, #![allow(dead_code)] removed from state.rs, anyhow downcast maps exit codes (StateReadError→1, else→2) → see docs/discoveries/P005.md
- 2026-05-28 P004: parse-report wired (stdin/--input → sentinel → row → JSON), #![allow(dead_code)] removed from row.rs, anyhow downcast maps exit codes (SentinelError→1, other→2), clippy if_same_then_else collapsed → see docs/discoveries/P004.md
- 2026-05-28 P003: sentinel.rs shipped (6 tests pass, str::find for literal markers), #![allow(dead_code)] per P002 pattern, eprintln! multiple-START warn is intentional operational stderr (DoD item 5 exempt) → see docs/discoveries/P003.md
- 2026-05-28 P002: row.rs + state.rs scaffold types shipped (8 tests pass), dead_code lint on unused pub types requires #![allow(dead_code)] until P004+ wire-in → see docs/discoveries/P002.md
- 2026-05-28 P001: scaffold CLI surface shipped (8 subcmd, clap derive, sync main), rustfmt wraps 3-field match arms multi-line → see docs/discoveries/P001.md
