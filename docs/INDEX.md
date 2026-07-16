# Docs Index

Living docs first, then dated specs/plans with status. Update this file when
a doc lands or changes state.

## Living

- `ARCHITECTURE.md` — layering rules + house rules (enforced by `tests/architecture.rs`)
- `../.claude/skills/ask-sandbox/SKILL.md` — the agent contract (single source)

## Specs (superpowers/specs)

| doc | status |
|-----|--------|
| `2026-07-15-ask-kernel-mvp0-design.md` | implemented (MVP-0) |
| `2026-07-16-sandbox-matter-pack-design.md` | implemented |
| `2026-07-16-fs-hdg-art-decouple-design.md` | implemented |
| `2026-07-16-ask-api-surface.md` | **superseded** — legacy aliases removed 2026-07-16 (overhaul); see `../README.md` + `ARCHITECTURE.md` |

## Plans (superpowers/plans)

| doc | status |
|-----|--------|
| `2026-07-15-ask-kernel-mvp0.md` | done |
| `2026-07-16-fs-hdg-art-decouple.md` | done |
| `2026-07-16-rts-selector-messaging.md` | done |

## 2026-07-16 overhaul (worktree `ask-overhaul`)

Security (FOV-masked snapshots, token-only act, dev-gated control),
multi-agent correctness (level change preserves all agents, nearest-agent
monsters, death/respawn), save v2 + serve persistence, agent-view direct
projection, verb registry + describe + balance + spatial, serve split,
architecture guard tests. See git log on `worktree-ask-overhaul`.
