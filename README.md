# Agent Civilization OS (ACO)

Agent-native civilization simulation. **World is authoritative**; agents connect over WebSocket, receive observations, submit actions. The frontend observes the map and selects agents — it does not run game rules.

```text
Player → select agent → World → observation → Agent Runtime → actions → World → map
```

## V1 Demo features

| Feature | Status |
|---------|--------|
| Grid map (16×12 ASCII / rot.js) | ✅ |
| Agent connect (Pi-style JSON over WS) | ✅ |
| Agent act (`move`, `harvest`, `idle`, `say`) | ✅ |
| Player select agent → focus in observation | ✅ |
| Mock policy + optional LLM mode | ✅ |
| SQLite event log | ✅ |

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

## Run (three terminals)

```bash
# 1) World Server (port 8080)
pnpm dev:world

# 2) Agent Runtime (mock — no API key needed)
cd apps/agent-runtime
python3 -m aco_runtime --mode mock

# 3) Frontend
pnpm dev:web
# → http://localhost:5173
```

Or use the helper (World + mock agent; start web separately):

```bash
chmod +x scripts/dev.sh
./scripts/dev.sh
# another terminal:
pnpm dev:web
```

### LLM mode (optional)

```bash
export ACO_LLM_API_KEY=sk-...
export ACO_LLM_BASE_URL=https://api.openai.com/v1   # or compatible gateway
export ACO_LLM_MODEL=gpt-4o-mini
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
packages/protocol/     Shared TS contracts (Action / Observation / Snapshot)
apps/world/            TypeScript World Server (tick, systems, WS, SQLite)
apps/web/              React + rot.js observer UI
apps/agent-runtime/    Python Agent Runtime (mock | llm)
docs/superpowers/      Design + implementation plan
```

## Protocol (summary)

- Frontend: `ws://127.0.0.1:8080/ws/frontend` — `snapshot` / `select_agent`
- Agent: `ws://127.0.0.1:8080/ws/agent?agentId=agent-1` — `hello` → `observation` → `action_batch`

## Tests

```bash
pnpm --filter @aco/world test
cd apps/agent-runtime && python3 -m unittest discover -s tests -v
```

## Non-goals (V1)

Skill market, organization editor, multi-agent diplomacy/combat, fog of war, Pixi/RTS view, Redis/Postgres.
