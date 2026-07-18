# ASK Architecture

Layering is **one-way** and enforced by `tests/architecture.rs`. If you need
an edge the rules forbid, the design is wrong — not the test.

```
data tables     f_info.rs  k_info.rs  r_info.rs  vaults.rs   (parse data/, no crate deps)
     ↓
components.rs   pure data model (Matter, Inventory, Health, VisionMemory, …)
     ↓
rules           sandbox.rs (tables: dig/scoop/place/recipes)
                systems/   (movement, dig, craft, combat, monster, death, …)
                systems/verbs.rs  verb registry (name/priority/doc/apply)
                systems/interact.rs  discovery ("what can I do on this cell")
     ↓
projections     describe.rs   ONE entity kind + ONE entity JSON
                viewer.rs     spectator snapshot (FOV-masked)
                agent_view.rs agent-local view (self/view/can/inbox/events)
                inspect.rs    click-through detail (visibility-gated by serve)
     ↓
serve/          mod.rs (AppState, router, sim task) · api.rs (handlers) · ws.rs
     ↓
static/         web client (identity-first render from feat_ids + /api/art)
```

## Rules of the house

1. **One truth per concept.** Verbs → `systems/verbs.rs`. Entity kinds →
   `describe::EntityKind`. Numbers → `balance.rs`. Range check →
   `actions::check_step`. Id allocation → `world::next_id`. Cell queries →
   `spatial.rs`. If you're writing it a second time, stop.
2. **FOV is server-side.** Projections never emit unseen cells: viewer masks
   feat_ids to 0, agent_view only renders the FOV window.
3. **Token = identity.** `agent_id` alone authorizes nothing. Ops = dev token.
4. **Presentation is not gameplay.** `art.rs`/`static/*` never feed simulation
   decisions (guarded by test since the FS-HDG split).
5. **Sim owns the world.** HTTP handlers resolve identity and call
   projections; they never mutate gameplay state directly (actions go
   through the bus → tick).
6. **Events flow one way.** Systems push `GameEvent` into `EventBuf`; the sim
   task drains into a capped ring; projections read the ring.
7. **Matter is conserved.** No verb/craft chain may be net-positive on any
   matter kind; natural sources (resource entities, fresh rock) are the only
   inputs. `place` returns displaced hard rock only; bonus-paying veins
   crumble; scooping a tree takes its resource. Guarded by
   `scoop_place_cycle_creates_nothing`, `ore_vein_cycle_cannot_print_iron`,
   `dig_place_cannot_print_iron`, `craft_chain_never_net_positive`,
   `plant_scoop_harvest_zero_sum`.
   World processes follow the same law: fire consumes fuel and its ash rate
   is < 1 block per wood block; deep springs RUN DRY (one deep cell yields
   exactly two shallow cells, and 2-shallow→1-deep craft makes the cycle
   zero-sum); trees never self-replicate (plant only). Carve-out: the
   grass/ash micro-economy (eat grass for hp, burn farm for ~10% rubble)
   is sanctioned — it has no path to wood/iron and is bounded by fuel
   cost and hp cap.

## Tick phases (sole entry: `Sim::step`)

1. `begin_tick` — TickStarted event, clear action queue
2. collect — `policy.decide()` per agent (BusPolicy: bus > registered-idle
   > human_control-idle > manual-idle > mock)
3. `apply_actions` — sorted by entity, dispatch via verb registry
4. `pickup_items` — same-cell auto pickup
5. `process_monsters` — nearest-agent chase/attack
6. `check_deaths` — hp 0 → drop pack + respawn
7. `process_world` — fire/water/grass evolve (every PROCESS_EVERY_N ticks,
   rules in process_rules.rs; glow recomputed from base + F:-line LIT emitters)
8. level change — `PendingLevelChange` → rebuild, preserving ALL agents
9. vision — union FOV (internal) + per-agent memory (bbox only)
10. `advance_tick`

## Key structures

- `Grid` = row-major `Vec<u16>` frog feat ids; semantics in `f_info::FeatTable`
  (the `f_info::id` constants are contract-tested against the data file).
- `Matter = Terrain{feat} | Resource{kind} | Object{kind_id}` — the pack atom.
- Save format v2 (`persist.rs`): version field, all entity kinds, agent
  identity, depth/seed/glow. v1 files still load.
