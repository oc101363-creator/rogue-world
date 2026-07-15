# Agent Simulation Kernel (ASK) — MVP-0 Design

**Date:** 2026-07-15  
**Status:** Approved for implementation  
**Codename:** ASK  

---

## 1. Positioning

**Not** Agent Roguelike. **Not** AI RTS.

**Is** a **Digital World Kernel** for Agents:

> Organization → Agent → Action → World

Mission: **Build World**, not Make Game.

### FrogComposband inheritance (thought only)

| Borrow | Frog source idea | ASK mapping |
|--------|------------------|-------------|
| Phased main loop | `process_player` → `process_monsters` → `process_world` → `game_turn++` | Collect → Apply → WorldSystems → CommitView → tick++ |
| Tile grid | `cave_type` feat + occupancy | `Grid<Terrain>` + ECS `Position` |
| Data-driven terrain flags | `f_info.txt` MOVE/PLACE/… | `TerrainFlags` (minimal) |
| Command → effect | `do_cmd_*` → `move_player_effect` | `Action` → apply systems |
| Deferred notice | `notice_stuff` / `handle_stuff` / `Term_fresh` | `EventBuf` + ASCII frame |
| Whole-world save | savefile serialize cave/entities | `serde` `WorldSnapshot` |
| Entity pools | `m_list` / `o_list` | `bevy_ecs` entities |

**Do not fork** Frog C code or game content (classes, dungeon, combat, exp, UI).

---

## 2. MVP-0 scope

### In

- Rust workspace + `ask-kernel` binary
- `bevy_ecs` only (no Bevy renderer / window App)
- Grid world 16×12, border walls
- Entities: Agent, Tree, Iron vein, Hut
- Actions: Move, Harvest, BuildHut, Idle
- Events this-tick buffer
- In-process mock agent policy
- ASCII render (`--watch` / print each step)
- serde JSON save/load
- Unit tests for move / harvest / build
- Stub modules: `gateway`, `memory`, `skill`, `org` (traits only)

### Out

- Python runtime / WebSocket
- Organization graph runtime
- Skill marketplace
- Full four-layer memory
- Combat, trade, talk
- Browser / Pixi
- Energy-based speed system (use 1 action/agent/tick)

---

## 3. Architecture

```text
ask-kernel/
  src/
    main.rs           # CLI
    lib.rs
    config.rs
    tick.rs           # phased loop
    grid.rs           # terrain + flags
    components.rs     # ECS components
    actions.rs        # Action enum + queue
    events.rs         # Event enum + buffer
    systems/
      mod.rs
      movement.rs
      harvest.rs
      build.rs
    agents/
      mod.rs          # AgentPolicy trait
      mock.rs
    view.rs           # ASCII
    persist.rs        # snapshot save/load
    gateway.rs        # stub trait
    memory.rs         # stub
    skill.rs          # stub
    org.rs            # stub
```

### Tick phases (sole entry)

```text
1. CollectActions   // mock policy → ActionQueue
2. ApplyActions     // sort by entity id, validate, mutate
3. WorldSystems     // empty / future regen
4. CommitView       // optional ASCII; clear or retain events
5. tick += 1
```

---

## 4. Data model

### Terrain

```rust
enum Terrain { Wall, Floor }
struct TerrainFlags { walk: bool, build: bool } // Wall: false/false; Floor: true/true
```

### Components

- `Position { x: i32, y: i32 }`
- `Glyph(char)`
- `Agent`
- `Inventory { wood: u32, iron: u32 }`
- `Resource { kind: Wood | Iron, amount: u32 }`
- `Building` // Hut

### Actions

```rust
enum Action {
  Move { dx: i32, dy: i32 }, // 4-way |dx|+|dy|==1
  Harvest,
  BuildHut,
  Idle,
}
```

One mutating action per agent per tick.

### Events

`TickStarted`, `Moved`, `MoveFailed`, `Harvested`, `ResourceDepleted`, `Built`, `BuildFailed`, `ActionRejected`

### Snapshot

```json
{
  "tick": 12,
  "width": 16,
  "height": 12,
  "cells": ["wall"|"floor", ...],
  "entities": [
    { "id": 1, "kind": "agent", "x": 2, "y": 9, "wood": 0, "iron": 0 },
    { "id": 2, "kind": "tree", "x": 8, "y": 2, "amount": 5 },
    { "id": 3, "kind": "iron", "x": 5, "y": 5, "amount": 5 },
    { "id": 4, "kind": "hut", "x": 3, "y": 9 }
  ]
}
```

Entity ids in snapshot may be stable u64 assigned at spawn.

---

## 5. Rules

| Action | Rules |
|--------|-------|
| Move | 4-way; target in bounds; terrain.walk; no blocking building required for MVP (agents can share cell with resource) |
| Harvest | Same cell has Resource amount>0; amount-=1; inventory +=1; amount==0 → despawn entity |
| BuildHut | Floor; no Building on cell; wood>=3; wood-=3; spawn Hut |
| Idle | No-op |

### Mock policy

1. If on resource with amount>0 → Harvest  
2. Else if wood>=3 and cell has no building → BuildHut  
3. Else step toward nearest Tree (Manhattan, 4-way)  
4. Else Idle  

---

## 6. CLI

```text
ask-kernel --steps 50
ask-kernel --watch --tick-ms 200
ask-kernel --steps 20 --save data/world.json
ask-kernel --load data/world.json --steps 10
ask-kernel --seed 1
```

Defaults: width=16, height=12, steps=30 if neither watch nor steps given use steps=30.

---

## 7. Toolchain

- Rust **stable recent** (update if host is 1.64)
- `bevy_ecs` version compatible with toolchain (prefer 0.14+ if MSRV allows; else pin)
- Edition 2021

---

## 8. Acceptance

1. `cargo test -p ask-kernel` passes (move, harvest, build)  
2. `cargo run -p ask-kernel -- --steps 40` shows agent gathering wood and optionally building hut  
3. Save then load continues consistently  
4. No network / Python required  

---

## 9. Future (not MVP-0)

- AgentGateway WebSocket + Python runtime  
- Organization graph  
- Skill policies  
- Memory layers  
- External viewers (TUI crate, browser) hanging off events/snapshots only  
