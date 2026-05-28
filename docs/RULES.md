# RULES — advisory-inbox

> **Workflow v2.1 doctrine** enforce spec. CLAUDE.md gives tóm tắt; this file gives chi tiết operational.
> **Source of truth:** `~/sos-kit/docs/WORKFLOW_V2.1.md` (durable, không rotate).
> **Pilot status:** advisory-inbox là test bed v2.1. Mọi rule áp dụng, KHÔNG bypass.

---

## §1. Lane Routing — 4 tier

Mỗi PR đi đúng 1 lane. Lane do **classifier deterministic** quyết (v2.1 §9). Trong pilot phase, classifier binary chưa ship → orchestrator LLM tạm classify theo bảng dưới, log reason vào PR body.

### Fast lane

**Scope cứng (whitelist):**
- Docs typo fix
- Comment polish (rewording không đổi semantics)
- README updates không touch invariants/security
- Non-architecture cleanup (trailing whitespace, import order)

**KHÔNG Fast lane (blacklist):**
- Scaffold (chốt module boundary, naming, CLI entry)
- Module boundary change (move file giữa folders)
- CLI entry shape (add/remove subcommand, change arg signature)
- Config root (add key, change default)
- Public API shape (export new, change signature)
- Dependency add (cargo add — KHÔNG Fast lane kể cả minor)

**Flow:** `Worker → tests → merge` (skip Architect + skip security review)

### Normal lane

**Scope:**
- Config schema change
- State schema change
- Pure internal behavior refactor
- Scaffold (per §Fast Lane blacklist)
- Dep version bump minor/patch

**Flow:** `Architect short → Worker challenge Turn 1 → Execute → tests → review-lite`

### Guarded lane

**Scope:**
- Process execution (`Command::spawn`)
- Filesystem persistence (write file ngoài `target/`, `.advisory-scan-state`, `docs/runlog/`)
- Outbound HTTP (chưa có trong advisory-inbox MVP, future surface)
- Token/secret handling
- MCP server logic (rmcp transport, tool dispatch)
- Auth flow (chưa có)
- AI prompt change (chưa có)

**Flow:** `Architect full → Worker CHALLENGE → RESPOND → Worker Turn 2 surgical → Execute → security-review-full → approval gate`

### Locked lane

**Scope:**
- Destructive migration (state file rewrite không backup)
- Auto-fix PR (KHÔNG cho phép — Sếp giữ van người-gate)
- Schema breaking change cross-version

**Flow:** Full Guarded + **human approval explicit BEFORE Architect spawn** + **mandatory dry-run** + **rollback plan in ticket**

---

## §2. Objection Taxonomy — 3 loại

Worker CHALLENGE phải tag objection bằng 1 trong 3:

### Mechanical objection
**Triệu chứng:** sai test count, sai path, sai baseline, sai wording nhỏ, KHÔNG đổi behavior/API/security.

**Flow:**
```
Worker: objection [mechanical] "Architect đếm test X, thật Y"
Orchestrator: route to Architect (KHÔNG tự amend)
Architect: ack one-line correction
Worker Turn 2: OPTIONAL skip nếu correction không ảnh hưởng behavior
```

### Implementation-shape objection
**Triệu chứng:** function signature sai, enum/newtype shape sai, return type sai, module boundary sai.

**Flow:**
```
Worker: objection [shape]
Architect: RESPOND short — chỉ address objection
Worker Turn 2: SURGICAL verify exact V2 changes
```

### Design/security objection
**Triệu chứng:** spec không khả thi platform thật, mở surface mới, secret risk, process treo, persistence không an toàn.

**Flow:**
```
Worker: objection [design]
Architect: RESPOND full — re-read INVARIANTS + spec doable
Worker Turn 2: BROAD re-verify
Security review: full nếu chạm Guarded lane
Approval gate: required
```

---

## §3. Surgical Turn 2

```
Turn 1 challenge = BROAD verification (full V1 review)
Architect RESPOND   = ONLY address objections (không mở scope)
Turn 2 challenge   = SURGICAL — chỉ V2 changes
                    → KHÔNG re-open whole debate
```

**Exception:** Design/security objection (§2.3) → Turn 2 broad nếu V2 thay đổi scope/surface.

---

## §4. Tool Availability Preflight — Architect Bước 0

Trước khi Architect viết phiếu chứa instruction "use tool X":

1. Verify tool X có trong tool envelope không (frontmatter `tools:` agent)
2. Nếu KHÔNG → ghi `[needs Worker Task 0 verify via <fallback>]`. KHÔNG ghi "MUST research via tool X".
3. Nếu CÓ → smoke test query nhỏ. Fail → treat như "không có".

