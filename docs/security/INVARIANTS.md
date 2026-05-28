# INVARIANTS — advisory-inbox

> Project-local invariant catalog consumed by Giám sát (boundary-check) via `/security-review`.
> Generic INV are baked into `agents/boundary-check.md`. Workflow + Local INV extend per advisory-inbox domain.

---

## §1. Generic INV (kit-level baseline)

### INV-1 — New env var → allowlist update

**Statement:** PR thêm new env var read (`std::env::var("X")`, `env!("X")`) PHẢI update `.tools/runtime-env.allowlist` với key mới trong correct group (required/optional/forbidden).

**Trigger keywords (Rust):** `std::env::var\(['\"]`, `env!\(['\"]`, `option_env!`.

**Status:** Active.

### INV-2 — New external call → timeout + error handling

**Statement:** PR thêm new HTTP/external call PHẢI có explicit timeout AND error-handling.

**Trigger keywords:** `reqwest::`, `hyper::`, `surf::`, `tokio::net::`, `tokio::process::Command`.

**Per-call check:** `.timeout()` on client OR per-request, AND `.await?` (or explicit `Err` match).

**Status:** ⚠️ N/A in Phase 1-3 (advisory-inbox = pure-local CLI, no external calls). Activates if Phase 4+ adds telemetry/upstream.

### INV-5 — Dep major bump → changelog audit

**Statement:** PR bumps any `Cargo.toml` dependency's MAJOR version PHẢI cite changelog review trong PR description.

**Trigger keywords:** `Cargo.toml` `[dependencies]` diff showing MAJOR component change.

**Status:** Active.

---

## §2. Workflow INV (v2.1 doctrine)

### INV-WF-001 — Trigger Verifiability

**Statement:** Mọi workflow tool / hook / classifier / doctor ship phải có:
1. **Trigger structure declared** — hook config / cron / launchd / orchestrator auto-spawn / pre-commit
2. **Trigger fires** — verify smoke test dry-run
3. **Behavior assert** — exit code đúng, output đúng, side-effect đúng

**Slogan:** *Build hook → test fire → assert behavior. No trigger = not shipped.*

**Source:** `~/sos-kit/docs/WORKFLOW_V2.1.md` §8. Sub-mechanism A (Trigger gap) crystallized.

**Layer 2 capability check example:**
```bash
# Hook smoke test
echo '{"tool_name":"Edit","tool_input":{"file_path":".env.production"}}' \
  | bash hooks/block-env-edit.sh
echo "exit=$?"
# expect: exit=2 + stderr contains "blocked: .env*"
```

