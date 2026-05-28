# BACKLOG — advisory-inbox

> Sprint pipeline. Mỗi phiếu = 1 PR. Phiếu order theo dependency.

---

## Active Sprint — Phase 1: Core CLI

### P001 — Scaffold CLI surface (clap derive)

- **Lane:** Normal (scaffold per RULES.md §1)
- **Tầng:** 1
- **Scope:** `src/main.rs` clap derive parse, 8 subcmd registered (no logic, just parse + exit 0). Module skeleton `src/cli/mod.rs`.
- **Acceptance:** `advisory-inbox --help` shows 8 subcmd. `advisory-inbox parse-report` exits 0 with TODO message.
- **Sub-mech checks:** B (cargo check), D (RULES.md §1 docs grep).

### P002 — Row + state types (serde)

- **Lane:** Normal
- **Tầng:** 1
- **Scope:** `src/row.rs` `AdvisoryRow` struct (8 fields per ARCHITECTURE §3). `src/state.rs` `StateFile` struct (schema_version, last_scan_at, seen_advisories, agent_version). Serde derive. Unit tests roundtrip.
- **Acceptance:** Type compile clean, roundtrip JSON test passes.
- **Sub-mech checks:** B, C (state schema_version bump rule).

### P003 — Sentinel parser

- **Lane:** Normal
- **Tầng:** 1
- **Scope:** `src/sentinel.rs` regex extract block between `<!-- INBOX_APPEND_START -->` and `<!-- INBOX_APPEND_END -->`. Handle: missing markers, empty block, multiple markers (use first pair + warn).
- **Acceptance:** 5 unit tests cover all cases. Fixture: real agent report.
- **Sub-mech checks:** B.

### P004 — parse-report subcmd

- **Lane:** Normal
- **Tầng:** 1
- **Scope:** Wire `cli/parse_report.rs` → sentinel parse → row parse → JSON stdout.
- **Acceptance:** `cargo run -- parse-report < fixtures/agent-report-1.md` outputs JSON matching expected.
- **Sub-mech checks:** B, D.

### P005 — dedup subcmd

- **Lane:** Normal
- **Tầng:** 1
- **Scope:** Wire `cli/dedup.rs` → read state JSON → filter rows → JSON stdout. Preserve `observed_ids[]` for state update.
- **Acceptance:** Fixture state with 3 IDs + report with 5 rows (2 match) → output `kept: 3, skipped: 2`.
- **Sub-mech checks:** B, C (state schema check).

### P006 — append subcmd (atomic write)

- **Lane:** Guarded (filesystem write per RULES.md §1)
- **Tầng:** 1
- **Scope:** Wire `cli/append.rs` → read inbox markdown → insert rows after `## Rows` heading → atomic write temp+rename.
- **Acceptance:** Fixture inbox + 2 rows → write OK, rows at top of `## Rows` section, original rows preserved. Atomic write verified (interrupt mid-write doesn't corrupt).
- **Sub-mech checks:** B, D, F (no token leak to logs).

---

## Phase 2 — State Machine

### P007 — migrate-state subcmd

- **Lane:** Guarded (legacy data migration)
- **Tầng:** 1
- **Scope:** Detect format (JSON / single-line ISO / missing) → write JSON v1 schema → preserve `last_scan_at` from legacy if present.
- **Acceptance:** 3 fixtures (json/legacy/missing) → all migrate clean.
- **Sub-mech checks:** B, C (migration completeness — count preserved).

### P008 — state-backfill subcmd

- **Lane:** Guarded
- **Tầng:** 1
- **Scope:** Extract advisory IDs from inbox rows status `processed`/`dismissed` → union with existing `seen_advisories[]`. P286 of tarot precedent.
- **Acceptance:** Fixture inbox 5 rows (3 processed) + state 1 ID → output state has 4 IDs.
- **Sub-mech checks:** B, C.

### P009 — scan-and-append composite

- **Lane:** Guarded
- **Tầng:** 1
- **Scope:** Compose 3 subcmd (parse → dedup → append + state update) in 1 atomic operation.
- **Acceptance:** End-to-end fixture: agent report + initial state + initial inbox → final state + final inbox match expected.
- **Sub-mech checks:** B, C, F.

---

## Phase 3 — MCP

### P010 — serve subcmd (rmcp stdio)

- **Lane:** Guarded (MCP transport per RULES.md §1)
- **Tầng:** 1
- **Scope:** `cli/serve.rs` start rmcp server with stdio transport. No tools yet, just handshake.
- **Acceptance:** Send JSON-RPC `initialize` → response valid per MCP spec.
- **Sub-mech checks:** A (MCP trigger fires from `.mcp.json`), B (rmcp API check via context7).

### P011 — MCP tool dispatch (6 tools)

- **Lane:** Guarded
- **Tầng:** 1
- **Scope:** `mcp/tools.rs` register 6 tools per ARCHITECTURE §6. Schema validate input. Reuse subcmd logic (no duplicate).
- **Acceptance:** Each tool callable via JSON-RPC, returns expected output. MCP error format on failure.
- **Sub-mech checks:** A, B, D.

---

## Phase 4 — Ship

### P012 — README + ARCHITECTURE polish + crates.io publish

- **Lane:** Normal (docs polish, không touch code per RULES.md §1)
- **Tầng:** 2
- **Scope:** README with quick-start. ARCHITECTURE diagram. `cargo publish --dry-run` clean.
- **Acceptance:** README < 200 lines, ARCHITECTURE complete.

### P013 — Install in tarot (replace 142-line Bash heredoc)

- **Lane:** Guarded (changes tarot security slash command)
- **Tầng:** 1
- **Scope:** In tarot repo: install `advisory-inbox` binary, rewrite `.claude/commands/advisory-scan.md` to call binary (5-10 lines), remove Bash heredoc (~135 lines).
- **Acceptance:** Tarot `/advisory-scan` test fire → same output as before. Smoke test against last scan.
- **Sub-mech checks:** A (slash command fires), C (state file compat preserved).

---

## Defer / Future (NOT MVP)

- Concurrency lock (multi-process state write safety)
- Inbox archive rotation (rows > 90 days dismissed → archive file)
- Watch mode (tail-follow state file)
- Severity threshold filter (`--min-severity High`)
- Plugin architecture (1 binary 1 job — REJECT mãi)
- Web UI dashboard (markdown đủ — REJECT)
- Auto-PR creator (Sếp giữ van người-gate — REJECT mãi)

---

## Recently shipped

(empty — Phase 1 not yet started)

---

## Sprint progress

- 🚧 Bootstrap (2026-05-28) — scaffold workflow v2.1, no code yet
- ⬜ P001 — scaffold CLI surface
- ⬜ P002 — row/state types
- ⬜ ...