**Anti-pattern:** Architect ghi "use context7 to verify rmcp API" trong khi runtime không có context7 → false confidence.

---

## §5. INV-WF-001 — Trigger Verifiability

> **Build hook → test fire → assert behavior.**
> **No trigger = not shipped.**

Mọi workflow tool / hook / classifier / doctor ship phải có:
1. **Trigger structure declared** — hook config / cron / launchd / orchestrator auto-spawn / pre-commit
2. **Trigger fires** — verify smoke test dry-run
3. **Behavior assert** — exit code đúng, output đúng, side-effect đúng

**Layer 2 capability check example (block-env-edit hook):**
```bash
echo '{"tool_name":"Edit","tool_input":{"file_path":".env.production"}}' \
  | bash hooks/block-env-edit.sh
# expect: exit=2 + stderr contains "blocked: .env*"
```

**Anti-pattern:**
- Hook file tồn tại nhưng chưa `chmod +x`
- Cron declared nhưng `if: false` chưa flip
- MCP configured nhưng OAuth chưa connected
- Function exported nhưng caller chưa import
- Rule tồn tại nhưng orchestrator quên triệu

**Reserved IDs:** `INV-WF-001` (this), future `INV-WF-002+` post-pilot.

---

## §6. Knowledge Durability — 4 tier

Mỗi rule/fact mới PHẢI có home explicit. KHÔNG home = chưa ship.

| Tier | Type | Home | Rotate? |
|------|------|------|---------|
| **Durable doctrine** | Rule, invariant, taxonomy, pattern catalog | `CLAUDE.md` / `docs/RULES.md` / `docs/security/INVARIANTS.md` / `.claude/agents/*.md` | **KHÔNG** |
| **Operational discoveries** | Instance debug, test failure narrative, anchor fix | `docs/DISCOVERIES.md` + `docs/discoveries/P<NNN>.md` | Khi > 1000 dòng → `docs/Archive/DISCOVERIES_ARCHIVE.md` |
| **Project debt** | Backlog item, deferred fix, advisory note | `docs/BACKLOG.md` | Item done → strikethrough, archive quarterly |
| **Run-specific facts** | Heartbeat, runlog, scan state | `docs/runlog/*.jsonl` / `.advisory-scan-state` | Per-run hoặc daily |

**Rule:** mỗi rule mới khai báo home trong commit message:
```
feat(P042): add foo

home: docs/RULES.md §X (durable)
```

---

## §7. Layer 2 Capability Check Matrix — Sub-mech A-F

Mọi phiếu MUST run applicable check trong EXECUTE Task 0:

| Sub-mech | Symptom | Check command | Expected |
|----------|---------|---------------|----------|
| **A — Trigger gap** | thing exists, nothing pulls trigger | smoke test hook fire `echo '<input>' \| bash <hook>` | exit code đúng + stderr match |
| **B — Capability gap** | spec ≠ runtime tool | `cargo check` | exit 0 |
| | | `cargo test <module>` | targeted pass |
| **C — Migration completeness** | schema migrated, old data lost | `jq '.field \| length' state.json` before/after | counts match (or grow) |
| **D — Persistence lifecycle** | doctrine in rotate-prone | `grep -l "<rule>" CLAUDE.md docs/RULES.md` | ≥1 hit persistent |
| **E — Environment drift** | local pass ≠ fresh-install | `cargo update --dry-run` | no surprise bump |
| | | `cargo build --release` from clean `target/` | exit 0 |
| **F — Runtime state gap** | source clean, runtime dirty | `grep -E 'ghp_\|gho_\|ghu_\|ghs_\|github_pat_' .git/config` | 0 hits |
| | | `bash scripts/session-start-banner.sh` | no forbidden key detected |

Fail → Discovery Report records it. Decide: fix in this phiếu OR escalate follow-up phiếu.

---

## §8. Runtime State Preflight (Sub-mech F)

### File `.tools/runtime-env.allowlist`

Project root. Committed (no values, key names only).

```yaml
required:
  - GITHUB_PERSONAL_ACCESS_TOKEN

optional:
  - RUST_LOG
  - CARGO_HOME

forbidden:
  - GITHUB_TOKEN     # collision with gh CLI keychain auth — use env -u
  - AWS_ACCESS_KEY_ID  # project doesn't use AWS
  - OPENAI_API_KEY     # project doesn't call OpenAI
```

### Session-start check (3 layer)

`scripts/session-start-banner.sh`:

