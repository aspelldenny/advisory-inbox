#!/usr/bin/env bash
# PreToolUse hook — block Edit/Write tới .env* files (except .env.example).
#
# Đầu vào: Claude Code hook spec gửi JSON qua stdin với { "tool_input": { "file_path": "..." } }.
# Fallback: $CLAUDE_TOOL_INPUT env var nếu stdin trống.
# Exit 2 → block tool call. Exit 0 → allow.
#
# Pattern: cứng Sub-mechanism F (Runtime state gap) — ngăn runtime env state bị tạp.
# Reference: ~/sos-kit/docs/WORKFLOW_V2.1.md §12 Runtime state preflight + INV-WF-001.
#
# advisory-inbox specific: tools/runtime-env.allowlist là human-gate cho env keys.
# `.env*` files KHÔNG nên tồn tại trong repo này (advisory-inbox không read env runtime).
# Hook blocks defensive — protects against accidental .env creation từ template projects.

set -euo pipefail

# Đọc input
if [ ! -t 0 ]; then
  INPUT=$(cat || echo "")
else
  INPUT="${CLAUDE_TOOL_INPUT:-}"
fi

# Không có input → pass through
if [ -z "$INPUT" ]; then exit 0; fi

# Parse file_path từ JSON
FILE_PATH=$(echo "$INPUT" | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    print(data.get('tool_input', {}).get('file_path', ''))
except Exception:
    print('')
" 2>/dev/null || echo "")

# Không có file_path → tool khác → pass
if [ -z "$FILE_PATH" ]; then exit 0; fi

# Basename để check pattern
BASE=$(basename "$FILE_PATH")

# Allowlist: .env.example là template, được phép edit (nếu future project cần)
if [ "$BASE" = ".env.example" ]; then exit 0; fi

# Allowlist: .runtime-env.allowlist là declared schema, được phép edit (no values)
if [ "$BASE" = ".runtime-env.allowlist" ]; then exit 0; fi

# Block .env và .env.* (any variant — local, production, etc.)
if echo "$BASE" | grep -qE '^\.env($|\.)'; then
  cat >&2 <<EOF
⛔ BLOCKED: Edit/Write tới $FILE_PATH bị chặn.

Lý do: advisory-inbox là pure-local CLI tool — KHÔNG read env runtime ở MVP.
Tạo .env* file = surface chưa-định-nghĩa, vi phạm Sub-mech F (runtime state).

Cách hợp lệ:
  - Edit .tools/runtime-env.allowlist (declared schema, no values, committed)
  - Add INV-1 entry trong docs/security/INVARIANTS.md nếu phiếu cần env read
  - Document trong docs/ARCHITECTURE.md §Config schema
EOF
  exit 2
fi

# Mọi file khác → allow
exit 0
