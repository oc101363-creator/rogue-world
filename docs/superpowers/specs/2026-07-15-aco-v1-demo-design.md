# Agent Civilization OS — V1 Demo Design

**Date:** 2026-07-15  
**Status:** Approved for implementation planning  
**Author:** 陈铭凯 / Claude  

---

## 1. Purpose

Build a **minimal, architecturally stable demo** of Agent Civilization OS (ACO):

> An Agent-native digital civilization simulation. Humans design/select agents and goals; agents decide and act in a shared world. UI is an observation layer, not the authority.

This document defines the V1 demo only. It is not the full product roadmap.

### 1.1 V1 success criteria (must demo)

1. **Agent can connect** — independent Agent Runtime speaks a Pi-style JSON protocol to the World.
2. **Agent can act** — Runtime submits actions (`move`, `harvest`, …); World validates and applies them.
3. **Player can select one agent** — selection is sent to World; World includes focus + allowed APIs in the next observation for that agent.
4. **Map + agent rendering + movement** — ASCII/grid map shows walls, resources, agent; movement is visible after actions apply.

### 1.2 Explicit non-goals (V1)

- Skill marketplace / declarative skill engine (beyond a stub module)
- Organization graph editor / multi-role command
- Full four-layer memory (personal/org/civ/global) persistence model
- Multi-agent cooperation, combat, diplomacy, trade
- Fog of war, pathfinding beyond one-step moves
- PixiJS / RTS visualization
- Redis, Kafka, NATS, RabbitMQ, Postgres

These may appear as empty folders or type stubs only.

---

## 2. Product direction (locked)

| Decision | Choice |
|----------|--------|
| Not an RTS engine | No OpenRA / Phaser RTS / VOIDSTRIKE / Unity / Godot as core |
| Reference stack | Screeps (world↔API split) + AI Town (agent world) + light ECS |
| Authority | **World Server is authoritative** |
| Agent boundary | Agents never mutate world state directly |
| Core loop | Event → Agent → Action → State → Event |
| Stack | TypeScript World, React + rot.js frontend, Python Agent Runtime, WebSocket, SQLite |

---

## 3. Architecture

### 3.1 Processes

```text
┌─────────────────┐     WebSocket      ┌──────────────────┐
│  Frontend       │◄──────────────────►│  World Server    │
│  React + rot.js │  snapshot/select   │  TypeScript      │
└─────────────────┘                    │  map, tick, ECS  │
                                       └────────┬─────────┘
                                                │ WebSocket
                                                │ observation / actions
                                       ┌────────▼─────────┐
                                       │  Agent Runtime   │
                                       │  Python          │
                                       │  Pi-style JSON   │
                                       └──────────────────┘
```

| Process | Owns | Does not own |
|---------|------|--------------|
| **World Server** | Map, entities, tick, action validation/execution, event log, focus state, WS gateways | LLM decisions, pixel rendering |
| **Agent Runtime** | Decision loop (mock or LLM), protocol client | Authoritative state |
| **Frontend** | Display map/entities, click-to-select, side panel | Game rules, movement legality |

### 3.2 Repository layout

```text
AgentGame/
├── packages/
│   └── protocol/           # Shared contracts (TS types + JSON Schema)
├── apps/
│   ├── world/              # TypeScript World Server
│   ├── web/                # React + rot.js
│   └── agent-runtime/      # Python Agent Runtime
├── docs/
│   └── superpowers/specs/
└── README.md
```

World internal modules:

```text
apps/world/src/
  core/           # entity store, world facade
  map/            # grid generation & queries
  systems/        # movement, harvest, …
  actions/        # action parse + validate + apply
  events/         # event bus + types
  gateway/        # websocket: frontend + agent
  persist/        # sqlite event log / snapshots
  scheduler/      # tick loop
```

### 3.3 Design principles for upgradeability

1. **Single authority** — only World mutates state.
2. **Contract-first** — `packages/protocol` is the single source for message shapes.
3. **Systems are pure-ish** — `(state, action) → (state', events)`; easy to test without LLM.
4. **Gateways are thin** — WS adapters translate bytes ↔ protocol messages; no game rules inside.
5. **Stubs over skips** — `memory/`, `skills/`, `org/` may exist as no-op modules with interfaces, not silent absences that force renames later.

---

## 4. World model

### 4.1 Map

- Fixed grid, default **16×12**.
- Cell kinds: `wall` | `floor`.
- Resources are entities on floor cells, not a third cell type (keeps rendering simple: glyph from top entity).
- V1 visibility: **full map** in observation (fog deferred).

Example ASCII (authoring / debug):

