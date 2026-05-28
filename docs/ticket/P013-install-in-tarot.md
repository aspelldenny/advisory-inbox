# PHIẾU P013: Install `advisory-inbox` into tarot — replace 142-line Bash heredoc in `/advisory-scan`

> **ID format:** `P013` — counter `.phieu-counter` = 13 sau P012 ship.
> **Filename:** `docs/ticket/P013-install-in-tarot.md`
> **Branch (advisory-inbox repo):** *none* — phiếu file lives here, but NO advisory-inbox code change ships; only `docs/discoveries/P013.md` is added on `main` (or a tiny `docs/P013-tarot-install` branch if PR preferred).
> **Branch (tarot repo):** `feat/advisory-inbox-binary-install`

---

> **Loại:** infra (cross-repo install — replaces 142-line Bash heredoc in tarot's security slash command with a thin invocation of the `advisory-inbox` binary built across P001-P012)
> **Tầng:** 1 (touches a security slash command — `/advisory-scan` — per CLAUDE.md "Touch security boundary → AUTO Tầng 1" + RULES.md §11 "Security boundary touched → AUTO Tầng 1")
> **Ưu tiên:** P1 (closes Phase 4 — advisory-inbox MVP complete only when tarot actually invokes the binary; until then the 142-line Bash heredoc remains the source-of-truth and the whole `advisory-inbox` build is shelfware)
> **Ảnh hưởng:**
> - **tarot repo (code change):** `~/tarot/.claude/commands/advisory-scan.md` (rewrite — 142 lines → ~15 lines body). `~/tarot/CHANGELOG.md` or `~/tarot/docs/CHANGELOG.md` (1-line entry — Worker locates which one tarot uses).
> - **tarot repo (one-time runtime, if needed):** `~/tarot/docs/security/.advisory-scan-state` (Sub-mech C migration: legacy single-line ISO → JSON v1 ONLY IF Worker Anchor #6 verifies legacy format; OR JSON-without-schema_version → JSON v1 via `jq` pre-step per Decision #3 Path C added in V2).
> - **advisory-inbox repo (docs only):** `docs/discoveries/P013.md` + 1-line index entry in `docs/DISCOVERIES.md`. NO `src/` change. NO `Cargo.toml` change. NO test change.
> - **Local machine (Worker's box):** `~/.cargo/bin/advisory-inbox` installed via `cargo install --path /Users/nguyenhuuanh/advisory-inbox --locked` (Sếp's box; not committed anywhere).
> **Dependency:**
> - P001-P012 (all advisory-inbox MVP shipped — CLI surface complete, `cargo publish --dry-run` clean per P012, 69 tests pass).
> - Specifically: P009 (`scan-and-append` composite — the subcmd the new slash command calls), P007 (`migrate-state` — needed if tarot state is legacy format).
> - **NOT dependent on crates.io publish.** Install path is `cargo install --path ...` from local source (per user brief — advisory-inbox not yet on crates.io; this phiếu validates end-to-end before that decision).
> **Lane:** **Guarded** (per RULES.md §1 — touches `~/tarot/.claude/commands/advisory-scan.md` which is a security slash command; cross-repo install operation; runtime state file format compat must be preserved). Flow: `Architect full → Worker CHALLENGE → RESPOND → Worker Turn 2 surgical → Execute → security-review-full → approval gate`. Worker MUST run `/security-review <PR>` on the tarot PR before merge.
> **Sub-mech áp dụng:**
> - **A — Trigger gap** (slash command fires). The whole point of the phiếu. Verify post-rewrite: invoking `/advisory-scan` in tarot actually runs the new binary path, exits 0, and produces the expected JSON output. Without this check, the new wrapper could be a no-op and Worker would mark "done" while nothing actually replaced the heredoc.
> - **B — Capability gap** (binary builds + installs + handles real input). `cargo install --path . --locked` exits 0; `advisory-inbox --version` returns `0.1.0`; smoke-test `scan-and-append` on a real fixture (tarot's last agent report export) produces same logical output as the old heredoc.
> - **C — Migration completeness** (state file compat). Tarot's current `.advisory-scan-state` MUST remain readable by the new binary. Either it's already JSON v1 (no action) OR JSON-without-schema_version (one-time `jq` pre-step per Decision #3 Path C) OR legacy single-line ISO (one-time `advisory-inbox migrate-state` run before first new scan). `seen_advisories[]` count post-install ≥ pre-install (NEVER less — Hard Stop if migration loses entries).
> - **D — Persistence lifecycle** (the install knowledge persists). Tarot's CHANGELOG entry + tarot's new `/advisory-scan` body itself document the binary dependency; advisory-inbox's `docs/discoveries/P013.md` documents the install procedure for future re-install (different machine, fresh clone). Both repos updated — knowledge in 2 durable homes.
> - **E — Environment drift** (fresh-install pass). `cargo install --path . --locked` from a clean shell with no `target/` cache must succeed on the same MSRV declared in P012 (`rust-version = "1.85"`). Worker runs `cargo clean && cargo install --path . --locked` before declaring B passed.
> - **F — Runtime state gap** (no token leak surfaced by the install or by the new slash command body). Grep new slash command + tarot CHANGELOG diff for `ghp_/gho_/ghu_/ghs_/github_pat_` patterns — 0 hits expected (pure procedural markdown + plain CLI invocation).

---

## Context

### Vấn đề hiện tại

advisory-inbox MVP đã ship hoàn chỉnh (P001-P012 — 12 phiếu merged, 69 tests pass, binary ~2.16 MB, `cargo publish --dry-run` exit 0). Nhưng **tarot — repo gốc cần thay — vẫn chạy 142-line Bash heredoc** trong `~/tarot/.claude/commands/advisory-scan.md`. Cho đến khi tarot thực sự gọi binary, toàn bộ advisory-inbox là **shelfware** — Sếp build xong nhưng không ai dùng.

P013 đóng vòng lặp: replace 142-line Bash heredoc bằng ~15-line wrapper gọi `advisory-inbox scan-and-append`.

**Symptoms cần fix:**

1. **Tarot's `/advisory-scan` is 142 lines of Bash quote-escape + jq arg-passing + awk HTML-comment-skip.** PROJECT.md §"Why this exists" (line 9): "Trong tarot, slash command `/advisory-scan` đã rebuild **4 lần** (P282/P284/P285/P286 of soulsign tarot) vì Bash quote escape + jq arg passing + awk skip HTML comment fragile." Each fix added 30-50 lines. The whole reason advisory-inbox exists is to delete this.

2. **The binary handles every code path the heredoc handled, deterministically.** Per ARCHITECTURE §1: `scan-and-append` is the composite (parse → dedup → append + state update) the slash command needs. Tested 69 times. Replacing 142 lines of Bash with one binary invocation is the entire MVP success criterion (PROJECT.md §Success criteria #5).

3. **State file format compat must be preserved.** Tarot's existing `.advisory-scan-state` was last touched by the legacy Bash heredoc (P286 of tarot precedent — see DISCOVERIES.md P008). Format could be JSON v1 (post-P282 of tarot) OR JSON-without-schema_version (Worker Turn 1 verified — see Anchor #6 + Decision #3 Path C added V2) OR legacy single-line ISO. New binary's `state::read` enforces JSON v1 (P005 discovery). If non-v1 → must run migration step before first new scan. Sub-mech C check is non-negotiable: `seen_advisories[]` count never decreases through this transition.

4. **No crates.io publish yet.** Per user brief + PROJECT.md Phase 4. Install via `cargo install --path /Users/nguyenhuuanh/advisory-inbox --locked` (local source). This phiếu validates the binary end-to-end against real tarot data; a separate Sếp decision post-P013 ships to crates.io.

Reference BACKLOG.md P013 (lines 116-122):
- Lane: Guarded (changes tarot security slash command).
- Tầng: 1.
- Scope: install binary, rewrite slash command (5-10 lines), remove ~135 lines heredoc.
- Acceptance: tarot `/advisory-scan` test fire → same output as before. Smoke test against last scan.
- Sub-mech checks: A (slash command fires), C (state file compat preserved).

### Giải pháp

**7 tasks across 2 repos.** Pattern: install → verify state compat → rewrite slash → smoke test → CHANGELOG → PR → Discovery Report.

#### Decision #1 — Install via `cargo install --path` from local source

Worker (from `~/advisory-inbox/`):
```bash
cargo install --path . --locked
```

- `--path .` — install from local source (no crates.io fetch).
- `--locked` — use `Cargo.lock` exactly (no transitive dep version drift; Sub-mech E hygiene).
- Result: `~/.cargo/bin/advisory-inbox` on PATH.
- Verify: `advisory-inbox --version` → `advisory-inbox 0.1.0` (or whatever `Cargo.toml` declares post-P012).

**KHÔNG use `cargo install --force`** unless Worker explicitly intends to overwrite an existing install (e.g., previous P013 attempt). Per CLAUDE.md Hard Stop #10 — escalate if `--force` is needed. First install on a clean box should not require `--force`.

**KHÔNG publish to crates.io.** Real `cargo publish` is a separate Sếp decision post-P013 (likely once tarot integration validates real-world behavior — at which point installs in other repos can use `cargo install advisory-inbox` from crates.io instead of `--path`).

#### Decision #2 — Slash command rewrite — ~15-line body

The NEW `~/tarot/.claude/commands/advisory-scan.md` is a markdown procedural file (not a Bash script). Slash commands in Claude Code are markdown instructions to Claude — the binary invocation happens via Claude running a Bash tool call.

**Target body (Worker fills in the exact prose; this is the architectural shape):**

```markdown
---
description: Scan project for advisories — spawn advisory-watch agent, then pipe its report into the advisory-inbox CLI (replaces the previous 142-line Bash heredoc; see ~/tarot/CHANGELOG.md for migration date)
---

# /advisory-scan

1. Spawn `@agent-advisory-watch` with `$ARGUMENTS` (empty = full pnpm/requirements/Cargo scan). Capture the agent's full markdown output as the report — the agent emits a `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` block per advisory-inbox ARCHITECTURE.md §4.

2. Run the advisory-inbox composite subcommand, piping the report on stdin (omit `--report` flag — binary reads stdin when flag absent per `--help`):

   ```bash
   advisory-inbox scan-and-append \
     --inbox docs/security/advisory-inbox.md \
     --state docs/security/.advisory-scan-state
   ```

3. Parse the JSON output (stdout): `{ "appended": N, "skipped_dedup": M, "total_open": K }`.

4. Report to Sếp:
   - `N` new advisories appended to inbox (with the row Date/ID/Severity/Package list).
   - `M` duplicates skipped (already in `seen_advisories[]`).
   - `K` total open rows currently in inbox.
   - Link: `docs/security/advisory-inbox.md` for Sếp review (gạt row open → processed/dismissed).

5. Exit code: 0 = success, 1 = input error (sentinel missing / inbox missing `## Rows`), 2 = processing error (parse/write). See `advisory-inbox --help` for full exit codes.

> Binary version: see `advisory-inbox --version`. Install: `cargo install advisory-inbox` (once published) or `cargo install --path ~/advisory-inbox --locked` (local source, current install method per advisory-inbox P013).
```

**Worker may adjust wording / heading shape / list ordering** — Tầng 2 self-decide. **MUST preserve:**
- The front-matter `description:` field (Claude Code uses this for slash command list display).
- The `@agent-advisory-watch` spawn step (agent is the data source; binary doesn't fetch network).
- The exact `--inbox` / `--state` flag spelling. **Stdin is read when `--report` is OMITTED entirely** (per `--help`: "Omit for stdin" — confirmed Worker Turn 1 O1.2). **DO NOT pass `--report -`** — binary treats `-` as literal filename and exits 2.
- The JSON output parsing instruction (Claude must surface counts to Sếp, not just exit silently).
- A pointer to `--help` for exit codes (so future debugging doesn't re-invent the wheel).

**Hard target:** body (post-front-matter) ≤ 25 lines including code fences. The "~15 lines" in the user brief is the prose target — code fence + front-matter add ~10 more. ≤ 25 total = OK.

**KHÔNG include:**
- Any `jq` invocation (binary does the parsing — that's the whole point).
- Any `awk` or `sed` invocation (binary handles HTML comments / sentinel markers).
- Any heredoc (`<<EOF ... EOF`) — replaced wholesale.
- Any `if [ ... ]` Bash conditional (binary returns exit code; Claude branches on that).
- Any inline state file format assumption (binary owns the schema).
- **Any `--report -` flag form** (V2 correction — binary doesn't accept `-` as stdin sentinel; omit the flag instead).

#### Decision #3 — State file compat (Sub-mech C — mandatory)

Tarot's existing `~/tarot/docs/security/.advisory-scan-state` was last written by the legacy Bash heredoc (P286 of tarot precedent). Format detection:

1. **Worker reads** `~/tarot/docs/security/.advisory-scan-state` (or wherever tarot's state file lives — Anchor #4 verify).

2. **Path A — Already JSON v1.** If first byte is `{` AND `jq '.schema_version'` returns `1` → no migration needed. Sub-mech C check: count `seen_advisories[]` length pre-install, verify preserved post-first-scan.

3. **Path B — Legacy single-line ISO.** If first line is an ISO timestamp like `2026-05-23T12:00:00Z` (no surrounding braces, first byte is a digit) → legacy. Worker runs **once**:
   ```bash
   advisory-inbox migrate-state --state ~/tarot/docs/security/.advisory-scan-state
   ```
   Output: `{ "from": "legacy", "to": "json-v1", "seen_count": N }`. `N` is expected to be `0` — legacy single-line ISO only stored `last_scan_at`, NOT `seen_advisories[]` (per ARCHITECTURE.md §2 "Legacy format"). After migration, `seen_advisories[]` will be `[]` → run `state-backfill` follow-up.

4. **Path C (NEW V2 — JSON-without-schema_version).** If first byte is `{` AND `jq '.schema_version'` returns `null` (field absent) AND `jq '.seen_advisories'` returns an array → file is JSON written by an older tarot heredoc that didn't know advisory-inbox's `schema_version` field. **Existing `seen_advisories[]` entries MUST be preserved** (this is the actual state Worker found in Turn 1 at `~/tarot/docs/security/.advisory-scan-state` — 2 entries: `CVE-2026-9256`, `CVE-2026-27205`).

   Worker runs a tarot-side pre-step (NOT a change to advisory-inbox source — that would violate Constraint #2):
   ```bash
   jq '. + {"schema_version": 1}' ~/tarot/docs/security/.advisory-scan-state > /tmp/state-fixed.json \
     && mv /tmp/state-fixed.json ~/tarot/docs/security/.advisory-scan-state
   ```
   After this, `jq '.schema_version'` returns `1`, all existing fields (`last_scan_at`, `seen_advisories`, `agent_version`) preserved verbatim. The file is now valid JSON v1; binary's `state::read` accepts it; `scan-and-append` runs normally. Sub-mech C: count stays ≥ 2 (the 2 pre-existing CVEs are preserved by `jq '. + {...}'` because that operator merges-with-overwrite-on-conflict and the source has no `schema_version` to conflict).

   **Verify post-jq:**
   ```bash
   jq '.schema_version' ~/tarot/docs/security/.advisory-scan-state   # → 1
   jq '.seen_advisories | length' ~/tarot/docs/security/.advisory-scan-state   # → ≥ 2
   jq -e '.last_scan_at and .seen_advisories and .agent_version' ~/tarot/docs/security/.advisory-scan-state   # → true
   ```

   **Follow-up note (out of P013 scope):** future P014 could enhance `advisory-inbox migrate-state` to auto-detect "JSON-without-schema_version" and add the field idempotently. For P013 we use the jq pre-step — it's a one-liner, fully reversible (snapshot taken pre-step), and doesn't require shipping new advisory-inbox source code.

5. **Path B follow-up — backfill empty seen list.** If Path B ran and `seen_advisories[]` is now `[]`, AND tarot inbox has rows with status `processed`/`dismissed`, Worker runs:
   ```bash
   advisory-inbox state-backfill \
     --state ~/tarot/docs/security/.advisory-scan-state \
     --inbox ~/tarot/docs/security/advisory-inbox.md
   ```
   Output: `{ "backfilled_count": N, "total_seen_after": M }`. After this, Sub-mech C holds: dedup correctly skips already-known advisories.

**Hard Stop (Sub-mech C):** if `seen_advisories[]` count post-install < pre-install at any point (e.g., migrate wipes data without backfill running, OR backfill fails to recover all known IDs, OR jq pre-step somehow loses entries), STOP and escalate. State file rollback: `cp ~/tarot/docs/security/.advisory-scan-state.before <state>` (snapshot taken in Pre-phiếu snapshot step below).

**Why this matters:** if dedup state is lost, the next `/advisory-scan` run will re-flag every previously-dismissed advisory as new → Sếp sees noise + loses signal. This is the exact failure mode P286 of tarot was built to prevent; P013 must not regress it.

#### Decision #4 — Smoke test (Sub-mech A — mandatory)

Worker runs the new `/advisory-scan` end-to-end after install + slash rewrite. Two acceptable approaches:

**Approach A (preferred — full integration):**
1. In `~/tarot/` working directory, invoke `/advisory-scan` (via Claude Code session, since it's a slash command).
2. Observe Claude:
   - Spawns `@agent-advisory-watch` (logs visible).
   - Receives agent report (markdown).
   - Runs `advisory-inbox scan-and-append --inbox ... --state ...` via Bash tool (stdin piped, `--report` omitted).
   - Surfaces JSON output to Sếp.
3. Verify side effects:
   - `~/tarot/docs/security/.advisory-scan-state` updated (newer `last_scan_at`, `seen_advisories[]` may grow).
   - `~/tarot/docs/security/advisory-inbox.md` updated (new rows under `## Rows` IF any new advisories found; else unchanged).
   - Exit code 0 (Claude reports success).

**Approach B (fallback — direct binary if real agent unavailable):**
1. Worker exports the last real agent report from tarot history (look in `~/tarot/docs/security/` or `~/tarot/docs/runlog/` for `*.md` files containing `<!-- INBOX_APPEND_START -->`).
2. Pipe to binary directly (omit `--report` — stdin is automatic):
   ```bash
   cat ~/tarot/docs/runlog/<last-scan>.md | advisory-inbox scan-and-append \
     --inbox ~/tarot/docs/security/advisory-inbox.md \
     --state ~/tarot/docs/security/.advisory-scan-state
   ```
3. Capture full JSON output.

**Approach A is preferred** because it exercises the slash command rewrite end-to-end (Sub-mech A trigger gap: the new markdown body must actually instruct Claude correctly; reading the markdown manually isn't the same as Claude reading + acting on it). If Approach A blocked by environment (e.g., advisory-watch agent unavailable in the EXECUTE session), fall back to B and **log explicitly in Discovery Report** that Sub-mech A trigger was verified via fallback B, not full A.

**Acceptance — "same output as before":**
- Same `appended` count (modulo new real advisories between old heredoc's last run and now — Worker uses the same input report for direct comparison if Approach B).
- Same row format in inbox (8 pipe-delimited columns — Date / Advisory ID / Source URL / Package / File:Line / Severity / Status / Note per ARCHITECTURE.md §3).
- Same dedup semantics (advisories already in `seen_advisories[]` are skipped, not re-appended).
- State file updated atomically (new `last_scan_at`, `seen_advisories[]` grows with `observed_ids` from the run).

#### Decision #5 — Backup / archive policy

Per user brief: **NO archive file.** Old 142-line heredoc is preserved via git history only (`git log -p ~/tarot/.claude/commands/advisory-scan.md` will show the full prior content). Worker does **NOT** create `~/tarot/.claude/commands/.archive/advisory-scan-v3.md` or any other archive copy.

Rationale: git is the archive. Creating a `.archive/` file adds maintenance burden, surface for drift, and confusion about which is canonical. Git diff suffices for forensics.

**Pre-phiếu snapshot** (mandatory — Worker auto first-step, runs from Worker's box):
```bash
PHIEU_ID="P013"
mkdir -p ~/advisory-inbox/.backup/${PHIEU_ID}
# Snapshot tarot files about to be changed:
cp ~/tarot/.claude/commands/advisory-scan.md ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-scan.md.before 2>/dev/null || true
cp ~/tarot/docs/security/.advisory-scan-state ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-scan-state.before 2>/dev/null || true
cp ~/tarot/docs/security/advisory-inbox.md ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-inbox.md.before 2>/dev/null || true
# Capture pre-install seen_advisories count (Sub-mech C baseline):
if [ -f ~/tarot/docs/security/.advisory-scan-state ]; then
  head -c 1 ~/tarot/docs/security/.advisory-scan-state > ~/advisory-inbox/.backup/${PHIEU_ID}/state-firstchar.txt
fi
# Capture tarot HEAD for rollback:
(cd ~/tarot && git rev-parse HEAD) > ~/advisory-inbox/.backup/${PHIEU_ID}/tarot-head.txt
echo "✓ P013 snapshot at ~/advisory-inbox/.backup/${PHIEU_ID}/"
```

**Rollback (if needed mid-EXECUTE):**
- Restore slash command: `cp ~/advisory-inbox/.backup/P013/advisory-scan.md.before ~/tarot/.claude/commands/advisory-scan.md`
- Restore state file: `cp ~/advisory-inbox/.backup/P013/advisory-scan-state.before ~/tarot/docs/security/.advisory-scan-state`
- Restore inbox: `cp ~/advisory-inbox/.backup/P013/advisory-inbox.md.before ~/tarot/docs/security/advisory-inbox.md`
- Tarot git reset: `cd ~/tarot && git reset --hard $(cat ~/advisory-inbox/.backup/P013/tarot-head.txt)`
- Uninstall binary: `cargo uninstall advisory-inbox` (optional — leaving it installed is harmless).

#### Decision #6 — Tarot CHANGELOG entry

Worker locates tarot's CHANGELOG (`~/tarot/CHANGELOG.md` OR `~/tarot/docs/CHANGELOG.md` — Anchor #5 verify which one exists; if both, the more recently updated one wins; if neither, create `~/tarot/CHANGELOG.md`).

Entry shape (newest-first, match tarot's existing convention — Worker reads top 50 lines pre-append to confirm shape):

```markdown
## <date> — Replaced 142-line Bash heredoc in /advisory-scan with advisory-inbox binary

- `/advisory-scan` slash command rewritten from 142-line Bash heredoc → ~15-line procedural markdown that invokes the `advisory-inbox` CLI (v0.1.0).
- Binary source: `~/advisory-inbox/` (advisory-inbox repo P001-P012). Install: `cargo install --path ~/advisory-inbox --locked` (not yet on crates.io as of this entry).
- State file `docs/security/.advisory-scan-state` <action>: [delete two; keep one]
  - "already JSON v1 — no migration needed" (Decision #3 Path A)
  - "JSON-without-schema_version detected → ran `jq '. + {\"schema_version\": 1}'` to upgrade in place; existing seen_advisories[] entries preserved" (Decision #3 Path C — V2)
  - "migrated from legacy single-line ISO to JSON v1 via `advisory-inbox migrate-state`; backfilled N seen advisories via `advisory-inbox state-backfill` from inbox `processed`/`dismissed` rows" (Decision #3 Path B)
- Smoke test: `/advisory-scan` invocation produced same logical output as prior heredoc run (Approach A) / via direct binary pipe of last-export report (Approach B). See `~/advisory-inbox/docs/discoveries/P013.md` for full Sub-mech A/B/C verification trace.
- Old 142-line heredoc preserved via git history only (no archive file).
```

Worker MAY adjust prose; the bullet points (binary version, install method, state migration action, smoke test outcome, archive note) are mandatory content.

#### Decision #7 — Tarot PR shape

Branch: `feat/advisory-inbox-binary-install`
Commit: `feat: replace advisory-scan Bash heredoc with advisory-inbox binary`
Body sections (per RULES.md §9 + tarot's own PR template if it has one):
- Lane override: N/A (no override — this is genuinely Guarded, security slash command touch).
- Summary: 1 paragraph — what changed (142-line heredoc → binary), why (PROJECT.md §"Why this exists" 4 rebuilds), how validated (Sub-mech A/B/C all green).
- State migration outcome: link to advisory-inbox `docs/discoveries/P013.md`.
- Files changed: `.claude/commands/advisory-scan.md` (rewrite), `CHANGELOG.md` or `docs/CHANGELOG.md` (entry).

PR opens in `~/tarot/` repo. Sếp reviews + merges in tarot. `/security-review <PR>` invoked by orchestrator post-push (per RULES.md §16 Phiếu Lifecycle step 7 — Guarded lane mandatory).

### Scope

- **CHỈ sửa (tarot repo):** `~/tarot/.claude/commands/advisory-scan.md` (rewrite — heredoc → thin wrapper). `~/tarot/CHANGELOG.md` (or `~/tarot/docs/CHANGELOG.md` — whichever exists) — prepend entry per Decision #6.
- **CHỈ sửa (advisory-inbox repo):** `docs/discoveries/P013.md` (NEW). `docs/DISCOVERIES.md` (prepend 1-line index entry). **NO `src/` change. NO `Cargo.toml` change. NO test change.** advisory-inbox is consumed as a finished product here.
- **CHỈ run (Worker's machine):** `cargo install --path ~/advisory-inbox --locked` (one time). Possibly `jq '. + {"schema_version": 1}'` pre-step (one time, Path C). Possibly `advisory-inbox migrate-state` + `advisory-inbox state-backfill` (one time each, Path B).
- **KHÔNG sửa (tarot repo):** any file under `~/tarot/src/`, `~/tarot/lib/`, or any other tarot source code. Any other slash command in `~/tarot/.claude/commands/`. `~/tarot/CLAUDE.md` (unless it explicitly references the 142-line heredoc — Anchor #7; if it does, 1-line update OK). `~/tarot/.mcp.json` (not in P013's scope; future phiếu may wire MCP server `advisory-inbox serve` into tarot, but THIS phiếu is CLI-only).
- **KHÔNG sửa (advisory-inbox repo):** anything under `src/`. `Cargo.toml`. `tests/`. `README.md`. `docs/ARCHITECTURE.md`. `docs/PROJECT.md`. `docs/RULES.md`. `docs/CHANGELOG.md` (advisory-inbox's CHANGELOG is for advisory-inbox code ships; P013 doesn't ship code IN this repo — only the install procedure is documented here, in `docs/discoveries/P013.md`).
- **KHÔNG tạo:** archive file `.archive/advisory-scan-v3.md` (Decision #5 — git is the archive). Any new file in advisory-inbox repo beyond `docs/discoveries/P013.md`.
- **KHÔNG run:** `cargo publish` (real). `cargo install --force` (escalate if needed — CLAUDE.md Hard Stop #10). Any tarot CI workflow that Worker doesn't explicitly understand.
- **KHÔNG modify** tarot's `~/tarot/docs/security/.advisory-scan-state` directly via text editor — only via (a) the documented `jq '. + {...}'` pre-step (Path C, V2) which is a structured field-merge, OR (b) `advisory-inbox migrate-state` / `state-backfill` / `scan-and-append` subcommands (binary owns atomic write per ARCHITECTURE.md §7 + INV-LOCAL-002).

### Skills consulted

**Architect did NOT invoke context7 or other research tools for P013.** Reasoning:

- The advisory-inbox CLI surface (subcommands, flags, exit codes, JSON output shape) is local knowledge — owned by `docs/ARCHITECTURE.md` (§1 CLI Surface, §2 State Schema, §3 Inbox Format, §4 Sentinel Marker, §7 Atomic Write). Architect Read these pre-phiếu.
- The state file format compat scenarios (JSON v1 vs legacy single-line ISO) are documented in ARCHITECTURE.md §2 + P007 / P008 / P282 / P286 of tarot precedent. Architect Read DISCOVERIES.md entries for those.
- Slash command markdown format (front-matter + procedural body) is Claude Code convention; no external lib API to verify.
- `cargo install --path` semantics are standard Cargo; no version-specific behavior to look up.

**Architect DID Read pre-phiếu** (advisory-inbox local files only — within envelope):
- `docs/BACKLOG.md` (P013 brief lines 116-122, Phase 4 context).
- `docs/PROJECT.md` (§"Why this exists" — the 4-rebuild history that motivates P013; §Success criteria #5 — smoke test acceptance).
- `docs/ARCHITECTURE.md` (§1 CLI surface incl `scan-and-append`/`migrate-state`/`state-backfill` flag spelling, §2 State schema legacy-vs-JSON-v1 detail, §3 Inbox format 8-column row, §4 Sentinel markers, §7 Atomic write).
- `docs/RULES.md` (§1 Lane Routing — Guarded; §11 Docs Gate Tầng 1 security boundary AUTO; §7 Sub-mech A-F matrix; §16 Phiếu Lifecycle SECURITY_REVIEW step).
- `docs/DISCOVERIES.md` (P007/P008/P009 — migration + backfill + composite ship history; informs Sub-mech C handling).
- `docs/ticket/TICKET_TEMPLATE.md` (this phiếu format).
- `docs/ticket/P012-polish-publish.md` (reference shape for the immediately-prior phiếu's verification anchor table + Tầng wording).

**Architect did NOT Read** (per envelope):
- Any `src/**` in advisory-inbox (envelope forbids).
- Any file in `~/tarot/` (envelope forbids reading other repos' code; tarot file paths and content are `[needs Worker verify]`).
- `~/tarot/.claude/commands/advisory-scan.md` content (the 142-line heredoc itself) — Architect knows it's 142 lines from BACKLOG.md + PROJECT.md citations but has not Read it.

All tarot-side anchors below carry `[needs Worker verify]`. All advisory-inbox-side anchors are `[verified]` against the docs Architect Read.

---

## Verification Anchors — Kiến trúc sư đã verify lúc viết phiếu

> Mỗi anchor PHẢI carry humility marker `[verified]` / `[unverified]` / `[needs Worker verify]`.

| # | Assumption | Verify bằng cách nào | Marker | Kết quả |
|---|-----------|---------------------|--------|---------|
| 1 | advisory-inbox CLI has subcommand `scan-and-append` with flags `--inbox <FILE>` / `--state <FILE>`, outputs JSON `{ "appended": N, "skipped_dedup": M, "total_open": K }`, and reads agent report from stdin when `--report` is OMITTED (per `--help`: "Omit for stdin"). | Architect Read ARCHITECTURE.md §1 lines 77-88. Updated V2 per Worker Turn 1 O1.2. | `[verified]` | ✅ V2 corrected: stdin form is `--report` OMITTED (not `--report -`). All phiếu references updated. |
| 2 | advisory-inbox CLI has subcommand `migrate-state --state <FILE>` (detects legacy single-line ISO → JSON v1) and `state-backfill --state <FILE> --inbox <FILE>` (extracts IDs from inbox rows status processed/dismissed). | Architect Read ARCHITECTURE.md §1 lines 56-75 (migrate-state) + 66-75 (state-backfill). | `[verified]` | ✅ Both subcommands documented. P007 + P008 DISCOVERIES.md entries confirm shipped. Note: `migrate-state` does NOT detect JSON-without-schema_version (Path C) — that path uses `jq` pre-step per V2 Decision #3. |
| 3 | advisory-inbox binary version is `0.1.0` per `Cargo.toml`. Post-install: `advisory-inbox --version` outputs `advisory-inbox 0.1.0`. | Architect did NOT Read Cargo.toml directly in Bước 0 for P013 (P012 anchor #2 already confirmed version field exists; not re-read). | `[unverified]` | ⏳ TO VERIFY at EXECUTE (`advisory-inbox --version` post-install). If version differs from 0.1.0, Worker logs actual + updates CHANGELOG entry accordingly (mechanical). |
| 4 | Tarot's `/advisory-scan` slash command lives at `~/tarot/.claude/commands/advisory-scan.md` and is 142 lines of Bash heredoc. Tarot's state file lives at `~/tarot/docs/security/.advisory-scan-state`. Tarot's inbox lives at `~/tarot/docs/security/advisory-inbox.md`. | Worker Turn 1 verified all 3 paths exist; `wc -l` = 142. | `[verified]` | ✅ Confirmed Worker Turn 1. |
| 5 | Tarot has a CHANGELOG file. Worker Turn 1 verified at `~/tarot/docs/CHANGELOG.md` (not root). Newest-first convention. | Worker Turn 1 verified. | `[verified]` | ✅ Use `~/tarot/docs/CHANGELOG.md`. |
| 6 | Tarot's existing `~/tarot/docs/security/.advisory-scan-state` format. | Worker Turn 1 verified. | `[verified]` | ✅ V2: file is **JSON-without-schema_version** (third format — Decision #3 Path C). First byte `{`. Content: `{"last_scan_at":"2026-05-26T01:50:00Z","seen_advisories":["CVE-2026-9256","CVE-2026-27205"],"agent_version":"advisory-watch@0.1.0"}`. `jq '.schema_version'` = `null`. Path C `jq '. + {"schema_version": 1}'` pre-step applies. Pre-step preserves both existing CVEs → Sub-mech C count ≥ 2 post-install. |
| 7 | Tarot's `~/tarot/CLAUDE.md` does NOT contain a hard-coded reference to "142-line heredoc" or `/advisory-scan` Bash implementation details that would go stale with P013. If it does, 1-line update in same PR. | Worker Turn 1 verified: 2 hits in CLAUDE.md (lines 231, 243) referencing `/advisory-scan` by name (slot names + tool descriptions) — no "142-line" or "heredoc" wording. | `[verified]` | ✅ ≤ 3 hits, no update needed (descriptions reference the slash command's existence, not its implementation). |
| 8 | `cargo install --path . --locked` from a clean `target/` succeeds on the MSRV declared in P012 Cargo.toml (`rust-version = "1.85"`). Resulting binary `~/.cargo/bin/advisory-inbox` is ~2.16 MB (per P011 DISCOVERIES). | Architect Read DISCOVERIES.md P011 (binary size) + P012 (MSRV declaration). Worker Turn 1 reconfirmed `cargo build --release` exit 0. | `[verified]` | ✅ Worker re-runs at EXECUTE for Sub-mech B/E confirmation. |
| 9 | `~/.cargo/bin/` is on Worker's `$PATH` post-install (standard Cargo convention; `rustup` adds it by default). | Standard environment assumption. | `[unverified]` | ⏳ TO VERIFY at EXECUTE (`which advisory-inbox` post-install). If not on PATH, Worker either (a) adds `~/.cargo/bin` to `$PATH` in shell rc, OR (b) invokes binary via full path `~/.cargo/bin/advisory-inbox` in the slash command. Option (b) is safer (no shell config drift). Tầng 2 self-decide. |
| 10 | Tarot's existing `/advisory-scan` slash command CAN be invoked end-to-end in the EXECUTE session (advisory-watch agent available, tarot pnpm/cargo lock files in place for real scan). | Architect did NOT verify tarot environment. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE. If advisory-watch agent unavailable OR tarot scan environment incomplete, fall back to Decision #4 Approach B (direct binary pipe with last-export report). Discovery Report logs which approach used. |
| 11 | The `.claude/commands/advisory-scan.md` front-matter (YAML between `---` markers) is preserved in the rewrite — specifically the `description:` field. New `description:` may differ in wording but the field MUST be present (Claude Code uses it for slash command list display). | Standard Claude Code slash command convention. | `[verified]` | ✅ Decision #2 specifies the front-matter explicitly. Worker preserves shape. |
| 12 | Sub-mech A trigger verification: after rewrite, invoking `/advisory-scan` in a tarot Claude Code session causes Claude to (1) spawn `@agent-advisory-watch`, (2) capture report, (3) run binary via Bash tool (stdin pipe, `--report` omitted), (4) report counts. This is the entire purpose of P013. | Architect specifies via Decision #4. Verification is at EXECUTE. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE via Approach A (preferred) or Approach B (fallback). Hard Stop if Sub-mech A cannot be verified by either approach — escalate as design objection (slash command shape doesn't match Claude Code's invocation model; phiếu needs re-spec). |
| 13 | Sub-mech C state preservation: `seen_advisories[]` count post-install MUST be ≥ pre-install (never less). Path A (no-op), Path B (migrate + backfill), Path C (jq pre-step) all preserve or grow count. | Decision #3 + DISCOVERIES.md P008 (state-backfill ship) + V2 Path C jq merge semantics. | `[verified]` | ✅ Tarot baseline = 2 (CVE-2026-9256, CVE-2026-27205). Path C jq pre-step preserves both. Worker re-counts post-EXECUTE. Hard Stop on regression. |
| 14 | Sub-mech B build/install: `cargo install --path ~/advisory-inbox --locked` exits 0; `advisory-inbox --help` lists `scan-and-append` subcommand. | Decision #1 + P012 verification trace. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE. Standard install path; failure here would be a P012 regression (very unlikely). |
| 15 | Sub-mech E env drift: `cargo clean && cargo install --path ~/advisory-inbox --locked` from a clean shell (no `target/` cache) succeeds. Catches any "works on my machine but not fresh-install" bugs introduced by P012's optional `rust-version` addition. | Decision #1 + RULES.md §7 matrix. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE. If fresh-install fails despite normal-install succeeding, escalate as shape objection (P012 metadata is incomplete). |
| 16 | Sub-mech F token leak: new slash command body + tarot CHANGELOG diff contain ZERO `ghp_/gho_/ghu_/ghs_/github_pat_` patterns. (Should be trivially zero — pure procedural markdown + plain CLI invocation; no secrets needed.) | Standard runtime preflight per RULES.md §8. | `[needs Worker verify]` | ⏳ TO VERIFY at EXECUTE (`grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' ~/tarot/.claude/commands/advisory-scan.md ~/tarot/docs/CHANGELOG.md 2>/dev/null`). Expected: 0 hits. |
| 17 | Tarot repo's main branch is `main` (not `master`) — modern convention; matches advisory-inbox. | Worker Turn 1 verified. | `[verified]` | ✅ `main`. |
| 18 | Tarot has a `.github/pull_request_template.md` OR no template. | Worker Turn 1 verified. | `[verified]` | ✅ No template at `~/tarot/.github/pull_request_template.md`. Worker authors PR body from scratch including Lane section. |
| 19 | The `.advisory-scan-state` JSON v1 schema (from ARCHITECTURE.md §2): `{ schema_version: 1, last_scan_at: ISO-8601, seen_advisories: [string], agent_version: string }`. Path C (V2) brings tarot's existing JSON-without-schema_version into this shape by adding the `schema_version: 1` field via `jq`, preserving all other fields verbatim. | Architect Read ARCHITECTURE.md §2 lines 123-146 + V2 Path C added. | `[verified]` | ✅ Path C jq pre-step yields a file matching the documented v1 shape. |
| 20 | The new slash command body's stdin invocation works correctly with Claude's Bash tool — Claude can pipe agent output to advisory-inbox via shell pipe (`--report` flag OMITTED, binary reads stdin per `--help`). | Worker Turn 1 verified via `--help` output and direct test. V2 corrected. | `[verified]` | ✅ V2: omit `--report` for stdin. `--report -` is INVALID (binary treats `-` as literal filename → exit 2). All phiếu references corrected. |

**Hard Stop triggers:**
- Anchor #6 + #13 — `seen_advisories[]` count regression at any step → STOP, restore state from snapshot, escalate.
- Anchor #12 — Sub-mech A cannot be verified (Approach A AND Approach B both fail) → STOP, escalate as design objection.
- Worker discovers tarot file paths differ materially from Anchor #4 expectations (e.g., no `.claude/commands/` directory at all) → STOP, AskUserQuestion before proceeding. (Anchor #4 confirmed V2 — paths exist.)

**Nếu cột "Kết quả" có ❌ → Kiến trúc sư đã biết assumption sai và ghi rõ trong phiếu cách xử lý.** V1 had 3 ❌ on anchors #1, #6, #19, #20 — all resolved in V2 (`--report` omit form + Path C jq pre-step). All remaining `[needs Worker verify]` anchors resolve at EXECUTE.

### Pre-phiếu snapshot (Worker auto first-step)

```bash
PHIEU_ID="P013"
mkdir -p ~/advisory-inbox/.backup/${PHIEU_ID}
# Snapshot tarot files about to be changed:
cp ~/tarot/.claude/commands/advisory-scan.md ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-scan.md.before 2>/dev/null || true
cp ~/tarot/docs/security/.advisory-scan-state ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-scan-state.before 2>/dev/null || true
cp ~/tarot/docs/security/advisory-inbox.md ~/advisory-inbox/.backup/${PHIEU_ID}/advisory-inbox.md.before 2>/dev/null || true
# Capture pre-install seen_advisories count (Sub-mech C baseline):
if [ -f ~/tarot/docs/security/.advisory-scan-state ]; then
  head -c 1 ~/tarot/docs/security/.advisory-scan-state > ~/advisory-inbox/.backup/${PHIEU_ID}/state-firstchar.txt
  # If JSON (Path A or Path C), capture exact seen count:
  jq '.seen_advisories | length' ~/tarot/docs/security/.advisory-scan-state 2>/dev/null \
    > ~/advisory-inbox/.backup/${PHIEU_ID}/seen-count-before.txt || echo "legacy-or-missing" > ~/advisory-inbox/.backup/${PHIEU_ID}/seen-count-before.txt
fi
# Capture tarot HEAD for rollback:
(cd ~/tarot && git rev-parse HEAD) > ~/advisory-inbox/.backup/${PHIEU_ID}/tarot-head.txt
# Capture tarot current branch:
(cd ~/tarot && git branch --show-current) > ~/advisory-inbox/.backup/${PHIEU_ID}/tarot-branch.txt
echo "✓ P013 snapshot at ~/advisory-inbox/.backup/${PHIEU_ID}/"
```

If install/rewrite goes wrong mid-execute: restore the `.before` copies + `git reset --hard $(cat ~/advisory-inbox/.backup/P013/tarot-head.txt)` in `~/tarot/` + `cargo uninstall advisory-inbox` (optional).

---

## Debate Log

> Auto-populated bởi Worker (CHALLENGE mode) và Architect (RESPOND mode).
> Sếp chỉ đọc lúc nghiệm thu — không can thiệp mid-debate trừ khi orchestrator triệu.
> Schema: 1 turn = 1 cặp Worker Challenge + Architect Response. Phiếu version bump V1 → V2 → ... mỗi turn Architect refine.
> Cap = 3 turns. Sau Turn 3 chưa consensus → force-escalate Sếp.

**Phiếu version:** V2 (Architect RESPOND Turn 1 — addressed O1.1 shape + O1.2 mechanical)

### Turn 1 — Worker Challenge

**Date:** 2026-05-28

**Anchor verification:**

- Anchor #1 ✅ — `scan-and-append` subcommand verified via `--help`. NOTE: `--report -` does NOT accept stdin (see O1.2 below).
- Anchor #2 ✅ — `migrate-state` and `state-backfill` subcommands confirmed via `--help` output.
- Anchor #3 ✅ — `Cargo.toml` version = `0.1.0`.
- Anchor #4 ✅ — `~/tarot/.claude/commands/advisory-scan.md` exists, `wc -l` = 142. `~/tarot/docs/security/.advisory-scan-state` exists. `~/tarot/docs/security/advisory-inbox.md` exists.
- Anchor #5 ✅ — CHANGELOG at `~/tarot/docs/CHANGELOG.md` (not root). Newest-first, section heading format.
- Anchor #6 ❌ — State file first byte is `{` (JSON), but `jq '.schema_version'` returns `null` — the file is **a third format**: JSON with `seen_advisories` and `last_scan_at` but **no `schema_version` field**. Content: `{ "last_scan_at": "2026-05-26T01:50:00Z", "seen_advisories": ["CVE-2026-9256", "CVE-2026-27205"], "agent_version": "advisory-watch@0.1.0" }`. See O1.1.
- Anchor #7 ✅ — `grep` in `~/tarot/CLAUDE.md` returns 2 hits referencing `/advisory-scan` slash command (slot names and tool descriptions at lines 231, 243) — no "142-line" or "heredoc" wording. ≤ 3 hits, no escalation needed.
- Anchor #8 ✅ — `cargo build --release` exits 0 (confirmed). Sub-mech E re-run at EXECUTE.
- Anchor #9 ⏳ — Deferred to EXECUTE post-install.
- Anchor #10 ⏳ — Cannot verify agent availability pre-EXECUTE. Approach B fallback available.
- Anchor #11 ✅ — Decision #2 specifies `description:` front-matter.
- Anchor #12 ⏳ — Depends on O1.2 resolution (stdin flag shape).
- Anchor #13 ⚠️ — Sub-mech C baseline: tarot state currently has `seen_advisories` length = 2. However O1.1 must be resolved first before migrate path is determined.
- Anchor #14 ⏳ — Deferred to EXECUTE (cargo install).
- Anchor #15 ⏳ — Deferred to EXECUTE (cargo clean + reinstall).
- Anchor #16 ⏳ — Deferred to EXECUTE (post-rewrite grep).
- Anchor #17 ✅ — `git -C ~/tarot branch --show-current` = `main`.
- Anchor #18 ✅ — No `~/tarot/.github/pull_request_template.md`. Worker authors PR body from scratch including Lane section.
- Anchor #19 ❌ — Tarot state file does NOT match the documented JSON v1 schema. `schema_version` field is absent. See O1.1.
- Anchor #20 ❌ — `--report -` does NOT work. Running `advisory-inbox scan-and-append --report - ...` returns `exit 2` with `reading report file '-': No such file or directory`. Binary treats `-` as literal filename. Correct stdin invocation is to OMIT `--report` entirely (confirmed via `--help`: "Omit for stdin"). Decision #2 and Task 4 template both use `--report -` which is broken. See O1.2.

---

**Objections (Tầng 1 — phiếu cần sửa):**

**[O1.1] [shape] — Tarot state file is a third unrecognized format**

Phiếu Decision #3 (lines 136-148) assumes exactly two formats:
- Path A: `schema_version: 1` JSON → already JSON v1, no migration.
- Path B: single-line ISO-8601 → legacy, run `migrate-state`.

Reality at `~/tarot/docs/security/.advisory-scan-state` (confirmed at `state.rs:150`):
```json
{
  "last_scan_at": "2026-05-26T01:50:00Z",
  "seen_advisories": ["CVE-2026-9256", "CVE-2026-27205"],
  "agent_version": "advisory-watch@0.1.0"
}
```
- `schema_version` field is absent.
- `serde_json::from_str::<StateFile>` fails → `StateReadError::Json { "missing field schema_version" }`.
- `migrate-state --dry-run` result: exit 1, `"format unrecognised (not JSON v1, not single-line ISO-8601 timestamp)"`.
- `scan-and-append` against this state: exit 1, `"malformed JSON: missing field schema_version"`.

This state file was written by tarot's legacy Bash heredoc (P286 of tarot) which didn't know about advisory-inbox's `schema_version` field. The existing 2 `seen_advisories` entries MUST be preserved (Sub-mech C hard stop).

**Proposed alternatives:**

A. **(Recommended) Tầng 2 workaround — manual state rewrite before Task 3.** Worker creates a compliant JSON v1 file from tarot's current state manually (inserting `schema_version: 1`, preserving all existing fields). Command: `jq '. + {"schema_version": 1}' ~/tarot/docs/security/.advisory-scan-state > /tmp/state-fixed.json && mv /tmp/state-fixed.json ~/tarot/docs/security/.advisory-scan-state`. After this, both `jq '.schema_version'` = 1 and `jq '.seen_advisories | length'` = 2. Then proceed normally — `migrate-state` will treat it as JSON v1 (idempotent re-write). Sub-mech C: 2 seen entries preserved → count ≥ 2 post-install.

B. Architect adds a new format path to `migrate-state` source (P014 scope). Unblocks after code change ships. P013 is deferred until P014 ready. Not recommended — delays P013 for a one-liner jq fix; also touches `src/` which P013 explicitly forbids.

**[O1.2] [mechanical] — `--report -` flag does not accept stdin; correct form is to omit `--report`**

Phiếu Decision #2 template (line 96), Task 4 (lines 541-545), Verification Trace table Sub-mech B (line 421), and Constraint #8 (line 786) all use `--report -` as the stdin invocation form.

Reality confirmed via `target/release/advisory-inbox scan-and-append --help`:
```
--report <REPORT>  Path to agent report. Omit for stdin
```
Running `advisory-inbox scan-and-append --report - ...` → exit 2, `"reading report file '-': No such file or directory"`. Binary treats `-` as a literal path.

Correct invocation: omit `--report` entirely. The slash command body in Task 4 must be changed from:
```bash
advisory-inbox scan-and-append \
  --report - \
  --inbox docs/security/advisory-inbox.md \
  --state docs/security/.advisory-scan-state
```
to:
```bash
advisory-inbox scan-and-append \
  --inbox docs/security/advisory-inbox.md \
  --state docs/security/.advisory-scan-state
```
(stdin is automatic when `--report` is omitted).

This is `[mechanical]` — wording change only, no behavior change (stdin still works, same pipe semantics). Worker can self-decide under Tầng 2 BUT since the phiếu body, constraint #8, and verification trace all specify `--report -` explicitly, correction must be acknowledged here to avoid confusion.

Suggested fix: Architect amend Decision #2 template, Task 4 concrete body, Constraint #8, and Verification Trace row to use `--report` omitted form OR document `--report -` as intended pipe idiom for future P014 (add `-` = stdin support to the binary). For P013 EXECUTE, Worker will use the omit-flag form and note it in Discovery Report.

---

**Status:** ✅ ARCHITECT RESPONDED — see Turn 1 — Architect Response below. Phiếu V1 → V2.

### Turn 1 — Architect Response (phiếu V2)

**Date:** 2026-05-28

- **[O1.1] [shape] → ACCEPT (Option A — jq pre-step).** Worker's analysis is correct: tarot state file is a third unrecognized format (JSON-without-schema_version, 2 existing CVE entries). Decision #3 in V1 only covered 2 paths; this is a structural gap. Action taken:
  - Added **Decision #3 Path C** documenting the jq pre-step: `jq '. + {"schema_version": 1}' ~/tarot/docs/security/.advisory-scan-state > /tmp/state-fixed.json && mv /tmp/state-fixed.json ~/tarot/docs/security/.advisory-scan-state`. This is a **tarot-side pre-step**, NOT a change to advisory-inbox `src/` (Constraint #2 preserved). The `jq + {...}` operator merges-with-overwrite, and since the source file has no `schema_version` to conflict with, all existing fields (`last_scan_at`, `seen_advisories`, `agent_version`) are preserved verbatim. Sub-mech C count stays ≥ 2 (preserves both `CVE-2026-9256` and `CVE-2026-27205`).
  - Updated Decision #3 to enumerate **three paths** (A/B/C) with explicit verification commands for each. Path C includes post-jq verification: `jq '.schema_version'` → 1, `jq '.seen_advisories | length'` → ≥ 2, `jq -e '.last_scan_at and .seen_advisories and .agent_version'` → true.
  - Updated **Anchor #6, #13, #19** result columns to ✅ V2-resolved. Updated **Decision #6** CHANGELOG entry options to enumerate all three paths.
  - Updated **Task 3** to branch on the three detected formats (Worker Task 2 detects which; Task 3 dispatches accordingly).
  - Added Path C update to **Sub-mech C** description in header (multi-path migration).
  - **Follow-up note (out of P013 scope):** future P014 could enhance `advisory-inbox migrate-state` to auto-detect "JSON-without-schema_version" and add the field idempotently inside the binary. Out of P013 scope — P013 uses the documented jq pre-step which is fully reversible (snapshot-protected) and doesn't require shipping new advisory-inbox source code. Logged here for backlog grooming.
  - Worker's recommended Option A confirmed; Option B (modify `src/`) rejected per V1 Constraint #2.

- **[O1.2] [mechanical] → ACK.** All `--report -` references replaced with `--report` omitted form. Updated in V2: Decision #2 template, Decision #2 KHÔNG-include list (added explicit "no `--report -`"), Task 4 concrete body, Task 5 Approach B pipe command, Constraint #8 (rephrased), Verification Trace Sub-mech B stdin test row. Anchor #1 marker updated `[verified]` (was `[verified]` with ❌ result — now ✅). Anchor #20 marker updated `[verified]` (was ❌ — now ✅ with omit form). No behavior change; stdin pipe semantics unchanged.

**Status:** ✅ RESPONDED — phiếu bumped to V1 → V2. Awaiting Worker Turn 2 surgical re-CHALLENGE to confirm V2 changes resolve both objections.

### Turn 2 — Worker Challenge

**Date:** 2026-05-28

**Mode:** SURGICAL — verify V2 delta only (O1.1 Path C added + O1.2 `--report -` removed from instruction sections).

**O1.1 [shape] — Path C jq pre-step — RESOLVED ✅**

Decision #3 V2 (lines 144-161) now enumerates three paths:
- Path A (JSON v1) — no-op.
- Path B (legacy single-line ISO) — `migrate-state` + optional `state-backfill`.
- Path C (JSON-without-schema_version) — `jq '. + {"schema_version": 1}'` pre-step with full post-jq verify commands (`jq '.schema_version'` → 1, `jq '.seen_advisories | length'` → ≥ 2, `jq -e '.last_scan_at and .seen_advisories and .agent_version'` → true).

Task 2 branch logic, Task 3 dispatch, Anchor #6/#13/#19 result columns, CHANGELOG entry options (Decision #6), and Sub-mech C description all updated for 3 paths. Sub-mech C hard stop on count regression preserved. Follow-up P014 noted out-of-scope. No `src/` change required. **V2 fully resolves O1.1.**

**O1.2 [mechanical] — `--report -` removed from instruction sections — RESOLVED ✅**

Verified V2 instruction sections:
- Decision #2 template (line 96-99): code fence omits `--report`, only `--inbox` + `--state`. ✅
- Decision #2 KHÔNG-include list (line 129): explicit "Any `--report -` flag form" added. ✅
- Task 4 concrete body (lines 666-670): same omit form, no `--report`. ✅
- Verification Trace Sub-mech B stdin test (line 524): `echo "" | advisory-inbox scan-and-append --inbox ... --state ...` with "(OMITTED `--report` flag — V2 corrected)" annotation. ✅
- Constraint #8 (line 913): "Binary stdin invocation MUST omit `--report` flag ... DO NOT pass `--report -`". ✅
- Anchor #1 result: ✅ "V2 corrected: stdin form is `--report` OMITTED (not `--report -`)". ✅
- Anchor #20 result: ✅ "V2: omit `--report` for stdin. `--report -` is INVALID". ✅

Remaining occurrences of `--report -` in the phiếu are: (a) historical Turn 1 Debate Log text (immutable record of what was objected to — correct to preserve), (b) "DO NOT pass" prohibitions in instruction sections (correct). No actionable instruction section still uses `--report -`. **V2 fully resolves O1.2.**

**Worker accepted V2 — no new challenges.** Anchor verification at Turn 2: O1.1 ✅ O1.2 ✅.

**Ready for EXECUTE.**

---

### Final consensus
- Phiếu version: V2.
- Total turns: 1 (Turn 1 Worker Challenge → Architect RESPOND V2 → Turn 2 Worker surgical verify).
- Approved (Guarded lane — explicit Sếp gate required per RULES.md §1): ✅ CONSENSUS REACHED. Worker Turn 2 confirms V2 resolves both O1.1 + O1.2. Proceeding to EXECUTE.

---

## Debug Log

> Worker emit observability records during EXECUTE. Mỗi entry = 1 cặp `event` + `evidence`.

```
[YYYY-MM-DDTHH:MM:SSZ] event=<name> evidence=<file:line or command output snippet>
```

Example:
```
[2026-05-28T14:00:00Z] event=cargo_install evidence=exit=0 binary=~/.cargo/bin/advisory-inbox size=2.16M
[2026-05-28T14:01:00Z] event=tarot_state_format evidence=first-char='{' format=json-without-schema-version seen_count=2
[2026-05-28T14:01:30Z] event=path_c_jq_prestep evidence=jq '. + {"schema_version": 1}' applied; post: schema_version=1 seen_count=2
[2026-05-28T14:02:00Z] event=slash_rewrite evidence=~/tarot/.claude/commands/advisory-scan.md lines_before=142 lines_after=18
[2026-05-28T14:05:00Z] event=smoke_test_approach evidence=A real_invocation appended=2 skipped_dedup=12 total_open=5
```

---

## Verification Trace (Sub-mechanism A/B/C/D/E/F — applicable to P013)

> Worker MUST run applicable Layer 2 capability checks (RULES.md §7 matrix) BEFORE marking phiếu DONE.

| Sub-mech | Check command | Expected | Actual | ✅/❌/N/A |
|----------|---------------|----------|--------|-----------|
| **A (trigger)** | Invoke `/advisory-scan` in tarot Claude Code session (Approach A) OR pipe last-export report into binary directly (Approach B) | New slash command body causes Claude to spawn agent + run binary + report counts; OR direct binary invocation exits 0 with same logical output | Approach B used (subagent cannot invoke /advisory-scan directly). `cat stub-report-8col.md \| advisory-inbox scan-and-append --inbox ... --state ...` → exit 0, `{"appended":1,"skipped_dedup":0,"total_open":1}` | ✅ |
| **B (capability)** | `cargo install --path ~/advisory-inbox --locked` | exit 0; `~/.cargo/bin/advisory-inbox` present | exit 0; binary at `/Users/nguyenhuuanh/.cargo/bin/advisory-inbox` (~51s clean build) | ✅ |
| **B (capability)** | `advisory-inbox --version` | `advisory-inbox 0.1.0` (Anchor #3) | `advisory-inbox 0.1.0` | ✅ |
| **B (capability)** | `advisory-inbox --help \| grep scan-and-append` | non-empty match | `scan-and-append  Composite: parse → dedup → append + state update` | ✅ |
| **B (capability)** | `echo "" \| advisory-inbox scan-and-append --inbox /tmp/test-inbox.md --state /tmp/test-state.json` (empty stdin via OMITTED `--report` flag — V2 corrected stdin form per Anchor #20) | exit 1 with stderr mentioning "sentinel" or "INBOX_APPEND_START" | exit 1, `error: missing sentinel start marker '<!-- INBOX_APPEND_START -->' in report` | ✅ |
| **C (Path C — JSON-without-schema_version, V2)** | `jq '. + {"schema_version": 1}' ~/tarot/docs/security/.advisory-scan-state > /tmp/state-fixed.json && mv /tmp/state-fixed.json ~/tarot/docs/security/.advisory-scan-state` | exit 0; post: `jq '.schema_version'` → 1, `jq '.seen_advisories \| length'` → ≥ 2 | exit 0; `schema_version`=1; `seen_advisories \| length`=2; all original fields preserved | ✅ |
| **C (migration — only if Anchor #6 = legacy, Path B)** | `advisory-inbox migrate-state --state ~/tarot/docs/security/.advisory-scan-state` | exit 0; JSON `{ "from": "legacy", "to": "json-v1", "seen_count": N }` | N/A — Path C applied (not Path B) | N/A |
| **C (migration — only if migrate yielded 0 seen, Path B)** | `advisory-inbox state-backfill --state ~/tarot/docs/security/.advisory-scan-state --inbox ~/tarot/docs/security/advisory-inbox.md` | exit 0; JSON `{ "backfilled_count": N, "total_seen_after": M }` with `M ≥ pre-install seen count` | N/A — Path C applied (not Path B) | N/A |
| **C (preservation)** | `jq '.seen_advisories \| length' ~/tarot/docs/security/.advisory-scan-state` (post-install) vs `cat ~/advisory-inbox/.backup/P013/seen-count-before.txt` | post ≥ pre | post=2, pre=2 → 2 ≥ 2 ✅ (CVE-2026-9256, CVE-2026-27205 preserved) | ✅ |
| **D (persistence)** | `grep -l "advisory-inbox" ~/tarot/docs/CHANGELOG.md ~/tarot/.claude/commands/advisory-scan.md 2>/dev/null` | ≥ 2 hits (slash command body + CHANGELOG entry both reference it) | 2 files matched | ✅ |
| **D (persistence)** | `ls ~/advisory-inbox/docs/discoveries/P013.md && grep -l "P013" ~/advisory-inbox/docs/DISCOVERIES.md` | both present | both present | ✅ |
| **E (env drift)** | `cd ~/advisory-inbox && cargo clean && cargo install --path . --locked` (fresh build) | exit 0; binary reinstalled | exit 0; clean build ~51s | ✅ |
| **E (env drift)** | `cargo update --dry-run` from `~/advisory-inbox/` | no surprise major bump (per P012 baseline) | wasip2/wit-bindgen MSRV-constrained downgrade (Rust 1.87 req vs MSRV 1.85) — expected, not a surprise major bump | ✅ |
| **F (runtime state)** | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' ~/tarot/.claude/commands/advisory-scan.md ~/tarot/docs/CHANGELOG.md 2>/dev/null` | 0 hits | 0 hits | ✅ |
| **F (runtime state)** | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' ~/tarot/.git/config` | 0 hits (token leak preflight per RULES.md §8) | 0 hits | ✅ |

---

## Nhiệm vụ

### Task 1: Install `advisory-inbox` binary from local source

**Working directory:** `~/advisory-inbox/` (NOT `~/tarot/`).

**Run:**
```bash
cd ~/advisory-inbox
cargo install --path . --locked
```

**Expected output:** `Installed package 'advisory-inbox v0.1.0' (executable 'advisory-inbox')` (or similar). Exit 0.

**Verify:**
```bash
which advisory-inbox        # expected: ~/.cargo/bin/advisory-inbox
advisory-inbox --version     # expected: advisory-inbox 0.1.0 (Anchor #3)
advisory-inbox --help        # expected: lists 8 subcommands incl scan-and-append
```

**Lưu ý:**
- KHÔNG use `--force` (Hard Stop #10). If `cargo install` fails because binary already installed from a prior P013 attempt, run `cargo uninstall advisory-inbox` first, then re-install.
- If `~/.cargo/bin` not on `$PATH` (Anchor #9), Worker Tầng 2 self-decides: (a) add to shell rc, OR (b) use full path `~/.cargo/bin/advisory-inbox` in slash command body. Option (b) is safer.
- Capture install output in Debug Log.

### Task 2: Verify tarot file paths + state file format

**Working directory:** `~/tarot/`.

**Run:**
```bash
cd ~/tarot
ls .claude/commands/advisory-scan.md
wc -l .claude/commands/advisory-scan.md  # Anchor #4 — expected 142
ls docs/security/.advisory-scan-state docs/security/advisory-inbox.md  # Anchor #4 paths
head -c 1 docs/security/.advisory-scan-state  # Anchor #6 — first byte
# V2: distinguish 3 formats now (Path A/B/C):
jq -e '.schema_version' docs/security/.advisory-scan-state 2>/dev/null && echo "PATH_A (json-v1)" \
  || (head -c 1 docs/security/.advisory-scan-state | grep -q '{' \
      && echo "PATH_C (json-without-schema_version)" \
      || echo "PATH_B (legacy single-line ISO)")
```

**Branch logic (V2 — 3 paths):**
- **Path A** — `jq '.schema_version'` returns `1` → state is JSON v1 → SKIP Task 3 (no migration needed). Capture `jq '.seen_advisories | length'` as Sub-mech C baseline.
- **Path C (V2)** — first byte is `{` BUT `jq '.schema_version'` returns `null` → state is JSON-without-schema_version (the actual tarot state per Worker Turn 1) → run **Task 3 Path C** (jq pre-step).
- **Path B** — first byte is a digit (year prefix `2`) → state is legacy single-line ISO → run **Task 3 Path B** (migrate-state + possibly state-backfill).
- If file missing → escalate (Anchor #4 + #6 wrong; phiếu re-spec needed).

**Also verify Anchor #5 (CHANGELOG location — already ✅ as `~/tarot/docs/CHANGELOG.md`), #7 (CLAUDE.md/README references — already ✅ ≤3 hits), #17 (main branch — already ✅), #18 (PR template — already ✅ none):**
```bash
ls ~/tarot/docs/CHANGELOG.md 2>/dev/null  # expected: present
grep -n "advisory-scan\|142.line\|heredoc" ~/tarot/CLAUDE.md ~/tarot/README.md 2>/dev/null  # 0-3 hits OK
git -C ~/tarot branch --show-current  # expected: main
```

**Lưu ý:**
- Run snapshot script (Pre-phiếu snapshot section above) BEFORE any modifications to tarot files.
- Log all verification results in Debug Log (especially which Path A/B/C was detected).
- This task is verification-only — no tarot file modifications yet.

### Task 3: State file migration (CONDITIONAL — branch on Task 2 path detection)

**File:** `~/tarot/docs/security/.advisory-scan-state`

**Path A (V1 unchanged) — already JSON v1:** SKIP this task entirely. Log "state already JSON v1, migration skipped" in Debug Log.

**Path C (V2 NEW) — JSON-without-schema_version (tarot's actual current state):**
```bash
jq '. + {"schema_version": 1}' ~/tarot/docs/security/.advisory-scan-state > /tmp/state-fixed.json \
  && mv /tmp/state-fixed.json ~/tarot/docs/security/.advisory-scan-state
```

**Verify Path C post-step:**
```bash
jq '.schema_version' ~/tarot/docs/security/.advisory-scan-state                       # expected: 1
jq '.seen_advisories | length' ~/tarot/docs/security/.advisory-scan-state             # expected: ≥ 2 (preserved CVE-2026-9256 + CVE-2026-27205)
jq -e '.last_scan_at and .seen_advisories and .agent_version' ~/tarot/docs/security/.advisory-scan-state  # expected: true (all original fields preserved)
```

After Path C, state file is valid JSON v1 → `scan-and-append` will accept it. NO `migrate-state` or `state-backfill` needed (file already contains the seen list).

**Path B (V1 unchanged) — legacy single-line ISO:**
```bash
advisory-inbox migrate-state --state ~/tarot/docs/security/.advisory-scan-state
# Expected output: { "from": "legacy", "to": "json-v1", "seen_count": 0 }
```

**Path B conditional follow-up — IF migrate yielded `seen_count: 0` AND inbox has `processed`/`dismissed` rows:**
```bash
advisory-inbox state-backfill \
  --state ~/tarot/docs/security/.advisory-scan-state \
  --inbox ~/tarot/docs/security/advisory-inbox.md
# Expected output: { "backfilled_count": N, "total_seen_after": M }
```

**Verify (Sub-mech C, all paths):**
```bash
jq '.schema_version' ~/tarot/docs/security/.advisory-scan-state  # expected: 1
jq '.seen_advisories | length' ~/tarot/docs/security/.advisory-scan-state  # expected: ≥ pre-install count
```

**Lưu ý:**
- Path C jq pre-step is atomic via temp+rename (`> /tmp/state-fixed.json && mv`). Safe to retry on failure.
- Path B subcommands are atomic per ARCHITECTURE.md §7. Safe to retry on failure.
- Hard Stop: if `seen_advisories[]` count post-step < pre-install count, restore from snapshot + escalate.
- Worker Task 2 path detection determines which Path A/B/C runs. NEVER run multiple paths on the same file.

### Task 4: Rewrite tarot `/advisory-scan` slash command

**File:** `~/tarot/.claude/commands/advisory-scan.md`

**Tìm:** entire current file (142 lines per Anchor #4 — Worker confirmed in Task 2).

**Thay bằng:** the body per Decision #2. Concretely (Worker may adjust prose / heading structure / list ordering, but content elements are mandatory):

```markdown
---
description: Scan project for advisories — spawn advisory-watch agent, then pipe its report into the advisory-inbox CLI (replaces the previous 142-line Bash heredoc; see CHANGELOG for migration date)
---

# /advisory-scan

1. Spawn `@agent-advisory-watch` with `$ARGUMENTS` (empty = full pnpm/requirements/Cargo scan). Capture the agent's full markdown output as the report — the agent emits a `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` block per advisory-inbox ARCHITECTURE.md §4.

2. Run the advisory-inbox composite subcommand, piping the report on stdin (omit `--report` flag — binary reads stdin when flag absent per `--help`):

   ```bash
   advisory-inbox scan-and-append \
     --inbox docs/security/advisory-inbox.md \
     --state docs/security/.advisory-scan-state
   ```

3. Parse the JSON output (stdout): `{ "appended": N, "skipped_dedup": M, "total_open": K }`.

4. Report to Sếp:
   - `N` new advisories appended to inbox (list new row Date/ID/Severity/Package per advisory).
   - `M` duplicates skipped (already in `seen_advisories[]`).
   - `K` total open rows currently in inbox.
   - Link: `docs/security/advisory-inbox.md` — Sếp gạt row open → processed/dismissed.

5. Exit code: 0 = success, 1 = input error (sentinel missing / inbox missing `## Rows`), 2 = processing error (parse/write). Full exit codes: `advisory-inbox --help`.

> Binary install: `cargo install --path ~/advisory-inbox --locked` (advisory-inbox not yet on crates.io as of CHANGELOG entry date). Version: `advisory-inbox --version`.
```

**Lưu ý:**
- Hard target: post-rewrite `wc -l ~/tarot/.claude/commands/advisory-scan.md` ≤ 30. Body itself ≤ 25 lines including code fences.
- Front-matter `description:` MUST be present (Anchor #11).
- **V2 correction:** the Bash code fence MUST OMIT `--report` (stdin is read when flag absent). DO NOT pass `--report -` — binary treats `-` as literal filename → exit 2 (confirmed Worker Turn 1 + Anchor #20).
- KHÔNG add `jq`, `awk`, `sed`, heredocs, or Bash conditionals. Binary handles parsing.
- If `~/.cargo/bin` not on PATH (Anchor #9), use full path `~/.cargo/bin/advisory-inbox` in step 2 instead of bare `advisory-inbox`.
- The Decision #2 example body is a template — Worker MAY rephrase prose for naturalness, MUST preserve all 5 numbered steps' semantic content.

### Task 5: Smoke test (Sub-mech A — mandatory)

**Approach A — Full integration (preferred):**
1. In a Claude Code session with `cwd = ~/tarot/`, invoke `/advisory-scan` (no args or with explicit `$ARGUMENTS` like a specific package).
2. Observe Claude:
   - Spawns `@agent-advisory-watch`.
   - Captures agent report.
   - Runs `advisory-inbox scan-and-append --inbox ... --state ...` via Bash tool (stdin piped, `--report` omitted per V2).
   - Surfaces JSON output + summary to Sếp (via Worker, in this context).
3. Verify side effects:
   - `~/tarot/docs/security/.advisory-scan-state` — `last_scan_at` updated to current ISO timestamp; `seen_advisories[]` count grew or stayed same.
   - `~/tarot/docs/security/advisory-inbox.md` — new rows appended under `## Rows` (if any new advisories found) OR file unchanged (if dedup skipped everything).
4. Capture JSON output verbatim in Debug Log.

**Approach B — Fallback (direct binary):**
1. Locate tarot's last real agent report export (Worker greps `~/tarot/docs/security/` + `~/tarot/docs/runlog/` for files containing `<!-- INBOX_APPEND_START -->`).
2. Pipe directly (V2: omit `--report` — stdin automatic):
   ```bash
   cat <last-report.md> | advisory-inbox scan-and-append \
     --inbox ~/tarot/docs/security/advisory-inbox.md \
     --state ~/tarot/docs/security/.advisory-scan-state
   ```
3. Capture exit code + full JSON output.

**Acceptance:**
- Exit code 0.
- JSON output parseable: `{ "appended": N, "skipped_dedup": M, "total_open": K }`.
- `K` (total_open) matches the count of rows with `status=open` in the inbox post-run.
- `seen_advisories[]` count in state file ≥ pre-install count (Sub-mech C).
- No partial write / corruption (atomic write per INV-LOCAL-002).

**Lưu ý:**
- Approach A preferred. Approach B is only acceptable IF Approach A blocked by environment (no advisory-watch agent available, no scan input). Discovery Report MUST log which approach used and why.
- Both approaches exercise Sub-mech A (trigger fires + behavior correct) and Sub-mech C (state preserved).
- If both Approach A and Approach B fail, Hard Stop per Anchor #12.

### Task 6: Tarot CHANGELOG entry

**File:** `~/tarot/docs/CHANGELOG.md` (per Anchor #5 — verified Worker Turn 1).

**Tìm:** top of file (newest-first convention).

**Thay bằng:** prepend per Decision #6 shape. Adjust to match tarot's existing CHANGELOG conventions (Worker reads top 50 lines pre-append).

**Mandatory content:**
- Date.
- Title: "Replaced 142-line Bash heredoc in `/advisory-scan` with advisory-inbox binary".
- Bullet: binary version (`0.1.0` per Anchor #3), install method (`cargo install --path ~/advisory-inbox --locked`).
- Bullet: state file migration action — pick exactly one phrase per Decision #3 outcome (V2 — 3 options now):
  - "already JSON v1 — no migration needed" (Path A), OR
  - "JSON-without-schema_version detected → ran `jq '. + {\"schema_version\": 1}'` to upgrade in place; existing seen_advisories[] entries preserved" (Path C — V2), OR
  - "migrated from legacy single-line ISO to JSON v1 via `advisory-inbox migrate-state`; backfilled N seen advisories via `advisory-inbox state-backfill` from inbox `processed`/`dismissed` rows" (Path B).
- Bullet: smoke test outcome — Approach A or B, link to `~/advisory-inbox/docs/discoveries/P013.md`.
- Bullet: old 142-line heredoc preserved via git history only (no archive file).

**Lưu ý:**
- Match tarot's existing entry shape (date format, heading level, bullet style).

### Task 7: Commit + PR in tarot

**Working directory:** `~/tarot/`.

**Run:**
```bash
cd ~/tarot
git checkout -b feat/advisory-inbox-binary-install
git add .claude/commands/advisory-scan.md docs/CHANGELOG.md
# If state file was migrated (Path B or Path C ran), do NOT git-add it — state files are runtime, typically gitignored. Verify with `git status` whether tarot tracks the state file. If tracked: add it; if untracked/ignored: leave.
git commit -m "feat: replace advisory-scan Bash heredoc with advisory-inbox binary"
git push -u origin feat/advisory-inbox-binary-install
gh pr create --title "feat: replace advisory-scan Bash heredoc with advisory-inbox binary" --body "<see below>"
```

**PR body (mandatory sections):**
```markdown
## Summary

Replace 142-line Bash heredoc in `/advisory-scan` slash command with thin invocation of `advisory-inbox` CLI (v0.1.0).

The heredoc was rebuilt 4 times (P282/P284/P285/P286 of soulsign tarot) due to Bash quote escape + jq arg passing + awk HTML-comment skip fragility. advisory-inbox replaces all this with a deterministic Rust binary (69 tests passing, 12 phiếu shipped P001-P012).

## Changes

- `.claude/commands/advisory-scan.md` — 142 lines → ~18 lines (procedural markdown that spawns `@agent-advisory-watch` then pipes report into `advisory-inbox scan-and-append` via stdin, `--report` flag omitted per V2 phiếu correction).
- `docs/CHANGELOG.md` entry.
- (Conditional, IF Path B or Path C ran) `docs/security/.advisory-scan-state` — upgraded to JSON v1 schema.

## State file migration

[Pick one based on Task 3 outcome]
- "State already JSON v1 — no migration needed (Path A)."
- "JSON-without-schema_version detected → ran `jq '. + {\"schema_version\": 1}'` to upgrade in place; existing seen_advisories[] entries preserved (Path C, P013 V2)."
- "State migrated legacy → JSON v1 via `advisory-inbox migrate-state`; N seen advisories backfilled from inbox `processed`/`dismissed` rows (Path B)."

## Smoke test

[Approach A or B per Task 5 outcome]
- Approach A: `/advisory-scan` invoked end-to-end in a Claude Code session; output: `{ "appended": N, "skipped_dedup": M, "total_open": K }`.
- Approach B: Direct binary pipe with last-export report from `<path>`; same output shape.

Sub-mech A (trigger fires) + Sub-mech C (state preserved) verified. Full trace: `~/advisory-inbox/docs/discoveries/P013.md`.

## Lane override

- original: guarded
- requested: guarded
- reason: N/A (no override)
- approved_by: N/A

## Old heredoc

Preserved via git history only (`git log -p .claude/commands/advisory-scan.md`). No archive file (per advisory-inbox P013 Decision #5).
```

**Lưu ý:**
- Branch base: `main` (per Anchor #17 — verified Worker Turn 1).
- Worker MUST also invoke `/security-review <PR>` post-push (per RULES.md §16 step 7 — Guarded lane mandatory). Orchestrator handles this in autonomous mode.
- KHÔNG `gh pr merge` from Worker. Sếp reviews + merges (or orchestrator after `/security-review` clean + autonomous approval).

### Task 8: Discovery Report (in advisory-inbox repo)

**Files:** `~/advisory-inbox/docs/discoveries/P013.md` (NEW) + `~/advisory-inbox/docs/DISCOVERIES.md` (prepend 1-line index entry, newest at top).

**Content (`docs/discoveries/P013.md`):**

```markdown
## Discovery Report — P013

### Assumptions trong phiếu — ĐÚNG:
- [List anchors verified ✅ at EXECUTE]

### Assumptions trong phiếu — SAI so với code thật:
- [Anchor X: phiếu V<N> ghi A, code/runtime thật là B → action taken]
- [V1 → V2 corrections already logged in Debate Log Turn 1; list any V2-on additional surprises here]
- [Nếu không có sai lệch → "Không có"]

### Edge cases / limitations phát hiện thêm:
- [Anchor #9 — was ~/.cargo/bin on PATH? Y/N + handling]
- [Tarot file path discrepancies vs Anchor #4 expectations (V2 confirmed paths exist — note any post-V2 surprises)]
- [Any other surprises during EXECUTE]

### Docs đã cập nhật theo discoveries:
- [Suggested P014 — enhance `advisory-inbox migrate-state` to auto-detect JSON-without-schema_version (Path C) and add schema_version: 1 idempotently. Out-of-scope for P013 per Turn 1 RESPOND.]
- [If tarot CLAUDE.md/README needed updates — Anchor #7 — list them]
- [Nếu không có → "Không có"]

### Layer 2 capability checks fired (Sub-mech A-F):
- A (trigger): [Approach A or B used; outcome]
- B (capability): cargo install exit code; --version output; --help confirms scan-and-append; stdin (omitted --report) test result
- C (migration): state format detected (Path A/B/C); seen_count before/after; jq pre-step OR migrate + backfill outcomes
- D (persistence): both repos updated (tarot CHANGELOG + tarot slash + advisory-inbox discoveries) — grep confirmation
- E (env drift): cargo clean + reinstall outcome; cargo update --dry-run outcome
- F (runtime state): grep token patterns in slash + CHANGELOG + .git/config

### Lane assignment + override (if any):
- Classifier output: Guarded (security slash command touch)
- Reason files: ~/tarot/.claude/commands/advisory-scan.md (security boundary)
- Override: no
- Security review: `/security-review <PR>` invoked + outcome

### Cross-repo notes:
- Tarot branch: feat/advisory-inbox-binary-install
- Tarot PR: <URL>
- Tarot HEAD before P013: [from snapshot]
- advisory-inbox HEAD: [git rev-parse HEAD when P013 discovery written]
- Heredoc line count before/after: 142 → <N>

### Pilot retrospective notes (Workflow v2.1):
- Debate Log Turn 1 caught 2 objections pre-EXECUTE (O1.1 shape: 3rd state format unknown to phiếu V1; O1.2 mechanical: `--report -` doesn't accept stdin). V2 RESPOND addressed both surgically. This is exactly the Workflow v2.1 Guarded-lane debate cycle working as intended.
- Note for sos-kit golden template: future architect prompts should include "verify CLI stdin idiom (`-` vs omitted flag) via `--help` BEFORE writing scan-and-append invocation samples". Logged here for Phase 5 retro per PROJECT.md §Roadmap.
```

**1-line index entry (`docs/DISCOVERIES.md`, prepend newest-at-top):**
```markdown
- 2026-MM-DD P013: tarot install complete — /advisory-scan 142→<N> lines, binary v0.1.0 installed via cargo install --path, state file <Path A no-op | Path C jq prestep schema_version added | Path B migrated+backfilled>, smoke test approach <A|B> exit 0, Sub-mech A/B/C/D/E/F all green; V1→V2 debate caught 3rd state format + --report stdin idiom → see docs/discoveries/P013.md
```

**Lưu ý:**
- Discovery Report is mandatory per CLAUDE.md DoD item 9 — even for cross-repo phiếu.
- This is the SOLE advisory-inbox repo change for P013 (plus the index entry). NO `src/` change. NO `Cargo.toml` change.
- If advisory-inbox `docs/discoveries/P013.md` lands on `main` directly (no PR), that's acceptable for docs-only files per repo convention. If a PR is preferred for audit, Worker opens a tiny `docs/P013-tarot-install` branch in advisory-inbox.

---

## Files cần sửa

| File | Repo | Thay đổi |
|------|------|---------|
| `~/tarot/.claude/commands/advisory-scan.md` | tarot | Task 4: rewrite — 142-line Bash heredoc → ~18-line procedural wrapper invoking `advisory-inbox scan-and-append` via stdin (`--report` flag OMITTED per V2). |
| `~/tarot/docs/CHANGELOG.md` | tarot | Task 6: prepend entry per Decision #6 (3-path migration option list per V2). |
| `~/tarot/docs/security/.advisory-scan-state` | tarot (runtime) | Task 3 (CONDITIONAL): Path A no-op / Path C `jq` add schema_version (V2) / Path B `migrate-state` + optional `state-backfill`. Branch determined by Task 2 format detection. |
| `~/advisory-inbox/docs/discoveries/P013.md` | advisory-inbox | Task 8: full Discovery Report (NEW). |
| `~/advisory-inbox/docs/DISCOVERIES.md` | advisory-inbox | Task 8: prepend 1-line index entry. |

## Files KHÔNG sửa (verify only)

| File | Repo | Verify gì |
|------|------|----------|
| `~/advisory-inbox/src/**` | advisory-inbox | NO code change. `cargo test --all` returns 69 tests pass (P011/P012 baseline). |
| `~/advisory-inbox/Cargo.toml` | advisory-inbox | NO change. Version stays `0.1.0`; deps unchanged. |
| `~/advisory-inbox/tests/**` | advisory-inbox | NO change. |
| `~/advisory-inbox/README.md`, `docs/ARCHITECTURE.md`, `docs/PROJECT.md`, `docs/RULES.md`, `docs/CHANGELOG.md` | advisory-inbox | NO change (P013 is install/integration phiếu — knowledge home is `docs/discoveries/P013.md`). |
| `~/tarot/src/**`, `~/tarot/lib/**`, any other tarot source | tarot | NO change. P013 is slash command rewrite only. |
| `~/tarot/.claude/commands/*` (other slash commands) | tarot | NO change. Only `advisory-scan.md` touched. |
| `~/tarot/.mcp.json` | tarot | NO change. P013 wires CLI invocation; MCP server wire-up is a future phiếu (`advisory-inbox serve` integration). |
| `~/tarot/docs/security/advisory-inbox.md` | tarot | NO direct text edit. May be modified by `scan-and-append` at Task 5 smoke test (binary owns atomic write). |
| `~/tarot/CLAUDE.md`, `~/tarot/README.md` | tarot | Anchor #7 verify only — already ✅ Worker Turn 1 (≤ 3 hits, no update needed). |
| `~/tarot/.claude/commands/.archive/` (anywhere) | tarot | DO NOT CREATE. Decision #5 — git is the archive. |

---

## Luật chơi (Constraints)

1. **Cross-repo phiếu.** Worker switches cwd between `~/advisory-inbox/` (for install + Discovery Report) and `~/tarot/` (for slash command rewrite + CHANGELOG + PR). Use absolute paths in all commands; verify cwd before each repo-specific step.
2. **NO advisory-inbox `src/`/`Cargo.toml`/`tests/` change.** P013 consumes advisory-inbox as a finished product. Bugs found in advisory-inbox during P013 EXECUTE → file follow-up phiếu, do NOT fix inline. (E.g., the Path C jq pre-step is a tarot-side workaround; a potential P014 to enhance `migrate-state` is out-of-scope here.)
3. **NO `cargo publish` (real).** `cargo install --path ~/advisory-inbox --locked` only. Real publish is Sếp's separate decision post-P013.
4. **NO `cargo install --force`** (Hard Stop #10). If existing install conflicts, `cargo uninstall advisory-inbox` first.
5. **NO archive file for old heredoc** (Decision #5). Git history is the archive.
6. **Sub-mech C — state file count MUST NOT decrease.** If `seen_advisories[]` post-install < pre-install at any point, restore from snapshot + escalate. Tarot users depend on dedup; regression here = noise flood.
7. **Sub-mech A — trigger MUST be verified** (Decision #4 Approach A or B). Marking phiếu done without verifying the new slash command actually fires is a doctrine violation (RULES.md §5 INV-WF-001 — "No trigger = not shipped").
8. **Binary stdin invocation MUST omit `--report` flag** (V2 correction per Worker Turn 1 O1.2 + Anchor #20). DO NOT pass `--report -` — binary treats `-` as literal filename and exits 2. Per `advisory-inbox scan-and-append --help`: "Omit for stdin". Slash command body, Approach B fallback, and Sub-mech B verification all use the omit-flag form.
9. **NO touching tarot files outside scope** (CLAUDE.md Hard Stop #6 — "Refactor code không liên quan đến phiếu"). Worker fixes ONLY `.claude/commands/advisory-scan.md` + tarot's `docs/CHANGELOG.md` (+ state file via Path A/B/C dispatch in Task 3).
10. **PR body MUST include Lane override section** (RULES.md §9), even if no override (write "N/A — no override").
11. **`/security-review <PR>` MUST run** post-push (RULES.md §16 Guarded lane step 7). Orchestrator triggers in autonomous mode.
12. **NO `git push --force`** anywhere (RULES.md §15). Standard `git push` only.
13. **NO `gh pr merge` from Worker.** Sếp or orchestrator merges after security review clean.
14. **All discoveries logged** (CLAUDE.md DoD item 9). Per-phiếu file + 1-line index. Even though no advisory-inbox code changed, the install procedure + cross-repo learnings + V1→V2 debate outcomes are durable knowledge.
15. **Tầng 2 self-decide allowed for:** exact slash command prose wording, CHANGELOG entry exact prose, whether to use full path `~/.cargo/bin/advisory-inbox` vs bare `advisory-inbox` in slash body (Anchor #9), Approach A vs B for smoke test (when both viable, A preferred). CHANGELOG file location ALREADY decided (`~/tarot/docs/CHANGELOG.md` per Anchor #5 ✅).

---

## Nghiệm thu

### Automated

- [ ] `cd ~/advisory-inbox && cargo install --path . --locked` — exit 0 (Sub-mech B).
- [ ] `advisory-inbox --version` outputs `advisory-inbox 0.1.0` or version per Cargo.toml (Anchor #3).
- [ ] `advisory-inbox --help` lists `scan-and-append` subcommand (Sub-mech B).
- [ ] `cd ~/advisory-inbox && cargo test --all` — 69 tests pass (unchanged — no advisory-inbox code change).
- [ ] `cd ~/advisory-inbox && cargo clippy --all-targets -- -D warnings` — clean (unchanged).
- [ ] `cd ~/advisory-inbox && cargo fmt --check` — no diff (unchanged).

### Manual Testing

- [ ] `wc -l ~/tarot/.claude/commands/advisory-scan.md` — outputs ≤ 30 (was 142; new body ≤ 25 lines + ~5 lines front-matter).
- [ ] `grep -c "advisory-inbox scan-and-append" ~/tarot/.claude/commands/advisory-scan.md` — outputs ≥ 1 (new wrapper invokes binary).
- [ ] `grep -c "INBOX_APPEND_START" ~/tarot/.claude/commands/advisory-scan.md` — outputs ≥ 1 (Reference to sentinel marker preserved in step 1).
- [ ] `grep -c "@agent-advisory-watch" ~/tarot/.claude/commands/advisory-scan.md` — outputs ≥ 1 (agent spawn step preserved).
- [ ] `grep -c "\-\-report -" ~/tarot/.claude/commands/advisory-scan.md` — outputs `0` (V2 — `--report -` form is INCORRECT, must not appear).
- [ ] Smoke test (Task 5 Approach A or B) — exit 0, JSON output parseable, side effects observed (state updated, inbox grew or unchanged with dedup correct).
- [ ] `jq '.schema_version' ~/tarot/docs/security/.advisory-scan-state` — outputs `1` (JSON v1 post-any-migration / Path A/B/C all yield this).
- [ ] `jq '.seen_advisories | length' ~/tarot/docs/security/.advisory-scan-state` — outputs ≥ pre-install count (Sub-mech C; ≥ 2 if Path C ran).
- [ ] No `~/tarot/.claude/commands/.archive/` directory created (Decision #5).

### Regression

- [ ] `~/tarot/docs/security/advisory-inbox.md` still has `## Rows` heading (Task 5 atomic write didn't corrupt).
- [ ] `~/tarot/docs/security/advisory-inbox.md` row format unchanged (8 pipe-delimited columns — Date / Advisory ID / Source URL / Package / File:Line / Severity / Status / Note per ARCHITECTURE §3).
- [ ] Other tarot slash commands in `~/tarot/.claude/commands/` untouched (`ls ~/tarot/.claude/commands/` shape same except `advisory-scan.md` content changed).
- [ ] advisory-inbox repo `cargo test --all` still 69 tests pass (no src/ change).
- [ ] No `unsafe { }` block added (Hard Stop #7 — N/A since no src/ change anyway).

### Docs Gate

- [ ] `~/tarot/docs/CHANGELOG.md` — P013 entry prepended per Decision #6 + Task 6.
- [ ] `~/advisory-inbox/docs/discoveries/P013.md` — full Discovery Report written (Task 8).
- [ ] `~/advisory-inbox/docs/DISCOVERIES.md` — 1-line P013 index entry prepended.
- [ ] `~/advisory-inbox/docs/ARCHITECTURE.md` — untouched (no advisory-inbox source change; no new tool/subcmd).
- [ ] `~/advisory-inbox/docs/CHANGELOG.md` — untouched (P013 doesn't ship advisory-inbox code).
- [ ] Tarot CLAUDE.md/README — Anchor #7 verified clean (Worker Turn 1 ✅).
- [ ] `docs-gate --all --verbose` (in advisory-inbox repo) — pass.

### Discovery Report

- [ ] `~/advisory-inbox/docs/discoveries/P013.md` — full report covers:
  - Tarot file paths verified (Anchors #4, #5, #7, #17, #18 outcomes — all ✅ Worker Turn 1).
  - State file format detected (Path A / Path B / Path C) + outcome of dispatch.
  - Heredoc line count: 142 → <N>.
  - Cargo install outcome (size, version).
  - Smoke test approach used (A or B) + JSON output captured.
  - Anchor #9 PATH outcome + slash-body invocation form (`advisory-inbox` vs `~/.cargo/bin/advisory-inbox`).
  - V1 → V2 debate Turn 1 outcomes (O1.1 ACCEPT Path C added; O1.2 ACK `--report` omitted).
  - Sub-mech A/B/C/D/E/F verification trace (table above filled).
  - Cross-repo notes: tarot branch + PR URL + heads.
  - Pilot retrospective notes (Workflow v2.1 Guarded debate cycle worked as intended).
- [ ] `~/advisory-inbox/docs/DISCOVERIES.md` — 1-line entry, newest-at-top.
- [ ] Sub-mechanism A-F Verification Trace table filled (above) with actual results.
- [ ] `/security-review <PR>` invoked on tarot PR, outcome logged in Discovery Report.

---

**End P013.**
