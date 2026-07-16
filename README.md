# Agent Simulation Kernel (ASK)

A **digital world kernel** for Agents — not an RTS, not a content Roguelike.

> Organization → Agent → Action → World

Inspired by FrogComposband/Angband **simulation structure** (tick, grid, action→update, saveable world). Code is original.

## MVP-0

```bash
# needs recent Rust stable (1.78+)
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ask-kernel
cargo run -p ask-kernel -- --steps 40
cargo run -p ask-kernel -- --watch --tick-ms 200
cargo run -p ask-kernel -- --steps 20 --save data/world.json
cargo run -p ask-kernel -- --load data/world.json --steps 10

# Web viewer + sandbox
cargo run -p ask-kernel -- --serve --port 8080 --tick-ms 250
# open http://127.0.0.1:8080/
```

## Sandbox API — two primitives

```
GET  /api/snapshot   → world + FOV + entities + interactions[] + events
POST /api/action     → { action: move | interact | drop | rest | idle }
GET  /api/me         → focused agent + interactions (from same snapshot)
GET  /api/actions    → action catalog
```

```bash
# read options the world currently offers
curl -s http://127.0.0.1:8080/api/me | jq '.interactions'

# move
curl -s -X POST http://127.0.0.1:8080/api/action \
  -H 'Content-Type: application/json' \
  -d '{"action":{"type":"move","dx":1,"dy":0}}'

# interact with a target (verb from interactions[])
curl -s -X POST http://127.0.0.1:8080/api/action \
  -H 'Content-Type: application/json' \
  -d '{"action":{"type":"interact","dx":0,"dy":0,"verb":"harvest"}}'

# default interact (single option / priority pick)
curl -s -X POST http://127.0.0.1:8080/api/action \
  -H 'Content-Type: application/json' \
  -d '{"action":{"type":"interact","dx":0,"dy":0}}'
```

**Actions:** `move` · `interact{dx,dy,verb?,slot?,recipe?}` · `drop` · `rest` · `idle`  

**Sandbox verbs:** `dig` `scoop` `place` `harvest` `plant` `build` `deconstruct` `craft` + doors/stairs/pickup  

**Pack:** `Matter` stacks — dig/scoop → pack; place/plant/build ← pack; craft transforms pack (16 recipes).

```bash
curl -s http://127.0.0.1:8080/api/me          # pack + interactions
curl -s http://127.0.0.1:8080/api/actions     # verbs + recipes
# scoop floor, craft door, dig wall, place block…
curl -s -X POST http://127.0.0.1:8080/api/action -H 'Content-Type: application/json' \
  -d '{"action":{"type":"interact","dx":0,"dy":0,"verb":"scoop"}}'
curl -s -X POST http://127.0.0.1:8080/api/action -H 'Content-Type: application/json' \
  -d '{"action":{"type":"interact","dx":0,"dy":0,"verb":"craft","recipe":"plank_door"}}'
```

Keys: `t` dig · `u` scoop · `v` place · `n` plant · `b` build · `x` deconstruct · `y` craft  
Spec: `docs/superpowers/specs/2026-07-16-sandbox-matter-pack-design.md`  
**Agent skill:** `.claude/skills/ask-sandbox/SKILL.md` (also `~/.claude/skills/ask-sandbox/`) — register → token → dig/place/craft.

### Multi-agent identity

```bash
# skill asks name/purpose, then:
curl -s -X POST http://127.0.0.1:8080/api/register \
  -H 'Content-Type: application/json' \
  -d '{"name":"Scout","purpose":"map west"}'
# → { token: "ask1_…", agent_id, x, y }  # give token to player for tracking

curl -s -X POST http://127.0.0.1:8080/api/action \
  -H 'Content-Type: application/json' \
  -d '{"token":"ask1_…","action":{"type":"move","dx":1,"dy":0}}'

curl -s "http://127.0.0.1:8080/api/track?token=ask1_…"
```

Web UI: left **AGENT TRACK** panel — paste tokens to follow multiple agents on one map (Rogue-80 chrome).

## Docs

- Design: `docs/superpowers/specs/2026-07-15-ask-kernel-mvp0-design.md`
- Plan: `docs/superpowers/plans/2026-07-15-ask-kernel-mvp0.md`

## Layout

```
crates/ask-kernel/     Rust world kernel
frogcomposband-master/ Reference only (not linked)
docs/                  Specs and plans
```
