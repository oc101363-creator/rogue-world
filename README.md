# Agent Civilization OS (ACO)

Agent-native civilization simulation. **World is authoritative**; agents connect over WebSocket, receive observations, submit actions. The frontend observes the map and selects agents — it does not run game rules.

```text
Player → select agent → World → observation → Agent Runtime → actions → World → map
```

## V1 Demo features

| Feature | Status |
|---------|--------|
| Grid map (16×12 ASCII / rot.js) | ✅ |
| **HTTP API + `aco` CLI** (primary agent surface) | ✅ |
| **`aco-play` Skill** — give any agent the skill to play | ✅ |
| Agent act (`move`, `harvest`, `idle`, `say`) | ✅ |
| Player select agent → focus | ✅ |
| Optional WS agent runtime (mock / LLM) | ✅ |
| SQLite event log | ✅ |

### Primary integration path

```text
Player's Agent  +  skills/aco-play  +  aco CLI  →  World HTTP API
```

You do **not** need a custom runtime to plug an agent in: install/copy `skills/aco-play`, point the agent at a running World, and let it `observe` / `act`.

## Spec & plan

- Design: [`docs/superpowers/specs/2026-07-15-aco-v1-demo-design.md`](docs/superpowers/specs/2026-07-15-aco-v1-demo-design.md)
- Plan: [`docs/superpowers/plans/2026-07-15-aco-v1-demo.md`](docs/superpowers/plans/2026-07-15-aco-v1-demo.md)

## Prerequisites

- Node 22+, pnpm 9+
- Python 3.11+

## Setup

```bash
pnpm install
pnpm --filter @aco/protocol build
python3 -m pip install -r apps/agent-runtime/requirements.txt
```

## Run

```bash
# 1) World Server (port 8080) — required
pnpm dev:world

# 2) Frontend (optional observer)
pnpm dev:web
# → http://localhost:5173

# 3) Control via CLI (any agent / human / script)
pnpm aco status
pnpm aco map
pnpm aco observe
pnpm aco act move --dx 1 --dy 0
pnpm aco act harvest
pnpm aco focus agent-1
```

### Give a player agent the skill

Copy or link [`skills/aco-play`](skills/aco-play) into the agent's skill directory.  
The skill teaches the observe → decide → act loop over `aco`.

### Optional: built-in mock / LLM runtime (WebSocket)

Still available if you want a process that plays without an external agent:

```bash
cd apps/agent-runtime && python3 -m aco_runtime --mode mock
# or
export ACO_LLM_API_KEY=...
python3 -m aco_runtime --mode llm
```

## What you should see

1. Browser shows walls `#`, floors `.`, mines `M`, agent `A`.
2. Mock agent walks toward the nearest ore and harvests; inventory increases.
3. Click the agent → glyph becomes `@` / side panel shows focus; World sets `focusedAgentId`.
4. Kill the agent process → World keeps ticking (agent idles), no crash.
5. Events land in SQLite (`apps/data/world.db` by default).

## Repo layout

```text
packages/protocol/     Shared TS contracts
apps/world/            World Server (tick, systems, HTTP API, WS, SQLite)
apps/cli/              aco CLI → HTTP API
apps/web/              React + rot.js observer
apps/agent-runtime/    Optional Python runtime (mock | llm over WS)
skills/aco-play/       Skill for external agents
docs/superpowers/      Design + plan
```

## Surfaces

| Surface | URL / command |
|---------|----------------|
| HTTP API | `http://127.0.0.1:8080/api/status` … |
| CLI | `pnpm aco <cmd>` |
| Frontend WS | `ws://127.0.0.1:8080/ws/frontend` |
| Agent WS (optional) | `ws://127.0.0.1:8080/ws/agent?agentId=agent-1` |

## Tests

```bash
pnpm --filter @aco/world test
cd apps/agent-runtime && python3 -m unittest discover -s tests -v
```

## Non-goals (V1)

Skill market, organization editor, multi-agent diplomacy/combat, fog of war, Pixi/RTS view, Redis/Postgres.
