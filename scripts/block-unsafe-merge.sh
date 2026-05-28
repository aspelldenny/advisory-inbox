#!/usr/bin/env bash
# PreToolUse hook — block `gh pr merge <N>` nếu PR touch security surface chưa có /security-review APPROVE comment.
#
# Đầu vào: Claude Code hook spec gửi JSON qua stdin với { "tool_input": { "command": "..." } }.
# Fallback: $CLAUDE_TOOL_INPUT env var nếu stdin trống.
# Exit 2 → block tool call (stderr message hiện UI). Exit 0 → allow.
#
# Pattern cứng Layer 2 Sub-mechanism A — em (Quản đốc) ép check, không dựa LLM remember.
# Reference: P230 scripts/block-env-edit.sh pattern + /security-review slash command.
#
# Override marker: command chứa `[security-review-skip:<reason>]` → allow với log warning.
#   Use case: doctrine/docs-only PR mà pattern match false-positive, Sếp đã review tay.
#
# Known limitation (Turn 2 architect respond): chỉ catch numbered form `gh pr merge <N>`.
# Branch-only form `gh pr merge --merge` (no number) BYPASS hook — defer P298 nếu thấy
# fire miss thực tế. Current sprint dùng numbered form predominantly + override marker
# available cho edge case. Reference: P297 Debate Log Turn 1 O1.2 + Turn 2 Quản đốc decide B.

set -euo pipefail

# Đọc input
if [ ! -t 0 ]; then
  INPUT=$(cat || echo "")
else
  INPUT="${CLAUDE_TOOL_INPUT:-}"
fi

# Không có input → pass through
if [ -z "$INPUT" ]; then exit 0; fi

# Parse command từ JSON
COMMAND=$(echo "$INPUT" | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    print(data.get('tool_input', {}).get('command', ''))
except Exception:
    print('')
" 2>/dev/null || echo "")

# Không có command → tool khác → pass
if [ -z "$COMMAND" ]; then exit 0; fi

# Match `gh pr merge <N>` (allow flag variants: --squash, --merge, --delete-branch, etc.)
# Known limitation: branch-only form `gh pr merge --merge` (no number) bypass hook — defer P298.
if ! echo "$COMMAND" | grep -qE 'gh pr merge[[:space:]]+[0-9]+'; then
  exit 0
fi

# Extract PR number (first numeric after `gh pr merge`)
PR=$(echo "$COMMAND" | sed -nE 's/.*gh pr merge[[:space:]]+([0-9]+).*/\1/p' | head -1)
if [ -z "$PR" ]; then exit 0; fi

# Override marker check
if echo "$COMMAND" | grep -qE '\[security-review-skip:[^]]+\]'; then
  REASON=$(echo "$COMMAND" | sed -nE 's/.*\[security-review-skip:([^]]+)\].*/\1/p')
  echo "⚠️  Security review override marker detected for PR #$PR. Reason: $REASON" >&2
  echo "    Allowing merge. Sếp đã review tay — em (hook) không block." >&2
  exit 0
fi

# Check security surface — Rust project pattern (advisory-inbox).
# SSOT: .claude/commands/security-review.md Step 3 path triggers (em PHẢI keep sync).
SECURITY_SURFACE_PATTERN='src/|Cargo\.toml|Cargo\.lock|\.mcp\.json|\.claude/settings|\.claude/agents/|\.tools/runtime-env|docs/security/|scripts/(block-|architect-guard|session-start)|\.github/workflows/'

DIFF_FILES=$(gh pr diff "$PR" --name-only 2>/dev/null || echo "")
if [ -z "$DIFF_FILES" ]; then
  # gh CLI fail (network/auth) → fail-safe: BLOCK với fallback message (KHÔNG silent allow)
  cat >&2 <<EOF
⛔ BLOCKED: gh pr diff #$PR thất bại (network/auth?).

Em (hook) KHÔNG verify được PR có touch security surface không.
Fail-safe: block merge để Sếp/Quản đốc kiểm tra tay.

Cách hợp lệ:
  - Kiểm tra gh auth status
  - Chạy: gh pr diff $PR --name-only
  - Nếu confirm KHÔNG touch security surface → re-run merge với marker:
      gh pr merge $PR --merge [security-review-skip:gh-cli-unavailable]
EOF
  exit 2
fi

# Check pattern match
if ! echo "$DIFF_FILES" | grep -qE "$SECURITY_SURFACE_PATTERN"; then
  # Also check .env.example skip (reuse logic từ security-review.md:48-54)
  NON_EXAMPLE=$(echo "$DIFF_FILES" | grep -E "^\.env" | grep -v '\.env\.example' || true)
  if [ -z "$NON_EXAMPLE" ]; then
    # PR không touch security surface → allow merge
    exit 0
  fi
fi

# PR touch security surface — check security-review comment APPROVE chưa
COMMENTS=$(gh pr view "$PR" --json comments --jq '.comments[].body' 2>/dev/null || echo "")
if echo "$COMMENTS" | grep -q '<!-- SECURITY_REVIEW_START -->'; then
  # Có review block. Check verdict.
  VERDICT_LINE=$(echo "$COMMENTS" | grep -A 50 '<!-- SECURITY_REVIEW_START -->' | grep -E '^Verdict:' | head -1)
  if echo "$VERDICT_LINE" | grep -q 'APPROVE'; then
    # APPROVE → allow
    exit 0
  fi
  # NEEDS_REVIEW or unknown → block
  cat >&2 <<EOF
⛔ BLOCKED: PR #$PR touch security surface VÀ /security-review verdict KHÔNG phải APPROVE.

Verdict line: $VERDICT_LINE

Hành động:
  1. Sếp đọc comment giám sát trên PR #$PR
  2. Nếu Sếp accept risk → re-run với marker:
     gh pr merge $PR --merge [security-review-skip:sep-accepted-needs-review]
  3. Nếu cần fix → spawn Worker EXECUTE fix theo INV flagged, push, gate sẽ re-fire
EOF
  exit 2
fi

# Touch security surface NHƯNG chưa có review → BLOCK
PR_URL="https://github.com/aspelldenny/advisory-inbox/pull/$PR"
cat >&2 <<EOF
⛔ BLOCKED: PR #$PR touch security surface NHƯNG chưa có /security-review.

Em (Quản đốc) suýt MISS triệu giám sát. Hook chặn để fix structural — KHÔNG dựa LLM remember.

Hành động:
  1. Chạy slash command (em tự gõ):
     /security-review $PR
  2. Đợi @agent-boundary-check verdict (advisory, post comment trên PR)
  3. Verdict APPROVE → re-run merge bình thường (hook sẽ allow)
  4. Verdict NEEDS_REVIEW → Sếp đọc comment + quyết (re-run với marker nếu accept)

Reference:
  - PR: $PR_URL
  - Rule: docs/ORCHESTRATION.md rule 9 (auto + manual merge security pre-check)
  - Slash: .claude/commands/security-review.md
  - Sub-mechanism A precedent: P281 + P285 + P297 (instance #9)
EOF
exit 2
