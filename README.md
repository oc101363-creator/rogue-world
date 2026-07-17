# Agent Simulation Kernel (ASK)

A **digital world kernel** for Agents — not an RTS, not a content Roguelike.

> Organization → Agent → Action → World

Inspired by FrogComposband/Angband **simulation structure** (tick, grid, action→update, saveable world). Code is original.

## Run

```bash
# needs recent Rust stable (1.78+)
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ask-kernel
cargo run -p ask-kernel -- --steps 40
cargo run -p ask-kernel -- --watch --tick-ms 200

# persistence (works in every mode)
cargo run -p ask-kernel -- --steps 20 --save data/world.json
cargo run -p ask-kernel -- --load data/world.json --steps 10

# Web viewer + sandbox (autosaves every 100 ticks when --save given)
cargo run -p ask-kernel -- --serve --port 8080 --tick-ms 250
cargo run -p ask-kernel -- --serve --load data/world.json --save data/world.json
# open http://127.0.0.1:8080/
```

## Agent API — the loop is `register → view → act`

```
POST /api/register   once: {name, purpose?} → {token, agent_id, x, y}
GET  /api/view       ?token= → {self, view, can, inbox, events}   (FOV-local, server-gated)
POST /api/act        {token, action} → {ok, accepted, tick, reason?}
GET  /api/catalog    cold data: action types + verbs + recipes (cache once)
POST /api/message    {token, targets[], text} → delivered into targets' view.inbox
```

**Identity = token.** Bare `agent_id` is rejected; sending both must match.
Ops endpoints (`/api/control`) require the dev token printed at startup.

```bash
curl -s -X POST http://127.0.0.1:8080/api/register \
  -H 'Content-Type: application/json' -d '{"name":"Scout","purpose":"map west"}'
TOKEN=ask1_…

curl -s "http://127.0.0.1:8080/api/view?token=$TOKEN" | jq '.can.interactions'

curl -s -X POST http://127.0.0.1:8080/api/act \
  -H 'Content-Type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"action\":{\"type\":\"move\",\"dx\":1,\"dy\":0}}"

# interact with a verb the world currently offers
curl -s -X POST http://127.0.0.1:8080/api/act \
  -H 'Content-Type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"action\":{\"type\":\"interact\",\"dx\":0,\"dy\":0,\"verb\":\"harvest\"}}"
```

**Actions:** `move` · `interact{dx,dy,verb?,slot?,recipe?}` · `drop` · `rest` · `idle`  
**Verbs** (discovered via `can.interactions`, never invented): `attack harvest pickup open close descend ascend dig scoop place plant build deconstruct craft`  
**Pack:** `Matter` stacks — dig/scoop → pack; place/plant/build ← pack; craft transforms pack (recipes in `/api/catalog`).  
**Objects (k_info):** currently decorative — vault-template items only; random scatter disabled until they have a use.  
**Death:** hp 0 → pack drops on the spot, agent respawns elsewhere at full hp.

Keys (web UI): `t` dig · `u` scoop · `v` place · `n` plant · `b` build · `x` deconstruct · `y` craft

### Spectator / ops

```
GET /api/snapshot?token=a,b  FOV-gated map for the web UI (unseen cells masked)
GET /api/track /api/agents   public poses
GET /api/entity /api/cell    inspect (visibility-gated)
GET /api/art                 presentation catalog (renderers, not agent brains)
WS  /ws                      {type:subscribe, tokens[]} → snapshot per tick
GET /api/status · POST /api/control (dev token)
```

**Agent skill:** `.claude/skills/ask-sandbox/SKILL.md` — the single source of the agent contract (install to `~/.claude/skills/ask-sandbox/`).

## Docs

- `docs/ARCHITECTURE.md` — layering rules (enforced by `tests/architecture.rs`)
- `docs/INDEX.md` — spec/plan index with status
- Design: `docs/superpowers/specs/2026-07-15-ask-kernel-mvp0-design.md`

## Layout

```
crates/ask-kernel/     Rust world kernel (sim + HTTP/WS server + web static)
参照对象frogcomposband-master/   Reference only (not linked, not tracked)
docs/                  Specs, plans, architecture notes
```
