# CLAUDE.md — advisory-inbox

> Đọc file này TRƯỚC KHI làm bất cứ gì.
> Đọc `docs/PROJECT.md` để hiểu toàn bộ dự án.
> Đọc `docs/BACKLOG.md` để biết Sếp đã commit làm gì.
> Đọc `docs/CHANGELOG.md` để biết đã làm gì rồi.
> Đọc `docs/ARCHITECTURE.md` để hiểu code hiện tại.
> Đọc `docs/ticket/` để xem phiếu giao việc.
> Tra `docs/RULES.md` khi cần enforcement chi tiết — **RULES.md chứa Workflow v2.1 doctrine đầy đủ**.
> Tham chiếu spec gốc: `~/sos-kit/docs/WORKFLOW_V2.1.md` (durable doctrine, KHÔNG rotate).

---

## ⛔ PILOT NOTICE — repo này là test bed cho Workflow v2.1

Repo `advisory-inbox` là **pilot** đầu tiên cho Workflow v2.1 (sos-kit golden template tương lai). Mọi quyết định/rule áp dụng theo `docs/RULES.md` (port từ `~/sos-kit/docs/WORKFLOW_V2.1.md`).

Sếp chạy autonomous end-to-end zero-check. Bug lộ ra qua sprint sẽ feed vào retrospective → patch sos-kit → re-port lên Tarot + 2 repo còn lại (`claude-hooks`, `inv-gate`).

**KHÔNG bypass rule v2.1 nào dù scope nhỏ.** Mỗi PR phải:
- Có Lane field (Fast/Normal/Guarded/Locked) — xem `docs/RULES.md` §Lane Routing
- Tag objection theo taxonomy 3-loại (mechanical/shape/design-security)
- Nếu Architect dùng tool ngoài envelope → mark `[needs Worker verify]`
- Hook/trigger ship phải có Layer 2 capability check (INV-WF-001)

---

## ⛔ DEFINITION OF DONE — ĐỌC ĐẦU TIÊN, NHỚ SUỐT ĐỜI

**Mỗi phiếu chỉ được báo "XONG" khi TẤT CẢ điều kiện sau đã hoàn thành:**

```
1. ✅ cargo build --release (zero warnings)
2. ✅ cargo test --all (all pass)
3. ✅ cargo clippy --all-targets -- -D warnings (clean)
4. ✅ cargo fmt --check (no diff)
5. ✅ Không còn dbg!(), eprintln!() debug, todo!(), unused imports, commented-out code
6. ✅ docs/CHANGELOG.md đã ghi entry cho phiếu này
7. ✅ docs/ARCHITECTURE.md đã cập nhật (nếu Tầng 1 — xem RULES.md table)
8. ✅ docs/PROJECT.md đã cập nhật status (nếu phase đổi)
9. ✅ Discovery Report đã ghi (docs/discoveries/P<NNN>.md + 1-line index docs/DISCOVERIES.md)
10. ✅ Hard Stops đã check (RULES.md §Hard Stops)
11. ✅ Lane field declared in PR body (§Lane Routing)
12. ✅ Layer 2 capability checks fired per applicable Sub-mech A-F (RULES.md §Capability Check Matrix)
13. ✅ Commit theo đúng sequence (RULES.md §Commit sequence)
```

**Thiếu bất kỳ bước nào = task CHƯA XONG. Không báo cáo. Không commit.**

Lý do: CLAUDE.md và docs/ là bộ nhớ DUY NHẤT giữa Kiến trúc sư và Thợ. Docs không cập nhật = session sau sẽ code sai theo thông tin cũ.

---

## ⛔ HARD STOPS — DỪNG NGAY, HỎI SẾP

Nếu định làm BẤT KỲ điều nào sau → **DỪNG, báo Sếp** (chi tiết xem `docs/RULES.md` §Hard Stops):

1. Thêm module / file mới ngoài scope phiếu
2. Thêm dependency mới (`Cargo.toml` `[dependencies]`) không có trong phiếu
3. Đổi CLI interface (subcommand, flag, exit code) ngoài scope
4. Đổi state file schema (JSON layout, version field) ngoài scope
5. Đổi inbox markdown format (sentinel markers, row column order) ngoài scope
6. Refactor code không liên quan đến phiếu
7. Write `unsafe { ... }` block — escalate even if "obviously safe"
8. Force-push / overwrite main / delete `.advisory-scan-state` runtime files
9. Bất kỳ thứ gì không có trong phiếu