**Anti-pattern:**
- Hook file tồn tại nhưng chưa `chmod +x`
- Cron declared nhưng `if: false`
- MCP configured nhưng OAuth chưa connected
- Function exported nhưng caller chưa import
- Rule tồn tại nhưng orchestrator quên triệu (P297 tarot instance #9)

**Trigger keywords for boundary-check:** new hook addition in `.claude/settings*.json`, new MCP server in `.mcp.json`, new cron entry, new slash command `.claude/commands/*.md` without Layer 2 smoke test.

**Status:** Active.

### INV-WF-002 — Knowledge durability home declared

**Statement:** Mọi rule / doctrine / workflow rule mới ship PHẢI có home explicit (1 trong 4 tier per RULES.md §6). Commit message PHẢI declare home.

**Trigger keywords:** new rule/policy added to CLAUDE.md / docs/RULES.md / docs/security/INVARIANTS.md without matching commit message `home:` line.

**Source:** Sub-mechanism D (Persistence lifecycle gap).

**Status:** Active.

---

## §3. Local INV (advisory-inbox-specific)

### INV-LOCAL-001 — `unsafe { }` block requires explicit rationale

**Statement:** PR introducing any `unsafe { }` block PHẢI include comment block:
1. Why safe Rust alternative was rejected
2. What invariants the unsafe code requires caller to uphold
3. Reference to `#[test]` exercising the unsafe path

**Why:** advisory-inbox is a local CLI tool. Almost never legitimate reason for unsafe. Standing rejection unless rationale bulletproof.

**Trigger keywords:** `unsafe\s*{`, `unsafe fn`, `unsafe impl`.

**Status:** Active.

### INV-LOCAL-002 — Atomic write protocol (inbox + state)

**Statement:** Any write to `docs/security/advisory-inbox.md` (target project) OR `.advisory-scan-state` (state file) PHẢI use temp+fsync+rename protocol:

```rust
let temp = NamedTempFile::new_in(target.parent()?)?;  // SAME filesystem
temp.write_all(content)?;
temp.as_file().sync_all()?;  // fsync data + metadata
temp.persist(target)?;  // atomic rename
```

**Forbidden:** `OpenOptions::append(true)` against target. `std::fs::write` direct (no temp+rename).

**Why:** Inbox + state are durable observability records. Partial write = corruption = Sếp's review loop broken. Same-filesystem temp file required (cross-fs rename = copy+delete, loses atomicity).

**Trigger keywords:** `OpenOptions::append`, `std::fs::write` near inbox/state paths, `std::fs::rename` outside `tempfile::persist`.

**Status:** Active. Inherits INV-21 of advisory-cron pattern.

**Concrete users:**
- P006 — `src/inbox.rs::write_atomic` (inbox markdown write path, shipped 2026-05-28). Reference shape established.
- P007 — `src/state.rs::write_atomic` (state JSON write path, shipped 2026-05-28). Reference shape mirrored exactly from P006.

Future state-write subcmds (P008 state-backfill, P009 scan-and-append, P011 MCP `append` tool) MUST follow same protocol.

**Note on user-supplied path:** `--inbox <FILE>` argument is user-controlled. Worker / Sếp PHẢI ensure path points to intended inbox markdown file (typo could write to wrong file). Atomic protocol ensures partial-write safety; it does NOT validate semantic intent. No file-content echoing into stderr/log (Sub-mech F clean).

### INV-LOCAL-003 — JSON serialization via serde_json (no manual)

**Statement:** PR introducing code that serializes structured data to JSON PHẢI use `serde_json::to_string` (or `to_writer`) via `#[derive(Serialize)]` — NEVER hand-rolled JSON string assembly.

**Why:** Manual JSON assembly skips escape step. Row data (vendor names, file paths, advisory descriptions) may contain `"` / `\` / Unicode → manual format produces invalid JSON or injection-prone.

**Trigger keywords:** `format!("{{\"...\":...}}")`, `write!(json, ...)` with non-escaped values.

**Status:** Active.

### INV-LOCAL-004 — MCP transport boundary

**Statement:** PR introducing MCP tool handlers (`src/mcp/tools.rs`) PHẢI:
1. Validate every input field at tool boundary BEFORE invoking core logic — path traversal scan (`..`), input length bounds, expected enum values.
2. Tool result serialization via `serde_json` (INV-LOCAL-003).
3. Tool handler errors propagate as MCP error objects (NOT process exit) — only transport-level errors escape to `serve` and map to exit code 5.

**Why:** MCP boundary = external attack surface. CLI was trusted (user owns keyboard). MCP callable by any process Claude talks to.

**Trigger keywords:** new MCP tool handler in `src/mcp/tools.rs`, `rmcp::ServerHandler` impls.

**Status:** Active.

### INV-LOCAL-005 — Sentinel marker format rename forbidden

**Statement:** PR renaming sentinel markers `<!-- INBOX_APPEND_START -->` / `<!-- INBOX_APPEND_END -->` PHẢI bump state schema_version AND provide migration tool. These markers are load-bearing — agent reports + slash commands depend on exact string match.

**Why:** Sentinel = wire contract between agent + advisory-inbox + downstream tooling. Rename without migration breaks all in-flight reports.

**Trigger keywords:** `INBOX_APPEND_START` / `INBOX_APPEND_END` literal string in diff, `sentinel` module changes.

**Status:** Active.

---

## §4. Sub-mechanism F — Runtime State Preflight

(Cross-reference: RULES.md §8.)

**Statement:** Source code clean (mechanical scan pass) ≠ runtime state clean. Hidden surfaces:
- `.git/config` token-in-URL
- `printenv` leak forbidden keys
- `~/.ssh/config` credential mismatch
- Untracked `.env*` with secrets

**Layer 2 checks (mandatory in `scripts/session-start-banner.sh`):**

```bash
# Check 1 — git config token leak
git config --get-regexp 'http.*extraheader|credential|insteadOf' \
  | grep -qE 'ghp_|gho_|ghu_|ghs_|github_pat_' \
  && echo "BLOCK: token-in-config" >&2 && exit 2 \
  || true

# Check 2 — env allowlist enforcement
yq '.forbidden | .[]' .tools/runtime-env.allowlist | while read key; do
  if printenv | grep -q "^${key}="; then
    echo "BLOCK: forbidden runtime key $key" >&2
    exit 2
  fi
done

# Check 3 — gh auth status (no credential mismatch)
gh auth status 2>&1 | head -3
```

**Status:** Active. Trigger fires SessionStart hook.

---

## §5. How INV are checked

1. Worker pushes PR
2. Quản đốc auto-invokes `/security-review <PR>` if Guarded/Locked lane
3. Slash command captures diff via `gh pr diff` and spawns Giám sát
4. Giám sát checks generic INV (rubric baked in `.claude/agents/boundary-check.md`)
5. Local INV (LOCAL-001..005) are documented for human review + Worker self-check during EXECUTE
6. Slash command parses sentinel-wrapped verdict + posts as PR comment (silent if APPROVE + 0 FLAG)
7. **ADVISORY mode:** verdict does NOT block merge. Sếp/orchestrator gates.

## §6. Why ADVISORY (not blocking)

- Generic INV can over-flag (false positives in domain code)
- Discipline > automation: Sếp reading comment + deciding = stronger signal than CI pass
- Future: extend to block on FLAGd INV — but kit ships ADVISORY default

## §7. Sentinel marker contract

Giám sát returns verdict wrapped in `<!-- security-review-start -->` ... `<!-- security-review-end -->`. Markers LOAD-BEARING — slash command grep-extracts the block. DO NOT rename without phiếu.

(INV-LOCAL-005 enforces sentinel marker stability for INBOX block — SAME rule applies to security-review markers.)