```bash
# Layer 1 — Forbidden key hard block + unexpected key warn
# Layer 2 — gh auth status
# Layer 3 — git config token-in-URL scan (Sub-mech F precedent)
```

**KHÔNG log secret value.** Chỉ log key names + counts.

### Trigger structure (INV-WF-001)

- **SessionStart hook** (`.claude/settings.local.json`)
- **Pre-commit hook** for `.tools/runtime-env.allowlist` changes

---

## §9. Lane Override Audit

### PR body section (required)

`.github/pull_request_template.md` include:

```markdown
## Lane override

- original: N/A
- requested: N/A
- reason: N/A (no override)
- approved_by: N/A
```

Khi override:
```markdown
## Lane override

- original: guarded
- requested: normal
- reason: <explicit reason>
- approved_by: orchestrator
```

### Commit tag (optional, defense-in-depth)

```
feat(P042): foo

[lane-override: original=guarded requested=normal reason="docs-only"]
```

### Grep tooling

```bash
gh pr view <N> --json body | jq -r .body | awk '/^## Lane override/,/^## /' | head -10
git log --grep="lane-override" --oneline
```

**Threshold:** override rate > 20% across last 50 PRs → classifier rule sai, tune.

---

## §10. Orchestrator Scope — KHÔNG amend technical

Orchestrator (Quản đốc) **KHÔNG** tự sửa technical content:
- KHÔNG sửa test count
- KHÔNG sửa function signature
- KHÔNG sửa path
- KHÔNG sửa wording technical
- KHÔNG sửa code/config

**Được làm:**
- Classify objection (mechanical / shape / design — §2)
- Route to đúng agent
- Maintain state machine (DRAFT → CHALLENGE → RESPOND → APPROVAL_GATE → EXECUTE)
- Halt nếu loop/deadlock
- Escalate Sếp khi gate fail

---

## §11. Docs Gate — 2 Tầng detail

### Tầng 1 (CỨNG) — thiếu update = KHÔNG commit

| Type of change | Required docs update |
|----------------|----------------------|
| CLI subcommand added/removed/renamed | `docs/ARCHITECTURE.md` §CLI surface + `README.md` quick-start |
| CLI flag added/removed/renamed | `docs/ARCHITECTURE.md` §CLI surface |
| Exit code added or semantic change | `docs/ARCHITECTURE.md` §CLI surface exit codes |
| `Cargo.toml` `[dependencies]` add/remove | `docs/CHANGELOG.md` entry citing crate + reason |
| State file JSON schema change | `docs/ARCHITECTURE.md` §State schema + migration note in CHANGELOG if breaking |
| Inbox markdown format change (sentinel marker, row column order) | `docs/ARCHITECTURE.md` §Inbox format + CHANGELOG |
| Module added/removed | `docs/ARCHITECTURE.md` §Modules table |
| MCP tool added/removed | `docs/ARCHITECTURE.md` §MCP surface |
| Security boundary touched (env var read, file write outside whitelist) | **AUTO Tầng 1.** `docs/security/INVARIANTS.md` review + CHANGELOG entry |
| New `unsafe { }` block | **AUTO Tầng 1.** `docs/security/INVARIANTS.md` rationale + CHANGELOG |
| Workflow doctrine update (this file, CLAUDE.md, agent handbook) | **AUTO Tầng 1.** Bump version note in CHANGELOG |

### Tầng 2 (mềm) — không block commit

- Local variable / parameter name rename
- Internal error message wording (non-CLI-facing)
- Internal log span name / level
- Comment edits
- Doc typo fix
- Code style (rustfmt)
- Adding tests without changing prod code

---

## §12. Hard Stops — DỪNG NGAY, HỎI SẾP

Worker đang EXECUTE gặp 1 trong các tình huống → STOP, escalate `AskUserQuestion`:

1. **Thêm module / file mới** ngoài scope phiếu
2. **Thêm dependency** không có trong phiếu (`Cargo.toml`)
3. **Đổi CLI interface** (subcommand, flag, exit code) ngoài scope
4. **Đổi state schema** (JSON layout, version field) ngoài scope
5. **Đổi inbox format** (sentinel markers, row columns) ngoài scope
6. **Refactor code không liên quan** đến phiếu
7. **Write `unsafe { }`** even if "obviously safe" — escalate
8. **Force-push** to recover from rebase conflict
9. **Delete `.advisory-scan-state` runtime files**
10. **`cargo install --force`** outside phiếu's worktree
11. **Edit `.claude/settings.local.json`** UNLESS phiếu explicitly lists it
12. **`rm -rf` on absolute paths** or `~/`
13. **Edit `.tools/runtime-env.allowlist` `forbidden:`** section (security-critical, requires Sếp ack)

