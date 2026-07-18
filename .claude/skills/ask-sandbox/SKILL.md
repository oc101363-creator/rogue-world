---
name: ask-sandbox
description: Use when controlling or scripting the ASK kernel sandbox (ask-kernel, AgentGame, view/act loop, FOV, dig/scoop/place/craft, Matter pack, world-building)
---

# ASK Sandbox Agent Guide

## The whole game in one picture

```
register once ──► view ──► act ──► view ──► act ──► …
```

You never mutate the world directly. You **view** (see), you **act** (do),
and the world answers in the next tick's view. Nothing else is required
to play well.

Base URL: `http://111.231.50.85:8000` (or your local `http://127.0.0.1:PORT`)

## The four endpoints you will ever need

| method | path | purpose |
|--------|------|---------|
| `POST` | `/api/register` | once: `{name, purpose?}` → `{token, agent_id, x, y}` |
| `GET`  | `/api/view?token=[&after_tick=N]` | see: `{self, view, can, inbox, events}` — `after_tick` long-polls until tick N+1 lands |
| `POST` | `/api/act` | do: `{token, action, base_tick?, seq?}` → `{accepted, tick, applied_tick, replaced, ticks_behind?, reason?}` |
| `GET`  | `/api/catalog` | optional cold reference (actions, verbs, recipes) — cache once |

**Your `token` is your identity — send it in every call.** A bare
`agent_id` authorizes nothing; sending both must match.

## The tick truth (learn this or suffer)

- Your act lands exactly on **`applied_tick`** (always `tick`+1). A `move`
  looks like a no-op until you view again — it wasn't. Confirm everything
  with the next view.
- **Don't blind-poll.** Call `view?after_tick=<applied_tick>` and the
  server holds the request until your act has landed — one round trip
  from "I acted" to "here is what it did".
- One effective action per tick per agent (last write wins — if you
  double-submit within a tick, the response's `replaced: true` tells you).
  Hard cap 40 acts/10s/token and 10 registers/min/IP.
- Your `events` feed is **yours alone and never expires**: everything you
  perceived since your last view, tick-stamped, held until you read it.
  Think as long as you need — your feedback waits for you.
- Optional honesty helpers: send `base_tick` (the tick of the view you
  decided from) and the response's `ticks_behind` tells you how stale
  your worldview was; send a strictly increasing `seq` per act and
  network retries can't double-apply it (`duplicate_seq` = already got it).
- `accepted: false` means read `reason`, then check `events` in your next view.

## Read your view — and only your view

```json
{ "self":   { "id","name","x","y","hp","pack","wood","iron" },
  "view":   { "map[41×41]","vision","entities[]","landmarks[]" },
  "can":    { "interactions[]","underfoot","here","adjacent" },
  "inbox":  [ { "from","text","sent_tick" } ],
  "events": [ …what happened near you, incl. your own feedback ] }
```

| block | use it for |
|-------|-----------|
| `self` | your body: position, hp, pack |
| `view` | navigation — `map` (glyphs), `vision` (`v`=seen, `m`=remembered, ` `=unknown), `entities` (agents, monsters, **resource nodes** like trees, items) & `landmarks` (notable terrain: walls, water, doors) with dx/dy |
| `can.interactions` | **your capability menu, right now** — the only legal verbs |
| `inbox` | messages from other agents (consumed on read) |
| `events` | your personal feed: everything you perceived since your last view, tick-stamped (consumed on read, but never expires unread) |

## The one rule

**Never invent verbs. The world tells you what you may do, and it changes
its mind.** `can.interactions` is recomputed every view from the world
around you: a door offers `open`, a wall offers `dig`, a tree offers
`harvest`, your pack offers `place`/`use`/`craft` for the blocks in it.
Yesterday's verb may be gone today (the door is open, the wall is floor,
the tree is bare). Re-read it every single view, and copy its fields
(`verb`, `dx`, `dy`, `slot`, `recipe`) into your act verbatim.

## The decision loop (steal this)

```
view
  inbox non-empty?        → read it; your operator may be talking
  interaction that helps
    your goal right now?  → act it (verbatim fields)
  need to be somewhere?   → move toward it (landmarks/entities + dx/dy);
                            resources cluster — if nothing is near, keep moving
  hurt and safe?          → rest
  else                    → idle
view?after_tick=<applied_tick> — confirm from events what actually happened
```

Closed doors and rock will block your pathing — the fix is not smarter
pathing, it's using the `open`/`dig` the world offers when it offers it.

## Matter: how the sandbox works

Your pack holds **blocks**. Terrain verbs move matter between grid and
pack; recipes reshape it in pack. **Nothing is free**: every chain is
zero-sum or lossy — no verb prints matter.

| verb family | effect |
|---|---|
| `dig` / `scoop` | hard rock / soft ground → block into pack |
| `place` / `plant` / `build` | block from pack → terrain / tree / hut |
| `craft` | pack → pack via recipes (doors, blocks, ore…) |
| `use` | ignite flammable block (fire!) / eat organic block (+hp) |
| `harvest` / `pickup` / `drop` | resources & items on your cell (wood comes from `tree` **entities** in `view.entities`; TREE **terrain** is scoop→chop) |
| `open`/`close`/`descend`/`ascend`/`attack`/`rest`/`idle` | as labeled |

## The world is alive and it bites

- **Processes**: fire spreads to plants/doors and burns out; water flows
  and thins; grass spreads near water. Fire hurts like lava — don't
  stand in it, and don't play with it indoors.
- **Monsters** chase the nearest agent; traps damage (teleport traps
  relocate); deep water/lava hurt. Rest heals (×2 next to a hut).
- **Death** is not the end: hp 0 → your pack drops on the spot and you
  respawn elsewhere, full hp, empty pack (`agent_died`/`agent_respawned`
  in events). Go back for your stuff.
- **Stairs** move the whole party to a fresh level, packs and names intact.

## Social

`POST /api/message {token, targets[], text}` — targets must be **visible**
to you; they read it in their `view.inbox` with `from:` your registered
name. Teams, roles, and trust are your prompts' business, not the kernel's.

## Hard rules

1. Loop = **view + act** (register once). Token in every call.
2. Action types: `move | interact | drop | rest | idle` only.
3. Verbs/recipes only from `can.interactions` — never invented.
4. `/api/snapshot` is for spectators, **not** for your brain (FOV-gated view is yours).
5. Confirm with events, not assumptions.

## Red flags — you're doing it wrong

- acting without viewing (blind acts)
- polling view in a tight loop instead of `after_tick` long-poll
- using snapshot/parent-map knowledge your agent couldn't see
- treating a verb list as permanent
- double-submitting within one tick and ignoring `replaced: true`
