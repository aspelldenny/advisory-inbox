# PROJECT — advisory-inbox

## Vision (1 câu)

Rust binary thay 142-line Bash heredoc trong `/advisory-scan` slash command — parse agent report, dedup, append inbox, migrate state — dual mode CLI + MCP để Claude Code session gọi mượt thay vì nhớ jq/awk pattern.

## Why this exists

Trong tarot, slash command `/advisory-scan` đã rebuild **4 lần** (P282/P284/P285/P286 of soulsign tarot) vì Bash quote escape + jq arg passing + awk skip HTML comment fragile. Mỗi fix thêm 30-50 dòng Bash heredoc trong markdown. LLM phải nhớ pattern đầy đủ khi spawn agent.

advisory-inbox replace toàn bộ logic Bash bằng Rust binary deterministic:
- **Input contract:** structured (stdin agent report markdown sentinel block hoặc CLI args)
- **Output contract:** structured JSON (stdout) hoặc file write atomic
- **Error contract:** exit code + structured stderr
- **Test contract:** `cargo test` deterministic, không depend shell version

LLM gọi `advisory-inbox <subcmd>` thay vì compose 142 dòng Bash. Slash command markdown shrink → 5-10 dòng.

## Scope cứng

### IN scope
- 6 subcommand:
  1. `parse-report` — extract rows từ agent report sentinel block (stdin)
  2. `dedup` — filter rows against state `seen_advisories[]`
  3. `append` — insert rows after `## Rows` heading trong inbox markdown (atomic write)
  4. `migrate-state` — legacy single-line ISO → JSON schema (P282 of tarot precedent)
  5. `state-backfill` — extract IDs từ inbox rows → state `seen_advisories[]` (P286 of tarot precedent)
  6. `serve` — MCP server stdio JSON-RPC, expose 5 above + 1 composite `scan-and-append`
- State file JSON schema: `{last_scan_at, seen_advisories[], agent_version}`
- Inbox markdown format: `## Rows` heading + pipe-delimited row table
- Sentinel marker: `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->`

### OUT scope (NOT building)
- Network fetch (agent fetches advisory, binary chỉ process)
- Telegram alert (advisory-cron handles, separate binary)
- Cron scheduling (advisory-cron handles)
- Multi-source advisory merge (current scope: 1 inbox, 1 state)
- Auto-PR creator cho dismissed advisory (Sếp giữ van người-gate)
- Web UI dashboard (markdown đủ)
- Plugin architecture (1 binary 1 job)

## Personas

- **Sếp (Chủ nhà)** — review inbox markdown, gạt row "dismissed" hoặc create phiếu fix
- **Orchestrator (Quản đốc Claude Code session)** — invoke binary qua CLI hoặc MCP để process agent output
- **Advisory-watch agent (Trinh sát)** — emit report với sentinel block, KHÔNG cầm binary
- **advisory-cron (separate)** — fire scheduled scan, output feed vào advisory-inbox

## Success criteria

1. **CLI mode:** `advisory-inbox parse-report < agent-output.md` → JSON stdout với rows extracted
2. **CLI mode:** `advisory-inbox scan-and-append --report <stdin> --inbox <path> --state <path>` → composite of 5 subcmd
3. **MCP mode:** `advisory-inbox serve` → JSON-RPC 2.0 stdio, 6 tools exposed
4. **Test:** `cargo test --all` ≥ 30 tests pass
5. **Smoke test:** real agent report (export từ tarot `/advisory-scan` last run) → process clean, output match expected
6. **Migration:** legacy state file (single-line ISO) → JSON schema OK, no data loss
7. **Binary size:** `< 5 MB` release build (strip + lto)

## Tech Stack

- Rust edition 2024, MSRV 1.85
- clap 4.x derive
- serde + serde_json
- chrono (ISO-8601)
- tokio (only for MCP `serve`)
- tempfile (atomic write temp + rename)
- regex (sentinel parser)
- rmcp 1.7.0 (MCP server)
- assert_cmd + predicates (CLI integration tests)

## Roadmap / Phases

### Phase 1 — Core CLI (P001-P006)
- P001 scaffold (CLI surface skeleton, clap derive)
- P002 row/state schema types (serde)
- P003 sentinel parser (regex extract `<!-- INBOX_APPEND_START/END -->`)
- P004 parse-report subcmd
- P005 dedup subcmd
- P006 append subcmd (atomic write)

### Phase 2 — State machine (P007-P009)
- P007 migrate-state subcmd
- P008 state-backfill subcmd
- P009 scan-and-append composite (compose 5 subcmd)

### Phase 3 — MCP (P010-P011)
- P010 serve subcmd (rmcp stdio JSON-RPC)
- P011 MCP tool dispatch (6 tools, schema validate)

### Phase 4 — Ship (P012-P013)
- P012 README + ARCHITECTURE polish + crates.io publish
- P013 install in tarot — replace 142-line Bash heredoc

### Phase 5 — Retrospective (post-pilot)
- Feed back to `~/sos-kit/docs/WORKFLOW_V2.1_RETRO_<date>.md`
- Patch sos-kit golden template
- Re-port to claude-hooks + inv-gate

## Constraints

- KHÔNG fetch network (agent handles)
- KHÔNG depend OS-specific (cross-platform macOS + Linux)
- KHÔNG break inbox format compat (tarot existing inbox files phải đọc được)
- KHÔNG break state file compat (migration handles legacy → JSON)

## Status

🚧 **Bootstrap (2026-05-28).** Phase 1 not yet shipped.
