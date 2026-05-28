# CHANGELOG — advisory-inbox

> Soft cap < 1000 dòng. Rotate batch cũ → `docs/Archive/CHANGELOG_ARCHIVE.md` khi vượt.

---

## 2026-05-28 — Bootstrap (P000)

- Initial repo seed via Workflow v2.1 pilot setup
- Cargo crate scaffolded (edition 2024, deps: clap/serde/tokio/chrono/rmcp/tempfile/regex/anyhow/thiserror)
- Workflow v2.1 doctrine ported from `~/sos-kit/docs/WORKFLOW_V2.1.md`
- Skeleton copied from `~/advisory-cron` (agents, scripts, ticket template, INVARIANTS)
- `docs/RULES.md` written với 17 sections covering all 13 v2.1 items
- `docs/PROJECT.md` vision + scope cứng
- `docs/ARCHITECTURE.md` 6 subcmd + state schema + inbox format + MCP surface
- `docs/BACKLOG.md` P001..P013 phiếu queued across 4 phase
- `.tools/runtime-env.allowlist` 3-group schema (required/optional/forbidden)
- `.github/pull_request_template.md` với Lane override section (v2.1 §13)

home: docs/RULES.md (durable doctrine port)