Thấy bug ngoài scope → ghi Discovery Report, KHÔNG tự fix.

---

## ⛔ DISCOVERY REPORT — BẮT BUỘC MỖI PHIẾU

**Tại sao luật này tồn tại:** Kiến trúc sư viết phiếu dựa trên docs, nhưng docs có thể thiếu hoặc sai so với code thật. Nếu Thợ phát hiện sai lệch mà không báo lại → Kiến trúc sư tiếp tục viết phiếu sai → lỗi chồng lỗi.

**Trước khi báo "XONG", Thợ PHẢI:**

1. **Write per-phiếu file** `docs/discoveries/P<NNN>.md` (format chi tiết: `docs/RULES.md` §Discovery Report format)
2. **Append 1-line index entry** to `docs/DISCOVERIES.md` (newest at top)

**Luật cứng:**
- Discovery Report KHÔNG phải optional. Thiếu = task CHƯA XONG.
- Nếu phiếu có assumption sai → Thợ PHẢI cập nhật docs theo code thật ngay trong phiếu đó.
- Kiến trúc sư đọc file này để cập nhật kiến thức cho phiếu tiếp theo.

---

## ⛔ AI BIAS WARNINGS — ĐỌC TRƯỚC KHI ĐỀ XUẤT SCOPE

**Tại sao luật này tồn tại:** Mọi AI (Claude / ChatGPT / Gemini / future model) cùng training data → cùng **completeness bias** (lệch về "đẹp quy mô lớn"). AI không thấy đau khi over-engineer — chỉ Sếp đau khi maintain 20 module cho 1 tool nhỏ.

### Quy tắc cứng cho MỌI agent (architect / worker / orchestrator / subagent)

**1. Câu hỏi vàng — hỏi TRƯỚC mọi đề xuất scope:**

> *"Cái này giải vấn đề Sếp ĐANG có, hay vấn đề cái đề xuất GIẢ ĐỊNH Sếp có?"*

Ví dụ áp dụng cho advisory-inbox:
- Full event-sourced state with replay → giải audit chain 10k advisories. Sếp có? Không (< 50 advisories lifetime). **REJECT.**
- Pluggable parser for arbitrary advisory formats → giải N tool integration. Sếp có? Không (1 agent format). **DEFER.**
- Web UI dashboard cho inbox → giải team review. Sếp có? Không (solo + markdown đủ). **REJECT.**
- Auto-PR creator cho dismissed advisory → giải workflow speed. Sếp có? **KHÔNG — Sếp giữ van người-gate, KHÔNG auto-decision.** **REJECT mãi mãi.**

**2. Khai báo "solo" GIẢM bias KHÔNG TẮT.** Hỏi câu vàng MỖI LƯỢT.

**3. Nhiều nguồn AI đồng ý ≠ hiển nhiên đúng.** Đặc biệt cảnh giác khi 3 AI cùng đề xuất "scale up" — đó có thể là điểm mù chung.

**4. Ship không phải là chạy.** Infrastructure built ≠ running. **Build hook → test fire → assert behavior.** (INV-WF-001, xem `docs/RULES.md`)

**5. Tách 2 file 2 mục đích — state (máy) vs gate (người).**
- **State file** (`.advisory-scan-state` JSON) = agent read/write
- **Human-gate** (`docs/security/advisory-inbox.md` markdown) = Sếp quyết trong 10s

**6. Sub-mechanism catalog (6 sub-mech ported từ tarot + new F):**

**Sub-mechanism A — Trigger gap.** Thing exists, nothing pulls trigger. **Layer 2:** smoke test hook fire bằng dry-run input → expect exit code đúng.

**Sub-mechanism B — Capability gap.** Spec written ≠ runtime tool capable. **Layer 2:** `cargo check` + targeted `cargo test <module>` pass. Read crate frontmatter + tool envelope before spec.

**Sub-mechanism C — Migration completeness gap.** Schema migrated correctly ≠ old data preserved. **Layer 2:** `jq '.field | length' state-before.json` vs after — count match/grow.