```text
################
#..............#
#......M.......#
#..............#
#...A..........#
#..............#
################
```

### 4.2 Entity model (light ECS)

```ts
type EntityId = string;

interface Entity {
  id: EntityId;
  type: "agent" | "resource";
  components: ComponentMap;
}

// Components used in V1
interface Position { x: number; y: number }
interface Appearance { glyph: string }      // 'A', 'M', …
interface Inventory { ore: number }
interface ResourceNode { ore: number }      // remaining
interface AgentBrain { runtimeSessionId?: string }
```

No heavy ECS library in V1. A small `EntityStore` with get/query by component is enough; can swap later without changing Action/Event contracts.

### 4.3 Tick loop

Default rate: **2 TPS** (500ms), configurable.

```text
onTick:
  1. Emit TICK_STARTED
  2. Collect pending actions for this tick (from agent gateway)
  3. Sort by actor entityId (deterministic)
  4. For each action: validate → apply → append events
     - invalid → ACTION_REJECTED, skip
  5. If no action for an agent → treat as idle (optional IDLE event)
  6. Persist events (sqlite)
  7. Broadcast snapshot to frontends
  8. Push observation to connected agent runtime(s)
  9. Start decision deadline timer for next tick’s actions
```

**Timeout:** if Runtime does not answer within decision deadline (default 2000ms), World proceeds with idle for that agent. Tick never blocks unbounded on LLM latency.

**Determinism:** given the same seed (map + initial entities) and the same ordered action sequence, state snapshots match. LLM output is outside the deterministic core.

---

## 5. Protocol contracts

Version field on all session hellos: `protocolVersion: "1.0"`.

### 5.1 Actions (World input from agents)

| type | payload | rules |
|------|---------|-------|
| `move` | `{ dx: -1\|0\|1, dy: -1\|0\|1 }` with \|dx\|+\|dy\| = 1 | four-way; fail if wall/OOB/occupied by wall |
| `harvest` | `{}` | success if same cell has resource with ore > 0; transfer 1 ore |
| `idle` | `{}` | no state change |
| `say` | `{ text: string }` | emits event only; max length 200 |

Hard cap: **≤ 6 action types** in V1. Do not add trade/attack/build until V2+.

```json
{
  "type": "action_batch",
  "id": "req-optional",
  "agentId": "agent-1",
  "tick": 12,
  "actions": [
    { "type": "move", "payload": { "dx": 1, "dy": 0 } }
  ]
}
```

World accepts at most **one primary world-mutating action per agent per tick** in V1 (`move` or `harvest` or `idle`); `say` may accompany. Extra mutators in the same batch are rejected.

### 5.2 Events (internal + logged)

Examples:

- `TICK_STARTED` `{ tick }`
- `MOVED` `{ entityId, from, to }`
- `MOVE_FAILED` `{ entityId, reason }`
- `HARVESTED` `{ entityId, resourceId, amount, inventoryOre }`
- `RESOURCE_DEPLETED` `{ resourceId }`
- `ACTION_REJECTED` `{ entityId, action, reason }`
- `AGENT_FOCUSED` `{ agentId }`
- `AGENT_UNFOCUSED` `{ agentId }`
- `AGENT_SAID` `{ entityId, text }`

### 5.3 Observation (World → Agent)

```json
{
  "type": "observation",
  "protocolVersion": "1.0",
  "tick": 12,
  "self": {
    "id": "agent-1",
    "x": 3,
    "y": 4,
    "inventory": { "ore": 2 }
  },
  "visible": {
    "width": 16,
    "height": 12,
    "tiles": ["################", "#..............#", "..."],
    "entities": [
      { "id": "ore-1", "type": "resource", "x": 8, "y": 2, "ore": 10, "glyph": "M" }
    ]
  },
  "events": [
    { "type": "AGENT_FOCUSED", "payload": { "agentId": "agent-1" } }
  ],
  "allowed_actions": ["move", "harvest", "idle", "say"],
  "focused": true,
  "goal": null
}
```

`allowed_actions` is mandatory so clients and LLMs do not invent verbs.

### 5.4 Frontend ↔ World messages

**Server → Client**

- `snapshot` — full render state: tick, grid glyphs, entities, focusedAgentId, recent events, inventories
- `hello` / `error`

**Client → Server**

- `select_agent` `{ agentId }` — V1: only one focused agent; selecting another replaces focus
- `ping`

Frontend never sends `move`/`harvest` in V1 (player operates by selecting agent and letting Runtime act; manual takeover is post-V1).

### 5.5 Agent Runtime ↔ World (Pi-style semantics)

