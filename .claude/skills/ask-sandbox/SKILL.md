---
name: ask-sandbox
description: Use when controlling or scripting the ASK kernel sandbox (ask-kernel, AgentGame, dig/scoop/place/craft, Matter pack, /api/me, /api/action, FOV map, terrain editing, agent world-building)
---

# ASK Sandbox Agent Guide

## Overview

Tick-based sandbox. **Never mutate the grid yourself** — only submit actions; kernel returns options + events.

**Loop:** register → `GET /api/me?token=` → choose `interactions[]` → `POST /api/action` with **token** → events.

Base URL: `http://111.231.50.85:8000` (WS: `ws://111.231.50.85:8000/ws`).

## Identity (required for multi-agent)

On first run, **ask the human** for a short `name` and optional `purpose`, then register:

```bash
curl -s -X POST http://111.231.50.85:8000/api/register \
  -H 'Content-Type: application/json' \
  -d '{"name":"Scout","purpose":"map the west wing"}'
# → { ok, token: "ask1_…", agent_id, x, y }
```

- **Save the `token`.** It is the secret identity code (show once to the player for tracking).
- All later calls use `"token":"ask1_…"` on `/api/action` and `?token=` on `/api/me`.
- Unlimited agents may register; each gets a unique token + world spawn.
- Spectators paste token into the web **AGENT TRACK** panel to follow that agent.

## Contract

1. Only action types: `move` | `interact` | `drop` | `rest` | `idle`
2. Do **not** invent verbs — use current `interactions[].verb`
3. Do **not** invent recipes — use `interactions[].recipe` or `GET /api/actions` → `recipes`
4. One effective action per tick (last write wins)
5. Dig/scoop → pack; place/craft/plant/build ← pack
6. Prefer **token** over raw agent_id

## API

```bash
# register (skill onboarding)
curl -s -X POST http://111.231.50.85:8000/api/register \
  -H 'Content-Type: application/json' \
  -d '{"name":"Miner","purpose":"dig granite"}'

TOKEN=ask1_...   # from register

curl -s "http://111.231.50.85:8000/api/me?token=$TOKEN"
curl -s http://111.231.50.85:8000/api/actions
curl -s -X POST http://111.231.50.85:8000/api/action \
  -H 'Content-Type: application/json' \
  -d "{\"token\":\"$TOKEN\",\"action\":{\"type\":\"interact\",\"dx\":0,\"dy\":0,\"verb\":\"scoop\"}}"

# spectator
curl -s "http://111.231.50.85:8000/api/track?token=$TOKEN"
curl -s http://111.231.50.85:8000/api/agents
```

| endpoint | purpose |
|----------|---------|
| `POST /api/register` | create identity → token |
| `GET /api/me?token=` | pos, pack, interactions |
| `POST /api/action` | `{token, action}` |
| `GET /api/track?token=` | public pose for UI |
| `GET /api/agents` | list registered (no secrets) |
| `GET /api/snapshot` | identity map (`feat_ids`) + vision |
| `GET /api/actions` | action/recipe catalog |
| `GET /api/art` | presentation catalog (materials, feat looks) |
| `POST /api/message` | send custom prompt to visible agents |

## Messages (RTS selector)

Any spectator with a tracked token can select visible agents in the web UI and send them a custom prompt. Agents receive those prompts inside `/api/me` exactly once.

```bash
# send a prompt to one or more agents (targets are StableId values)
curl -s -X POST http://111.231.50.85:8000/api/message \
  -H 'Content-Type: application/json' \
  -d '{"token":"ask1_...","targets":[7,12],"text":"build a hut"}'

# agent runtime polls me and sees messages
curl -s 'http://111.231.50.85:8000/api/me?token=ask1_...' | jq '.messages'
```

Your agent client should inspect `.messages[]` and decide whether to obey based on its own passphrase or sender IP checks. The kernel only guarantees visibility: a sender cannot message an agent it cannot currently see.

## Actions

| type | body |
|------|------|
| `move` | `{dx,dy}` four-way unit only |
| `interact` | `{dx,dy, verb?, slot?, recipe?}` underfoot `(0,0)` or adjacent |
| `drop` | `{index}` drop pack slot underfoot |
| `rest` / `idle` | heal 1 HP / wait |

- `slot` — pack index for `place`
- `recipe` — id for `craft`

## Verbs

| verb | effect |
|------|--------|
| `dig` | hard rock → rubble/floor; feat → pack (+iron from treasure veins) |
| `scoop` | soft surface → pack (floor/dirt/grass/tree/water/lava/door…) |
| `place` | pack Terrain → cell; may return displaced surface |
| `harvest` | tree/iron entity → Resource |
| `plant` | wood or TREE block → TREE + harvestable entity |
| `build` / `deconstruct` | wood ↔ hut |
| `craft` | pack recipe transform |
| `pickup` / `open` / `close` / `descend` / `ascend` | items, doors, stairs |
| `attack` | melee (not sandbox focus) |

If `verb` omitted, kernel picks by priority (harvest/pickup before dig/scoop/craft).

## Pack (Matter)

```
Terrain{feat} | Resource{wood|iron} | Object{...}
me.pack[] = {slot, qty, label, matter}   # truth
me.wood / me.iron                        # sums only
```

Cannot place non-walkable under self. Permanent walls immutable.

## Craft recipes

Live list: `GET /api/actions`. Common ids:

`plank_door` (2 wood→door) · `sapling` (1 wood→TREE) · `compact_rock` (2 rubble→granite) · `crush_rock` · `ore_vein` · `mountain` (3 granite) · `smelt_iron` · `chop_wood` · `fill_floor` · `deep_pool` · `dirt_from_rubble` · …

```json
{"type":"interact","dx":0,"dy":0,"verb":"craft","recipe":"plank_door"}
```

## Goal recipes

**Corridor:** move until `dig` offered → dig → optional place elsewhere  
**Door:** harvest wood → craft `plank_door` → place  
**Grove:** harvest → plant on dirt/floor → harvest later  
**Reshape floor:** scoop surfaces → craft/place desired feat  

## Decision flow

```
interactions empty? → move toward goals / open space
else → pick interaction matching goal (copy dx,dy,verb,slot,recipe exactly)
     → POST /api/action
     → GET /api/me; check recent_events (dug|scooped|placed|crafted|action_rejected)
```

## Common mistakes

| wrong | right |
|-------|--------|
| `{"type":"dig"}` | `interact` + `verb:"dig"` |
| craft without `recipe` | pass recipe id |
| place with empty pack | dig/scoop/craft first |
| invent `mine`/`build_wall` | only five action types |
| spam same tick | wait next me tick |
| mock still moving you | `human_control: true` |

## Red flags

Inventing actions/verbs/recipes · ignoring `interactions[]` · writing grid via API · treating `items` strings as mutable state · combat-first when task is world-building

## Code map

`sandbox.rs` rules/recipes · `systems/interact.rs` · `systems/dig.rs` · `systems/craft.rs` · `components.rs` Matter · `serve.rs` HTTP