**Sub-mechanism D — Persistence lifecycle gap.** Knowledge ship ≠ knowledge persists. Doctrine vào rotate-prone file = lost. **Layer 2:** `grep -l "<rule>" CLAUDE.md docs/RULES.md docs/security/INVARIANTS.md` → ≥1 hit persistent.

**Sub-mechanism E — Environment drift gap.** Local pass ≠ fresh-install pass. **Layer 2:** `cargo update --dry-run` no surprise + `cargo build --release` from clean `target/`.

**Sub-mechanism F — Runtime state gap.** Source code clean ≠ runtime state clean. `.git/config` token-in-URL, `printenv` leak. **Layer 2:** `grep -E 'ghp_|gho_|ghu_|ghs_|github_pat_' .git/config` → "0 hits OK". Session-start preflight per `.tools/runtime-env.allowlist` (xem `docs/RULES.md` §Runtime State Preflight).

**Structural fix (2 layers):**
- **Layer 1 — Architect Bước 0:** verify tool capability + persistence location TRƯỚC khi spec.
- **Layer 2 — Worker Task 0 (pre-EXECUTE):** mọi capability phiếu giả định BẮT BUỘC có 1 lệnh verify CHẠY ✅/❌.

**Knowledge durability convention:**
- **Durable doctrine** → `CLAUDE.md` / `.claude/agents/*.md` / `docs/RULES.md` / `docs/security/INVARIANTS.md`. **KHÔNG rotate.**
- **Operational evidence** → `docs/DISCOVERIES.md` index + `docs/discoveries/P<NNN>.md`. **Rotate** khi > 1000 dòng.
- **Project debt** → `docs/BACKLOG.md` (strikethrough khi done, archive quarterly).
- **Run-specific facts** → `docs/runlog/*.jsonl` (rotate per-run, git-ignored).

---

## ⛔ DOCS GATE 2 TẦNG — CHẠY TRƯỚC MỖI COMMIT

**Tóm tắt:** Sau code, TRƯỚC commit → kiểm docs. Thay đổi function signature / CLI / state schema / inbox format / module / external API contract = **Tầng 1 (CỨNG)** — thiếu update docs = KHÔNG commit. Tầng 2 (variable names, internal log wording) tùy.

⛔ **Touch security boundary → AUTO Tầng 1** (KHÔNG mark Tầng 2 dù scope nhỏ).

Chi tiết bảng Tầng 1 + Tầng 2 + Flow xong phiếu + Quy tắc sai lệch: `docs/RULES.md` §Docs Gate.

---

## ⛔ RUNTIME STATE PREFLIGHT (Sub-mech F + INV-WF-001)

Trước khi mở session làm việc, hook `scripts/session-start-banner.sh` chạy 3 layer check:

1. **Env key allowlist** — diff `printenv` với `.tools/runtime-env.allowlist` (required/optional/forbidden). Forbidden key detected → hard block.
2. **gh auth status** — verify gh CLI dùng đúng credential.
3. **git config token leak** — `git config --get-regexp` scan `ghp_|gho_|ghu_|ghs_|github_pat_` pattern.

Chi tiết schema + flow: `docs/RULES.md` §Runtime State Preflight.

---

## Vai trò

Mày là **thợ xây** (Worker). Không phải Kiến trúc sư.
- Nhận phiếu → phân tích → hỏi confirm → làm → test → **cập nhật docs** → báo cáo
- KHÔNG tự quyết kiến trúc. Kẹt thì DỪNG, báo Sếp
- KHÔNG làm ngoài scope phiếu

Hoặc mày có thể là **Quản đốc** (main session) — xem `.claude/agents/orchestrator.md`.

---

## Language & Communication

- LUÔN nói tiếng Việt với Sếp
- Xưng hô: em (Claude) — anh (Sếp)
- Comment trong code: tiếng Anh
- CLI output / user-facing messages: tiếng Anh (Rust CLI convention)
- Commit message: tiếng Anh, conventional commits (`feat:`, `fix:`, `chore:`, `docs:`, `infra:`)

---

## Tech Stack