Transport: **WebSocket**, JSON objects (one message per WS frame). Semantics inspired by [Pi RPC](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md): typed messages, optional correlation `id`, explicit lifecycle — **not** embedding ACO inside the Pi coding-agent binary.

| Direction | type | purpose |
|-----------|------|---------|
| W→A | `hello` | session, agentId, protocolVersion, map size |
| W→A | `observation` | decision input for a tick |
| A→W | `action_batch` | decision output |
| A→W | `hello_ack` | runtime ready |
| either | `error` | recoverable protocol/app error |
| either | `ping` / `pong` | keepalive |

Runtime modes:

1. **`mock`** — rule policy: path one step toward nearest ore, harvest when adjacent/same cell; always works offline.
2. **`llm`** — required path when API key present: observation → prompt → model → parse JSON actions → validate locally against `allowed_actions` → send. On parse failure, fall back to `idle` and log.

Environment: `ACO_LLM_API_KEY`, `ACO_LLM_BASE_URL`, `ACO_LLM_MODEL`, `ACO_RUNTIME_MODE=mock|llm`.

---

## 6. Player selection flow

```text
User clicks agent glyph on map
  → Frontend sends select_agent { agentId }
  → World sets focusedAgentId
  → World emits AGENT_FOCUSED
  → Next observation for that agent has focused: true
  → Snapshot to all frontends updates selection highlight
```

V1 has a single agent entity; the flow still implements the real focus API so multi-agent does not rewrite the gateway later.

---

## 7. Persistence

| Data | Store |
|------|--------|
| Live world state | In-memory |
| Event log | SQLite table `events(id, tick, type, payload_json, created_at)` |
| Optional snapshot | SQLite `snapshots(tick, state_json)` every N ticks (default 10) |

No external cache/queue services.

---

## 8. Frontend UX (minimal)

- Full-window (or main panel) rot.js display of the grid
- Glyphs: `#` wall, `.` floor, `M` resource, `A` agent; focused agent highlighted (e.g. `@` or color)
- Side panel: tick, focused agent id, inventory, last ~20 events
- Click entity cell with agent → select
- Connection status indicator

No build menus, no command cards, no multi-select.

---

## 9. Configuration defaults

| Key | Default |
|-----|---------|
| Map size | 16×12 |
| Tick rate | 2 TPS |
| Agent decision timeout | 2000 ms |
| Initial ore nodes | 3 |
| Initial ore per node | 10 |
| World WS port | 8080 |
| Frontend dev port | 5173 |
| SQLite path | `data/world.db` |

---

## 10. Acceptance tests (demo checklist)

1. Start World + Web + Runtime with one command doc each (or compose script).
2. Browser shows map with walls, at least one `M`, one `A`.
3. With `mock` runtime, agent moves toward ore and harvests; inventory increases; ore decreases or depletes.
4. Click agent → side panel shows selection; World logs `AGENT_FOCUSED`; observation has `focused: true`.
5. Stop Runtime → World keeps ticking; agent idles; no crash.
6. SQLite `events` table contains recent ticks.
7. With LLM key + `ACO_RUNTIME_MODE=llm`, agent still produces valid actions (or idle on failure) without breaking World.

---

## 11. Testing strategy (engineering)

- **Unit:** action validators, movement/harvest systems, entity store (no WS).
- **Integration:** World tick with injected action_batch → assert state + events.
- **Protocol:** golden JSON fixtures in `packages/protocol/fixtures/`.
- **Manual:** demo checklist above.

Automated e2e browser tests are optional for V1.

---

## 12. Implementation phases (for planning skill)

Ordered for vertical slices:

1. `packages/protocol` — types + schemas  
2. `apps/world` — map, entities, tick, systems, in-memory only  
3. World WS gateways + sqlite log  
4. `apps/agent-runtime` — mock client  
5. `apps/web` — rot.js + select  
6. LLM mode in runtime  
7. README + demo script  

---

## 13. Open decisions closed by this spec

| Question | Resolution |
|----------|------------|
| Player control model | Select only; no manual move commands in V1 |
| Agent intelligence | Mock always available; LLM required as implementable mode |
| Protocol inspiration | Pi-style message types over WebSocket, not pi binary host |
| Fog of war | Off in V1 |
| Multi-agent | Data model allows N; demo ships with 1 |
| Monorepo tool | pnpm or npm workspaces (plan picks one) |

---

## 14. Future hooks (do not implement now)

- Skill: `Event → matcher → action template | policy prompt`
- Organization: agent graph + goal delegation
- Memory: personal/org/civ/global stores behind a `MemoryPort`
- Visualization: swap rot.js for Pixi without changing World protocol
- RTS view: another frontend client on the same snapshot stream