For each: `AskUserQuestion` với options A. abandon op / B. Sếp executes manually / C. update phiếu scope (return to Architect).

---

## §13. Discovery Report Format (mandatory)

**Per-phiếu file:** `docs/discoveries/P<NNN>.md`

```markdown
## Discovery Report — P<NNN>

### Assumptions trong phiếu — ĐÚNG:
- [Liệt kê từng assumption khớp code thật]

### Assumptions trong phiếu — SAI so với code thật:
- [Assumption X: phiếu ghi A, code thật là B → đã sửa docs]
- [Nếu không có sai lệch → "Không có"]

### Edge cases / limitations phát hiện thêm:
- [Phiếu không đề cập nhưng phát hiện khi đọc/sửa code]
- [Nếu không có → "Không có"]

### Docs đã cập nhật theo discoveries:
- [File nào đã sửa, sửa gì]
- [Nếu không có → "Không có"]

### Layer 2 capability checks fired (Sub-mech A-F):
- [List sub-mechanism check ran + result]

### Lane assignment + override (if any):
- Classifier output: <lane>
- Reason files: <list>
- Override: <yes/no, reason if yes>
```

**Index entry** in `docs/DISCOVERIES.md` (newest at top):
```markdown
- 2026-MM-DD P<NNN>: <one-line summary>, <key finding> → see docs/discoveries/P<NNN>.md
```

---

## §14. Commit Sequence

```
1. Code changes (tested pass per Step Gate)
2. Update docs/CHANGELOG.md (Tầng 1 entry minimum)
3. Update docs/ARCHITECTURE.md (Tầng 1 sections per §11 matrix)
4. Update CLAUDE.md if conventions changed (rare)
5. Write Discovery Report (per-phiếu file + 1-line DISCOVERIES.md index)
6. git add <specific files>  # KHÔNG git add -A blindly
7. cargo build --release && cargo test --all && cargo clippy --all-targets -- -D warnings && cargo fmt --check
8. git commit -m "<type>(P<NNN>): <summary>"
9. git push
10. gh pr create với PR body include §9 Lane override section
```

---

## §15. Git Workflow Safety

| Operation | Allowed | Forbidden |
|-----------|---------|-----------|
| `git push <branch>` | ✅ | `git push --force` / `-f` |
| `git reset --hard` | only inside phiếu worktree | outside phiếu worktree |
| `git checkout -b` | ✅ for new phiếu branch | overwriting `main` |
| `git rebase main` | ✅ to update phiếu branch | rebase main onto branch |
| `git merge --squash` | post-PR approval | direct to main without PR |
| `gh pr merge` | only after `/security-review` if Guarded lane | bypass security review |

---

## §16. Phiếu Lifecycle

1. **CLASSIFY** — Orchestrator (LLM tạm, classifier binary tương lai) phân lane. PR body section khai báo lane.
2. **DRAFT** — Architect writes phiếu V1 in `docs/ticket/P<NNN>-<slug>.md`. Header mandatory: `Lane: <fast|normal|guarded|locked>` + `Tầng: 1|2`.
3. **CHALLENGE** (Normal/Guarded/Locked only) — Worker reads phiếu + grep-verifies anchors + writes Debate Log Turn 1 với objection tag (§2).
4. **RESPOND** — Architect responds per objection taxonomy. Mechanical → short ack. Shape → V2 short. Design → V2 full.
5. **APPROVAL_GATE** — orchestrator narrate (autonomous) or `AskUserQuestion` (interrupted).
6. **EXECUTE** — Worker Task 0 (Layer 2 checks per §7) → codes → tests → Discovery Report → commits → pushes.
7. **SECURITY_REVIEW** (Guarded/Locked) — orchestrator invokes `/security-review <PR>`.
8. **MERGE** — Sếp or orchestrator merges PR after green CI + APPROVE.
9. **CLEANUP** — branch deleted, phiếu moved to `Recently shipped`.

---

## §17. max_attempts Policy (reserve for future retry surface)

advisory-inbox MVP KHÔNG có retry mechanism. Khi future phiếu thêm retry (e.g., MCP transport reconnect, state file write retry), apply rule:

| `max_attempts` | Action |
|----------------|--------|
| `< 1` | Hard reject |
| `1` | Default OK (no retry) |
| `2..5` | OK |
| `>= 6` | Soft warn |
| `> 10` | Hard reject |

**Backoff:** `<= 3600` seconds. No sleep after final attempt. Alert ONCE post-final.

---

**End RULES v2.1.**
