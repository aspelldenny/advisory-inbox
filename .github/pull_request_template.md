<!--
PR template for advisory-inbox — enforces Workflow v2.1 §13 Lane override audit.

REQUIRED sections:
- Summary
- Lane (Fast/Normal/Guarded/Locked — declared, with override if classifier disagrees)
- Test plan
- Discovery Report link (per RULES.md §13)
-->

## Summary

<!-- 1-3 bullets describing what changes + why. Reference phiếu number. -->

- Phiếu: P<NNN>
- Change: <brief>
- Why: <reason>

---

## Lane

- declared: <fast | normal | guarded | locked>
- reason: <surface touched / scope match RULES.md §1>

### Lane override

<!-- If classifier output differs from declared lane, fill in.
If no override, keep N/A. -->

- original: N/A
- requested: N/A
- reason: N/A (no override)
- approved_by: N/A

---

## Test plan

<!-- Bulleted checklist for testing. -->

- [ ] `cargo build --release` zero warnings
- [ ] `cargo test --all` all pass
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo fmt --check` no diff
- [ ] (if subcmd added/changed) Manual smoke test: `advisory-inbox <subcmd> < fixture` → expected output
- [ ] (if MCP changed) JSON-RPC handshake test: `echo '...' | advisory-inbox serve` → valid response

---

## Discovery Report

- File: `docs/discoveries/P<NNN>.md`
- Layer 2 checks fired: <list Sub-mech A-F>

---

## Sub-mechanism applicability

<!-- Mark which Sub-mech apply for this phiếu. -->

- [ ] A — Trigger gap (hook/cron/MCP/slash command added)
- [ ] B — Capability gap (new crate dep / tool dependency)
- [ ] C — Migration completeness (state schema change / data format change)
- [ ] D — Persistence lifecycle (doctrine update — needs home: declared in commit)
- [ ] E — Environment drift (`cargo update`-sensitive change)
- [ ] F — Runtime state gap (env var read / token handling / git config touched)

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)
