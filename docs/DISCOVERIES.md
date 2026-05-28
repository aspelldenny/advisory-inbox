# DISCOVERIES — advisory-inbox

> Operational evidence log. Soft cap < 1000 dòng. Rotate khi vượt → `docs/Archive/DISCOVERIES_ARCHIVE.md`.
>
> Per-phiếu detail: `docs/discoveries/P<NNN>.md`. This file is 1-line index, newest at top.

---

<!-- Entries appended here, newest at top. Format:
- YYYY-MM-DD P<NNN>: <one-line summary>, <key finding> → see docs/discoveries/P<NNN>.md
-->

- 2026-05-28 P003: sentinel.rs shipped (6 tests pass, str::find for literal markers), #![allow(dead_code)] per P002 pattern, eprintln! multiple-START warn is intentional operational stderr (DoD item 5 exempt) → see docs/discoveries/P003.md
- 2026-05-28 P002: row.rs + state.rs scaffold types shipped (8 tests pass), dead_code lint on unused pub types requires #![allow(dead_code)] until P004+ wire-in → see docs/discoveries/P002.md
- 2026-05-28 P001: scaffold CLI surface shipped (8 subcmd, clap derive, sync main), rustfmt wraps 3-field match arms multi-line → see docs/discoveries/P001.md