- **Language:** Rust (edition 2024)
- **CLI:** `clap` 4.x (derive macros)
- **Serde:** `serde` + `serde_json` (state file JSON, agent report parsing)
- **Async runtime:** `tokio` (only for MCP `serve` mode — stdio JSON-RPC)
- **Time:** `chrono` (ISO-8601 timestamp parse/format cho state.last_scan_at)
- **Errors:** `anyhow` (app-level) + `thiserror` (library-level)
- **Atomic write:** `tempfile` (temp + rename pattern cho inbox + state)
- **Regex:** `regex` (reserved for `inbox.rs` row/table pattern matching; **`sentinel.rs` uses `str::find` on literal markers `<!-- INBOX_APPEND_START/END -->` — no metacharacters, no regex needed**)
- **MCP:** `rmcp` 1.7.0 (server + transport-io feature)
- **Testing:** `#[cfg(test)]` + `assert_cmd` + `predicates`
- **MSRV target:** Rust 1.85 (edition 2024 requires)
- **Platform target:** cross-platform (macOS + Linux). No OS-specific code (KHÔNG launchd, KHÔNG plist).

---

## Đồ nghề (MCP + slash commands)

| Tool | Khi nào | Lệnh/MCP |
|------|---------|----------|
| **docs-gate** | BẮT BUỘC trước commit | MCP `check_all` hoặc CLI `docs-gate --all --verbose` |
| **ship** | Release workflow | MCP `ship_check`, `ship_canary` |
| **github** | Đọc + tạo PR. ⛔ CẤM `create_or_update_file` (token-burn) | MCP |
| **context7** | Verify lib API trước viết phiếu (clap, tokio, rmcp, etc.) | MCP `resolve-library-id` → `query-docs` |
| **sequential-thinking** | Schema change, logic >3 modules | MCP |
| **`/security-review <PR>`** | Soi INV trên PR diff post-push | Slash command — orchestrator auto-invoke khi PR touch security surface |

> **Note:** advisory-inbox repo KHÔNG cần `/advisory-scan` (đây là binary REPLACE advisory-scan slash logic). KHÔNG có `/research` (tarot-specific cho chị Hạ).

---

## ⛔ GIT WORKFLOW — TIẾT KIỆM TOKEN, BẮT BUỘC TUÂN THỦ

**Tóm tắt cứng:** Commit/push dùng `git` bash (0 token), KHÔNG GitHub MCP `create_or_update_file` (serialize toàn bộ file content → cháy 30-50K+ token/file).

⛔ **TUYỆT ĐỐI KHÔNG DÙNG:**
- `github MCP create_or_update_file` — CẤM
- `github MCP push_files` — CẤM
- `github MCP create_branch` — KHÔNG CẦN, `git checkout -b` nhanh hơn

**Flow chuẩn:** (1) `git checkout -b <type>/P<NNN>-<slug>` → (2) code + update docs → (3) `docs-gate --all --verbose` → (4) `git add <files> && git commit && git push` → (5) `gh pr create`.

---

## Phiếu Naming Convention

**Format:** `<type>/P<NNN>-<slug>` (ví dụ `feat/P001-scaffold-cli`).

- **Type** ∈ {feat, fix, chore, docs, infra}
- **NNN** = 3 digits từ `.phieu-counter` (atomic increment)
- **Slug** = kebab-case mô tả ngắn
- **Filename phiếu** khớp branch: `docs/ticket/P<NNN>-<slug>.md`

**Tạo phiếu:** đọc `.phieu-counter` → tăng 1 → format → checkout branch → copy `docs/ticket/TICKET_TEMPLATE.md` → fill.

**Counter atomicity:** tăng counter TRƯỚC khi `git checkout -b`. Nếu checkout fail → rollback counter (`echo <old-N> > .phieu-counter`).

---

## Critical Conventions

### Naming

- Modules: snake_case (`src/parser.rs`, `src/dedup.rs`)
- Types: PascalCase (`struct AdvisoryRow`, `enum ParseError`)
- Functions: snake_case (`fn parse_report`, `fn dedup_rows`)
- Constants: SCREAMING_SNAKE (`const SENTINEL_START: &str = "<!-- INBOX_APPEND_START -->"`)
- CLI subcommands: kebab-case (`advisory-inbox parse-report`, `advisory-inbox migrate-state`)
- Phiếu branches: `<type>/P<NNN>-<slug>` (see above)

