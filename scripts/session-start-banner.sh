#!/usr/bin/env bash
# SessionStart banner — advisory-inbox (Rust project, sos-kit v2.1 pilot)
# Surfaces: active sprint from BACKLOG, runtime state preflight (Sub-mech F), open PRs.
# Quản đốc reads this at session start to decide opening narrative.

set -u

# 1. Active sprint from BACKLOG.md
if [ -f docs/BACKLOG.md ]; then
    # Find first H2 containing "Active sprint" (case-insensitive)
    ACTIVE=$(awk '
        /^## .*[Aa]ctive [Ss]print/ { capturing=1; print; next }
        capturing && /^## / { exit }
        capturing { print }
    ' docs/BACKLOG.md | head -40)
    if [ -n "$ACTIVE" ]; then
        echo "📋 Active sprint:"
        echo "$ACTIVE" | sed 's/^/  /'
        echo ""
    fi
fi

# 2. Advisory staleness (rule 10 — orchestrator auto-spawn trigger)
STATE_FILE="docs/security/.advisory-scan-state"
if [ ! -f "$STATE_FILE" ]; then
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🚨 Advisory-watch: chưa scan lần nào — gõ /advisory-scan để first scan"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
else
    LAST_SCAN=$(grep -oE '"last_scan_at"[[:space:]]*:[[:space:]]*"[^"]+"' "$STATE_FILE" 2>/dev/null \
                | sed -E 's/.*"last_scan_at"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/' \
                | head -1)
    if [ -z "$LAST_SCAN" ]; then
        LAST_SCAN=$(cat "$STATE_FILE" 2>/dev/null | tr -d '[:space:]')
    fi
    LAST_EPOCH=$(date -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_SCAN" +%s 2>/dev/null \
                 || date -d "$LAST_SCAN" +%s 2>/dev/null \
                 || echo 0)
    NOW_EPOCH=$(date +%s)
    if [ "$LAST_EPOCH" -gt 0 ]; then
        DAYS_SINCE=$(( (NOW_EPOCH - LAST_EPOCH) / 86400 ))
        if [ "$DAYS_SINCE" -ge 7 ]; then
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "🚨 Advisory-watch: lần scan cuối $DAYS_SINCE ngày trước (>= 7 ngày stale)"
            echo "    Orchestrator BẮT BUỘC auto-spawn /advisory-scan early-session"
            echo "    (per docs/ORCHESTRATION.md Hard rule 10)"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        elif [ "$DAYS_SINCE" -ge 3 ]; then
            echo "⚠️  Advisory-watch: lần scan cuối $DAYS_SINCE ngày trước — cân nhắc /advisory-scan"
        fi
    fi
fi

# 3. Open PRs (if gh CLI available + repo has remote)
if command -v gh >/dev/null 2>&1 && git remote get-url origin >/dev/null 2>&1; then
    OPEN_PRS=$(gh pr list --state open --json number,title,headRefName --limit 5 2>/dev/null)
    if [ -n "$OPEN_PRS" ] && [ "$OPEN_PRS" != "[]" ]; then
        echo "🔀 Open PRs:"
        echo "$OPEN_PRS" | jq -r '.[] | "  #\(.number) (\(.headRefName)): \(.title)"' 2>/dev/null
        echo ""
    fi
fi

# 4. Phiếu counter status (for context)
if [ -f .phieu-counter ]; then
    NEXT_N=$(cat .phieu-counter | tr -d '[:space:]')
    echo "🎫 Next phiếu ID: P$(printf '%03d' $((10#$NEXT_N + 1)))"
fi

# 5. Runtime state preflight (Sub-mech F per v2.1, INV-WF-001 trigger)
if [ -f .tools/runtime-env.allowlist ]; then
    # Extract forbidden keys (simple line-based parse, skip comments + blanks)
    FORBIDDEN=$(awk '
        /^forbidden:/ { capturing=1; next }
        capturing && /^[a-z]+:/ { exit }
        capturing && /^\s*-\s/ {
            sub(/^\s*-\s*/, "")
            sub(/\s.*$/, "")
            if ($0 != "" && $0 !~ /^#/) print
        }
    ' .tools/runtime-env.allowlist)

    BLOCKED_KEYS=""
    for key in $FORBIDDEN; do
        if printenv | grep -q "^${key}="; then
            BLOCKED_KEYS="$BLOCKED_KEYS $key"
        fi
    done

    if [ -n "$BLOCKED_KEYS" ]; then
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "🚫 BLOCK: forbidden runtime key(s) detected:$BLOCKED_KEYS"
        echo "    → .tools/runtime-env.allowlist marks these forbidden"
        echo "    → unset / use env -u trước khi tiếp tục"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    fi
fi

# 6. Git config token leak check (Sub-mech F, tarot P305 precedent INV-010)
if git config --get-regexp 'http.*extraheader|credential|insteadOf' 2>/dev/null \
    | grep -qE 'ghp_|gho_|ghu_|ghs_|github_pat_'; then
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🚫 BLOCK: GitHub token-in-config detected (Sub-mech F)"
    echo "    → git config --unset <key> hoặc switch sang gh CLI keychain auth"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
fi
