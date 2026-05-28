# Advisory Inbox

> P008 state-backfill test fixture: 5 rows, 3 processed/dismissed, 2 open.

## Rows

| Date | Advisory ID | Source URL | Package | File:Line | Severity | Status | Note |
|------|-------------|-----------|---------|-----------|----------|--------|------|
| 2026-05-28 | CVE-2026-9001 | https://example.com/9001 | next@<15.5.17 | src/middleware.ts:42 | High | processed | reviewed |
| 2026-05-28 | CVE-2026-9002 | https://example.com/9002 | flask@<2.3.5 | app.py:8 | Medium | dismissed | not applicable |
| 2026-05-28 | CVE-2026-9003 | https://example.com/9003 | tokio@<1.40 | src/main.rs:1 | Critical | processed | patched |
| 2026-05-28 | CVE-2026-9004 | https://example.com/9004 | serde@<1.0.200 | src/lib.rs:5 | Low | open | pending review |
| 2026-05-28 | CVE-2026-9005 | https://example.com/9005 | clap@<4.5 | src/main.rs:10 | Medium | open | pending review |