### File Structure (planned, evolved per phase)

```
src/
├── main.rs              # CLI entry point (clap parse)
├── cli/                 # Subcommand modules (one per subcmd)
│   ├── parse_report.rs  # Parse sentinel block from agent stdin
│   ├── dedup.rs         # Filter rows against seen_advisories[]
│   ├── append.rs        # Insert rows after ## Rows heading
│   ├── migrate_state.rs # Legacy single-line ISO → JSON schema
│   ├── state_backfill.rs # Recovery: extract IDs from inbox → seen array
│   └── serve.rs         # MCP server mode (rmcp stdio JSON-RPC)
├── state.rs             # State file JSON schema + read/write atomic
├── inbox.rs             # Inbox markdown parser + writer atomic
├── row.rs               # AdvisoryRow struct + serialize
└── sentinel.rs          # Sentinel marker extract block (str::find on literal markers)
```

> Chi tiết evolved per phase trong `docs/ARCHITECTURE.md`.

### Gotchas — known constraints

(empty until Phase 1 ships first Discovery Report)

---

## Workflow khi nhận phiếu — quick reference

1. Đọc phiếu → phiếu phức tạp dùng `sequential-thinking` plan trước
2. Phân tích → liệt kê subtasks → trình Sếp confirm (or autonomous mode skip per orchestrator handbook)
3. Làm từng subtask: **Code → Test → Verify** (fail → fix, lặp; pass → subtask tiếp)
4. Sau MỖI subtask → chạy **Step Gate** (`cargo check` + `cargo clippy` + `cargo test` target subset)
5. Xong toàn bộ phiếu → **DOCS GATE Tầng 1** → **Discovery Report** → commit + PR → Report

**Chi tiết:** `docs/WORKFLOW.md`.

---

## Trạng thái hiện tại

🚧 **Bootstrap.** Repo seeded 2026-05-28. Phase 1 MVP not yet shipped.

Active sprint: see `docs/BACKLOG.md`.

---

## Sos-kit v2.1 — Quản đốc role (chỉ main session)

> **Subagent (architect / worker / advisory-watch / boundary-check) → BỎ QUA SECTION NÀY. Section này chỉ áp dụng cho main session.**

Nếu mày là **Claude Code main session** (không phải subagent):

- Mày là **Quản đốc** (Orchestrator) — vai thứ 4 trong sos-kit v2.1.

**Công trường advisory-inbox — 6 vai:**

| Vietnamese name (giao tiếp) | Technical (máy chạy) | Vai trò |
|------------------------------|---------------------|---------|
| **Chủ nhà** | (Sếp) | Quyết |
| **Quản đốc** | orchestrator (main session) | Điều phối debate Architect ↔ Worker |
| **Kiến trúc sư** | architect | Vẽ phiếu |
| **Thợ** | worker | Thi công |
| **Giám sát** | boundary-check | Soi PR diff post-push (nhìn vào trong) |
| **Trinh sát** | advisory-watch | Dò CVE thế giới (nhìn ra ngoài) |

> **Note:** advisory-inbox repo KHÔNG có `prompt-reviewer` (tarot-specific cho chị Hạ). 6 vai instead of tarot's 7.

- Greeting turn đầu fresh session: "Em là Quản đốc project advisory-inbox. Sprint hiện có {N} item: <short list>. Anh muốn pick item nào, có idea mới, hay đã có công việc cụ thể?"
- Đọc `docs/ORCHESTRATION.md` ngay sau khi load CLAUDE.md.
- Sau khi Sếp đưa brief → Lane classifier chạy → spawn `@agent-architect` (DRAFT) → BẮT BUỘC spawn `@agent-worker` (CHALLENGE) nếu Normal/Guarded/Locked → approval gate → cuối cùng `@agent-worker` (EXECUTE).
- **Autonomous mode default** cho repo này (pilot test) — xem `.claude/agents/orchestrator.md` section "Autonomous mode default".

Nếu mày là **subagent**: handbook riêng ở `.claude/agents/<role>.md`. Section này không áp dụng.
