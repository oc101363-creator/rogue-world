# World Processes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (parallelized — tasks are file-disjoint by design; see "Parallel dispatch" at the end). Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the world evolve on its own — fire spreads/dies, water flows/thins, grass spreads — via one data-driven process engine, plus a generic `use` verb, with zero changes to vision/terrain/auth systems beyond additive ones.

**Architecture:** `process_rules.rs` (pure data table) + `systems/process.rs` (engine, runs every 8 ticks after check_deaths) + `systems/use_item.rs` (one verb, dispatches by block flags). All rates in `balance.rs`. Deterministic via `hash(seed ^ tick ^ cell_idx)`. Conservation per ARCHITECTURE rule 7 is re-verified per rule with tests.

**Tech Stack:** Rust (bevy_ecs), cargo test. All tasks TDD.

## Global Constraints

- Repo root is `/Users/mingkaichen/项目/AgentGame` (main branch, work directly here — changes are additive and covered by tests).
- Run `export PATH="$HOME/.cargo/bin:$PATH"` before cargo. `cargo test -p ask-kernel` must stay green (74 baseline) with ZERO warnings.
- Commit format: `<type>(<scope>): <summary>` + trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Design spec: `docs/superpowers/specs/2026-07-17-world-processes-design.md` — rules are the 7 table rows there; rates as in spec (fire spread 15, burnout 6, douse 100, evaporate 20, deep flow 2, shallow flow 8, grass 8; PROCESS_EVERY_N 8).
- Layering (guarded by tests/architecture.rs): process_rules.rs may depend only on f_info; systems/* may not import art/serve/viewer; components stays dependency-free of rules.

---

### Task 1: FIRE feat + id 常量 + 契约测试

**Files:**
- Modify: `crates/ask-kernel/data/f_info.txt` (append entry)
- Modify: `crates/ask-kernel/src/f_info.rs` (id constant)
- Modify: `crates/ask-kernel/tests/f_info_contract.rs` (add FIRE case)

**Interfaces:**
- Produces: `f_info::id::FIRE: u16 = 99`; feat name "FIRE" with flags WALK|LAVA|LIT (walk → enterable, LAVA flag → on_enter_cell damage branch already applies, LIT → glows).

- [ ] **Step 1: failing test**

Append to `crates/ask-kernel/tests/f_info_contract.rs` inside `semantic_flags_match_constants` (or new test `fire_feat_is_walking_lava_lit`):

```rust
#[test]
fn fire_feat_contract() {
    let t = table();
    let f = t.get(id::FIRE).expect("FIRE feat must exist");
    assert_eq!(f.name, "FIRE");
    assert!(f.walk, "FIRE is enterable";
    assert!(f.lava, "FIRE reuses lava damage branch");
    assert!(t.get(id::FIRE).map(|x| x.glyph).unwrap_or('?') == '!');
}
```

Note: if `walk`/`lava` assertion fields differ, match FeatInfo field names (`walk`, `lava`).

- [ ] **Step 2: run, expect compile error (`id::FIRE` missing)**

Run: `cargo test -p ask-kernel --test f_info_contract 2>&1 | tail -3`

- [ ] **Step 3: implement**

Append to `crates/ask-kernel/data/f_info.txt` (at end of file, format matches existing entries):

```
N:99:FIRE
E:spreading fire
G:!:r
F:WALK | LAVA | LIT
```

In `crates/ask-kernel/src/f_info.rs` `pub mod id`, after `MOUNTAIN`:

```rust
    pub const FIRE: u16 = 99;
```

- [ ] **Step 4: run**

Run: `cargo test -p ask-kernel --test f_info_contract 2>&1 | tail -3` → PASS. Also `cargo test -p ask-kernel --lib` → PASS (f_info load test).

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/data/f_info.txt crates/ask-kernel/src/f_info.rs crates/ask-kernel/tests/f_info_contract.rs
git commit -m "feat(f_info): FIRE feat (N:99) — walkable lava-flagged lit terrain

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: process_rules.rs 纯数据规则表

**Files:**
- Create: `crates/ask-kernel/src/process_rules.rs`
- Modify: `crates/ask-kernel/src/lib.rs` (register module)

**Interfaces:**
- Consumes: `f_info::id::*`, `f_info::FeatInfo` flags (tree/brake/door/water/walk)
- Produces (used by engine Task 4):
```rust
pub enum CellCond { FeatIs(FeatId), TreeLike, Water, ShallowWater, Grass }
pub enum NeighborCond { None, Flammable, AnyFeat(FeatId), FlowTarget, DirtWithWaterNear }
pub enum ProcessAction {
    NeighborBecomes(FeatId),
    SelfBecomes(FeatId),
    SelfBecomesOneOf(&'static [(FeatId, u8)]),
    NeighborAndSelf { neighbor: FeatId, self_becomes: Option<(FeatId, u8)> },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause { Fire, Water, Growth }
pub struct ProcessRule { pub name: &'static str, pub on: CellCond, pub neighbors: NeighborCond, pub action: ProcessAction, pub chance_pct: u8, pub cause: Cause }
pub fn rules() -> &'static [ProcessRule]
pub fn is_flammable(f: &crate::f_info::FeatInfo) -> bool  // tree || brake || door
pub fn is_flow_target(f: &crate::f_info::FeatInfo) -> bool // walk && !water && !door
```

- [ ] **Step 1: failing test**

`tests/` not needed — put unit tests in the module itself (`#[cfg(test)] mod tests`): assert rules() has 7 entries, all referenced feat ids exist in `f_info::table()`, names unique.

- [ ] **Step 2: run `cargo test -p ask-kernel --lib process_rules 2>&1 | tail -3` → module missing error**

- [ ] **Step 3: implement `crates/ask-kernel/src/process_rules.rs`**

```rust
//! World process rules — PURE DATA, zero ECS (same philosophy as sandbox.rs).
//!
//! 只写简单规则，不写手工结局。Gameplay (torch posts, moats, fire belts,
//! farms) is emergent from these rows, never hand-authored features.

use crate::f_info::{self, FeatId, FeatInfo};

#[derive(Clone, Copy, Debug)]
pub enum CellCond {
    FeatIs(FeatId),
    ShallowWater,
    Grass,
}

#[derive(Clone, Copy, Debug)]
pub enum NeighborCond {
    None,
    /// Any 4-neighbor that burns: tree/brake/door flags.
    Flammable,
    /// Any 4-neighbor equal to this feat.
    AnyFeat(FeatId),
    /// Water may flow here: walk && !water && !door.
    FlowTarget,
    /// Any 4-neighbor DIRT, with water within Manhattan distance 3.
    DirtWithWaterNear,
}

#[derive(Clone, Copy, Debug)]
pub enum ProcessAction {
    NeighborBecomes(FeatId),
    SelfBecomes(FeatId),
    SelfBecomesOneOf(&'static [(FeatId, u8)]),
    NeighborAndSelf {
        neighbor: FeatId,
        self_becomes: Option<(FeatId, u8)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Fire,
    Water,
    Growth,
}

pub struct ProcessRule {
    pub name: &'static str,
    pub on: CellCond,
    pub neighbors: NeighborCond,
    pub action: ProcessAction,
    pub chance_pct: u8,
    pub cause: Cause,
}

/// Burns: any plant or woodwork (flags, never an id list).
pub fn is_flammable(f: &FeatInfo) -> bool {
    f.tree || f.brake || f.door
}

/// Water may flow into this: open, not already water, not a door.
pub fn is_flow_target(f: &FeatInfo) -> bool {
    f.walk && !f.water && !f.door
}

use crate::balance as b;
use crate::f_info::id;

/// THE world processes. Order matters: fire, water, grass; first rule to
/// transform a cell wins that tick.
pub fn rules() -> &'static [ProcessRule] {
    &[
        ProcessRule {
            name: "fire_spread",
            on: CellCond::FeatIs(id::FIRE),
            neighbors: NeighborCond::Flammable,
            action: ProcessAction::NeighborBecomes(id::FIRE),
            chance_pct: b::FIRE_SPREAD_PCT,
            cause: Cause::Fire,
        },
        ProcessRule {
            name: "fire_burnout",
            on: CellCond::FeatIs(id::FIRE),
            neighbors: NeighborCond::None,
            action: ProcessAction::SelfBecomesOneOf(&[(id::FLOOR, 90), (id::RUBBLE, 10)]),
            chance_pct: b::FIRE_BURNOUT_PCT,
            cause: Cause::Fire,
        },
        ProcessRule {
            name: "fire_douse",
            on: CellCond::FeatIs(id::SHALLOW_WATER),
            neighbors: NeighborCond::AnyFeat(id::FIRE),
            action: ProcessAction::NeighborBecomes(id::FLOOR),
            chance_pct: 100,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_evaporate",
            on: CellCond::ShallowWater,
            neighbors: NeighborCond::AnyFeat(id::FIRE),
            action: ProcessAction::SelfBecomes(id::FLOOR),
            chance_pct: b::WATER_EVAPORATE_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_flow_deep",
            on: CellCond::FeatIs(id::DEEP_WATER),
            neighbors: NeighborCond::FlowTarget,
            action: ProcessAction::NeighborBecomes(id::SHALLOW_WATER),
            chance_pct: b::WATER_FLOW_DEEP_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_flow_shallow",
            on: CellCond::ShallowWater,
            neighbors: NeighborCond::FlowTarget,
            action: ProcessAction::NeighborAndSelf {
                neighbor: id::SHALLOW_WATER,
                self_becomes: Some((id::FLOOR, 100)),
            },
            chance_pct: b::WATER_FLOW_SHALLOW_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "grass_spread",
            on: CellCond::Grass,
            neighbors: NeighborCond::DirtWithWaterNear,
            action: ProcessAction::NeighborBecomes(id::GRASS),
            chance_pct: b::GRASS_SPREAD_PCT,
            cause: Cause::Growth,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sane() {
        let t = f_info::table();
        let rs = rules();
        assert_eq!(rs.len(), 7);
        for r in rs {
            match r.on {
                CellCond::FeatIs(f) => assert!(t.get(f).is_some(), "{}: bad feat", r.name),
                _ => {}
            }
            match r.action {
                ProcessAction::NeighborBecomes(f) | ProcessAction::SelfBecomes(f) => {
                    assert!(t.get(f).is_some(), "{}: bad action feat", r.name)
                }
                ProcessAction::SelfBecomesOneOf(list) => {
                    for (f, _) in list {
                        assert!(t.get(*f).is_some());
                    }
                }
                ProcessAction::NeighborAndSelf { neighbor, self_becomes } => {
                    assert!(t.get(neighbor).is_some());
                    if let Some((f, _)) = self_becomes {
                        assert!(t.get(f).is_some());
                    }
                }
            }
        }
        let names: std::collections::HashSet<_> = rs.iter().map(|r| r.name).collect();
        assert_eq!(names.len(), 7, "rule names unique");
        // fire feats really are flammable/dousable per flags
        let fire = t.get(id::FIRE).unwrap();
        assert!(fire.walk && fire.lava);
    }
}
```

`lib.rs`: add `pub mod process_rules;` (alphabetical near persist).

- [ ] **Step 4: run `cargo test -p ask-kernel --lib 2>&1 | tail -4` → PASS**

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/src/process_rules.rs crates/ask-kernel/src/lib.rs
git commit -m "feat(process): data-driven world process rule table (7 rows, pure data)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: 事件变体 + FOV 门 + 前端日志

**Files:**
- Modify: `crates/ask-kernel/src/events.rs`
- Modify: `crates/ask-kernel/static/render.js` (formatEvents)

**Interfaces:**
- Produces: `GameEvent::TerrainChanged { at: (i32,i32), from: u16, to: u16, cause: crate::process_rules::Cause }`; `GameEvent::Consumed { entity: u64, label: String, hp: i32 }`; `event_visible` covers both.

- [ ] **Step 1: failing test**

Append to `crates/ask-kernel/src/events.rs` a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::VisionMap;

    #[test]
    fn terrain_changed_visibility_rules() {
        let mut vis = VisionMap::new(10, 10);
        vis.flags[(2 * 10 + 3) as usize] = crate::vision::F_VIEW | crate::vision::F_LITE;
        let ev = GameEvent::TerrainChanged {
            at: (3, 2),
            from: 96,
            to: 99,
            cause: crate::process_rules::Cause::Fire,
        };
        assert!(event_visible(&ev, &vis, None));
        let far = GameEvent::TerrainChanged {
            at: (8, 8),
            from: 96,
            to: 99,
            cause: crate::process_rules::Cause::Fire,
        };
        assert!(!event_visible(&far, &vis, None));
        let ate = GameEvent::Consumed { entity: 7, label: "GRASS".into(), hp: 5 };
        assert!(event_visible(&ate, &vis, Some(7)));
        assert!(!event_visible(&ate, &vis, Some(8)));
    }
}
```

- [ ] **Step 2: run `cargo test -p ask-kernel --lib events 2>&1 | tail -3` → compile error (variants missing)**

- [ ] **Step 3: implement**

In `crates/ask-kernel/src/events.rs`, add variants (after `AgentRespawned`):

```rust
    /// World process transformed a cell (fire/water/growth).
    TerrainChanged {
        at: (i32, i32),
        from: u16,
        to: u16,
        cause: crate::process_rules::Cause,
    },
    /// Agent consumed an organic block for hp.
    Consumed {
        entity: u64,
        label: String,
        hp: i32,
    },
```

In `event_visible` add arms:

```rust
        GameEvent::TerrainChanged { at, .. } => at_seen(*at),
        GameEvent::Consumed { entity, .. } => is_self(*entity),
```

In `crates/ask-kernel/static/render.js` `formatEvents`, append before the closing brace of the loop body:

```js
    else if (t === "terrain_changed") pushLog(`≋ ${ev.cause} (${ev.at[0]},${ev.at[1]})`);
    else if (t === "consumed") pushLog(`吃 ${ev.label} hp=${ev.hp}`);
```

- [ ] **Step 4: run `cargo test -p ask-kernel --lib 2>&1 | tail -4` → PASS**

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/src/events.rs crates/ask-kernel/static/render.js
git commit -m "feat(events): TerrainChanged + Consumed with FOV-gated visibility

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: 引擎 systems/process.rs + tick 接入 + 动态 glow + balance

**Files:**
- Create: `crates/ask-kernel/src/systems/process.rs`
- Modify: `crates/ask-kernel/src/systems/mod.rs` (register + re-export)
- Modify: `crates/ask-kernel/src/tick.rs` (insert phase)
- Modify: `crates/ask-kernel/src/vision.rs` (GlowMask gains base_mask + recompute helper)
- Modify: `crates/ask-kernel/src/balance.rs` (rate constants)
- Modify: `crates/ask-kernel/src/systems/mod.rs`

**Interfaces:**
- Consumes: `process_rules::{rules, CellCond, NeighborCond, ProcessAction, Cause, is_flammable, is_flow_target}` (T2); `GameEvent::TerrainChanged` (T3); `f_info::id`, `Grid`, `GlowMask`
- Produces: `pub fn process_world(world: &mut World)` — called from tick after `check_deaths`

- [ ] **Step 1: failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn fire_spreads_to_forest_and_burns_out_deterministically() {
    use ask_kernel::components::VisionMemory;
    use ask_kernel::world::{TickCounter, WorldSeed};

    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 123;
    let mut kw = KernelWorld::new(&cfg);
    let g = kw.world.resource::<Grid>();
    let (w, h) = (g.width, g.height);
    drop(g);
    // strip to a clean stage: granite everywhere, a grass corridor, fire at one end
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for i in 0..grid.cells.len() {
            grid.cells[i] = id::GRANITE;
        }
        for x in 1..(w - 1) {
            grid.set(x, h / 2, id::GRASS);
        }
        grid.set(1, h / 2, id::FIRE);
    }
    // run process ticks until spread happens (or cap)
    let mut burned = false;
    for _ in 0..(ask_kernel::balance::PROCESS_EVERY_N * 30) {
        *kw.world.resource_mut::<TickCounter>() = TickCounter(kw.tick() + 1);
        ask_kernel::systems::process_world(&mut kw.world);
        if kw.world.resource::<Grid>().get(2, h / 2) == Some(id::FIRE) {
            burned = true;
            break;
        }
    }
    assert!(burned, "fire never spread to adjacent grass");
    // and some fire eventually burns out
    let mut out_seen = false;
    for _ in 0..(ask_kernel::balance::PROCESS_EVERY_N * 40) {
        *kw.world.resource_mut::<TickCounter>() = TickCounter(kw.tick() + 1);
        ask_kernel::systems::process_world(&mut kw.world);
        let g = kw.world.resource::<Grid>();
        if (1..(w - 2)).any(|x| {
            let f = g.get(x, h / 2);
            f == Some(id::FLOOR) || f == Some(id::RUBBLE)
        }) {
            out_seen = true;
            break;
        }
    }
    assert!(out_seen, "fire never burned out");
    // determinism: same seed fresh run → identical first spread tick
    let mut kw2 = KernelWorld::new(&cfg);
    {
        let mut grid = kw2.world.resource_mut::<Grid>();
        for i in 0..grid.cells.len() {
            grid.cells[i] = id::GRANITE;
        }
        for x in 1..(w - 1) {
            grid.set(x, h / 2, id::GRASS);
        }
        grid.set(1, h / 2, id::FIRE);
    }
    let mut tick2 = 0u64;
    loop {
        tick2 += 1;
        *kw2.world.resource_mut::<TickCounter>() = TickCounter(kw2.tick() + 1);
        ask_kernel::systems::process_world(&mut kw2.world);
        if kw2.world.resource::<Grid>().get(2, h / 2) == Some(id::FIRE) || tick2 > 400 {
            break;
        }
    }
    let mut tick1 = 0u64;
    let mut kw1 = KernelWorld::new(&cfg);
    {
        let mut grid = kw1.world.resource_mut::<Grid>();
        for i in 0..grid.cells.len() {
            grid.cells[i] = id::GRANITE;
        }
        for x in 1..(w - 1) {
            grid.set(x, h / 2, id::GRASS);
        }
        grid.set(1, h / 2, id::FIRE);
    }
    loop {
        tick1 += 1;
        *kw1.world.resource_mut::<TickCounter>() = TickCounter(kw1.tick() + 1);
        ask_kernel::systems::process_world(&mut kw1.world);
        if kw1.world.resource::<Grid>().get(2, h / 2) == Some(id::FIRE) || tick1 > 400 {
            break;
        }
    }
    assert_eq!(tick1, tick2, "same seed must reproduce the same process");
    let _ = VisionMemory::new(1, 1);
    let _ = WorldSeed(0);
}
```

- [ ] **Step 2: run → `process_world` / `balance::PROCESS_EVERY_N` missing**

- [ ] **Step 3: implement**

`crates/ask-kernel/src/balance.rs` append:

```rust
// --- world processes ---
pub const PROCESS_EVERY_N: u64 = 8;
pub const FIRE_SPREAD_PCT: u8 = 15;
pub const FIRE_BURNOUT_PCT: u8 = 6;
pub const WATER_EVAPORATE_PCT: u8 = 20;
pub const WATER_FLOW_DEEP_PCT: u8 = 2;
pub const WATER_FLOW_SHALLOW_PCT: u8 = 8;
pub const GRASS_SPREAD_PCT: u8 = 8;
/// Glow radius of LIT feats (torch/fire).
pub const LIT_GLOW_RADIUS: i32 = 5;
```

`crates/ask-kernel/src/systems/process.rs` (new):

```rust
//! World process engine — applies process_rules to the Grid every N ticks.
//! 只写简单规则，不写手工结局: this file knows HOW to run rules, never WHAT
//! the game is. New processes go in process_rules.rs, not here.

use bevy_ecs::prelude::*;

use crate::events::{EventBuf, GameEvent};
use crate::f_info::{self, FeatId};
use crate::grid::Grid;
use crate::process_rules::{self, CellCond, NeighborCond, ProcessAction};
use crate::vision::GlowMask;
use crate::world::{TickCounter, WorldSeed};

/// Deterministic per-cell-per-tick roll (0..100).
fn roll(seed: u64, tick: u64, idx: usize) -> u8 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((idx as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
        | 1;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % 100) as u8
}

fn cell_matches(cond: &CellCond, feat: FeatId) -> bool {
    match *cond {
        CellCond::FeatIs(f) => feat == f,
        CellCond::ShallowWater => feat == f_info::id::SHALLOW_WATER,
        CellCond::Grass => feat == f_info::id::GRASS,
    }
}

fn neighbor_matches(
    grid: &Grid,
    x: i32,
    y: i32,
    cond: &NeighborCond,
) -> Option<(i32, i32)> {
    let table = f_info::table();
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    match *cond {
        NeighborCond::None => None,
        NeighborCond::Flammable => dirs.iter().find_map(|&(dx, dy)| {
            grid.get(x + dx, y + dy)
                .and_then(|f| table.get(f))
                .filter(|info| process_rules::is_flammable(info))
                .map(|_| (x + dx, y + dy))
        }),
        NeighborCond::AnyFeat(want) => dirs.iter().find_map(|&(dx, dy)| {
            if grid.get(x + dx, y + dy) == Some(want) {
                Some((x + dx, y + dy))
            } else {
                None
            }
        }),
        NeighborCond::FlowTarget => dirs.iter().find_map(|&(dx, dy)| {
            grid.get(x + dx, y + dy)
                .and_then(|f| table.get(f))
                .filter(|info| process_rules::is_flow_target(info))
                .map(|_| (x + dx, y + dy))
        }),
        NeighborCond::DirtWithWaterNear => {
            let dirt = dirs.iter().find_map(|&(dx, dy)| {
                if grid.get(x + dx, y + dy) == Some(f_info::id::DIRT) {
                    Some((x + dx, y + dy))
                } else {
                    None
                }
            })?;
            // water within Manhattan distance 3 of the GRASS cell
            let r = 3;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r || (dx == 0 && dy == 0) {
                        continue;
                    }
                    if grid
                        .get(x + dx, y + dy)
                        .and_then(|f| table.get(f))
                        .map(|info| info.water)
                        .unwrap_or(false)
                    {
                        return Some(dirt);
                    }
                }
            }
            None
        }
    }
}

pub fn process_world(world: &mut World) {
    let tick = world.resource::<TickCounter>().0;
    if tick % crate::balance::PROCESS_EVERY_N != 0 {
        return;
    }
    let seed = world.get_resource::<WorldSeed>().map(|s| s.0).unwrap_or(1);
    let (w, h, cells) = {
        let g = world.resource::<Grid>();
        (g.width, g.height, g.cells.clone())
    };
    let rules = process_rules::rules();
    let mut changed: Vec<(i32, i32, FeatId, FeatId, process_rules::Cause)> = Vec::new();
    let mut claimed: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for rule in rules {
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if claimed.contains(&idx) {
                    continue;
                }
                let feat = cells[idx];
                if !cell_matches(&rule.on, feat) {
                    continue;
                }
                let Some((nx, ny)) = neighbor_matches(&Grid { width: w, height: h, cells: cells.clone() }, x, y, &rule.neighbors)
                    .or(if matches!(rule.neighbors, NeighborCond::None) { Some((x, y)) } else { None })
                else {
                    continue;
                };
                if roll(seed, tick, idx) >= rule.chance_pct {
                    continue;
                }
                match rule.action {
                    ProcessAction::NeighborBecomes(f) => {
                        claimed.insert((ny * w + nx) as usize);
                        changed.push((nx, ny, f, feat, rule.cause));
                    }
                    ProcessAction::SelfBecomes(f) => {
                        claimed.insert(idx);
                        changed.push((x, y, f, feat, rule.cause));
                    }
                    ProcessAction::SelfBecomesOneOf(list) => {
                        let r2 = roll(seed ^ 0xA5A5, tick, idx) as u32;
                        let mut acc = 0u32;
                        let mut pick = list[0].0;
                        for (f, wgt) in list {
                            acc += *wgt as u32;
                            if r2 < acc {
                                pick = *f;
                                break;
                            }
                        }
                        claimed.insert(idx);
                        changed.push((x, y, pick, feat, rule.cause));
                    }
                    ProcessAction::NeighborAndSelf { neighbor, self_becomes } => {
                        claimed.insert((ny * w + nx) as usize);
                        changed.push((nx, ny, neighbor, feat, rule.cause));
                        if let Some((sf, pct)) = self_becomes {
                            if roll(seed ^ 0x5A5A, tick, idx) < pct {
                                claimed.insert(idx);
                                changed.push((x, y, sf, feat, rule.cause));
                            }
                        }
                    }
                }
            }
        }
    }

    // apply
    let mut events: Vec<GameEvent> = Vec::new();
    {
        let mut grid = world.resource_mut::<Grid>();
        for (x, y, to, from, cause) in &changed {
            grid.set(*x, *y, *to);
            events.push(GameEvent::TerrainChanged {
                at: (*x, *y),
                from: *from,
                to: *to,
                cause: *cause,
            });
        }
    }
    if !events.is_empty() {
        let mut buf = world.resource_mut::<EventBuf>();
        for e in events {
            buf.push(e);
        }
    }

    // dynamic glow: recompute from base room glow + LIT feats (torch/fire)
    if tick % crate::balance::PROCESS_EVERY_N == 0 {
        crate::vision::recompute_glow(world);
    }
}
```

`crates/ask-kernel/src/vision.rs` — GlowMask gains a base layer + recompute:

```rust
#[derive(Resource, Clone, Debug)]
pub struct GlowMask {
    pub width: i32,
    pub height: i32,
    pub mask: Vec<bool>,
    /// Generation-time room glow — dynamic glow is always `base ∪ LIT feats`.
    pub base: Vec<bool>,
}

impl GlowMask {
    pub fn new(width: i32, height: i32) -> Self {
        let n = (width * height) as usize;
        Self {
            width,
            height,
            mask: vec![false; n],
            base: vec![false; n],
        }
    }

    pub fn from_slice(width: i32, height: i32, slice: &[bool]) -> Self {
        let n = (width * height) as usize;
        let mut mask = vec![false; n];
        for i in 0..n.min(slice.len()) {
            mask[i] = slice[i];
        }
        let base = mask.clone();
        Self { width, height, mask, base }
    }
}

/// mask := base (room light) ∪ glow radius of every LIT feat (fire/torch).
/// Called by the process engine; vision reads `mask` as usual.
pub fn recompute_glow(world: &mut World) {
    let table = f_info::table();
    let (w, h, lit_cells) = {
        let g = world.resource::<Grid>();
        let lit: Vec<(i32, i32)> = (0..g.height)
            .flat_map(|y| (0..g.width).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                g.get(x, y)
                    .and_then(|f| table.get(f))
                    .map(|info| info.lit)
                    .unwrap_or(false)
            })
            .collect();
        (g.width, g.height, lit)
    };
    let r = crate::balance::LIT_GLOW_RADIUS;
    let mut glow = world
        .get_resource_mut::<GlowMask>()
        .map(|g| {
            let mut g = g.clone();
            g
        })
        .unwrap_or_else(|| GlowMask::new(w, h));
    glow.mask.copy_from_slice(&glow.base);
    for (cx, cy) in lit_cells {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= w || y >= h {
                    continue;
                }
                glow.mask[(y * w + x) as usize] = true;
            }
        }
    }
    world.insert_resource(glow);
}
```

(FeatInfo needs a `lit` flag — CHECK: f_info.rs parses flags; if `lit` isn't parsed, add `let lit = up.contains("LIT");` to parse_f_info and the FeatInfo field. LIT is already referenced in G: lines as third field; the F: line flags include LIT.)

`tick.rs` — insert after `check_deaths`:

```rust
        // world processes: fire/water/grass evolve on their own
        process_world(&mut self.kernel.world);
```

(and import `process_world` from crate::systems)

`systems/mod.rs`: `pub mod process;` + `pub use self::process::process_world;`

- [ ] **Step 4: run**

`cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'` → all PASS (new test may need tick-count tuning; keep ≤ 400 loops).

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/src/systems/process.rs crates/ask-kernel/src/systems/mod.rs crates/ask-kernel/src/tick.rs crates/ask-kernel/src/vision.rs crates/ask-kernel/src/balance.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "feat(process): engine + dynamic glow — the world evolves every 8 ticks

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: use 动词（可燃点火 / 有机食用）

**Files:**
- Create: `crates/ask-kernel/src/systems/use_item.rs`
- Modify: `crates/ask-kernel/src/systems/verbs.rs` (registry row)
- Modify: `crates/ask-kernel/src/systems/interact.rs` (discovery)
- Modify: `crates/ask-kernel/src/systems/mod.rs`

**Interfaces:**
- Consumes: `GameEvent::{TerrainChanged, Consumed}` (T3), `id::FIRE` (T1), `is_flammable` (T2)
- Produces: `pub fn apply_use(world, agent, slot: Option<usize>, dx: i32, dy: i32)`; verb "use" priority 14, doc "use a pack block: ignite flammable / eat organic"

- [ ] **Step 1: failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn use_ignites_wood_and_eats_grass() {
    use ask_kernel::components::Matter;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 131;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0 + 1, floor.1, id::TREE);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 2);

    // ignite adjacent tree with a wood block
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 1,
            dy: 0,
            verb: Some("use".into()),
            slot: Some(0),
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0 + 1, floor.1),
        Some(id::FIRE),
        "wood block should ignite the tree"
    );

    // eat a grass block for hp
    kw.world.get_mut::<Health>(agent).unwrap().hp = 10;
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Terrain { feat: id::GRASS }, 1);
    let grass_slot = kw
        .world
        .get::<Inventory>(agent)
        .unwrap()
        .slots
        .len() - 1;
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("use".into()),
            slot: Some(grass_slot),
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 11);
}
```

- [ ] **Step 2: run → `use` unknown_verb**

- [ ] **Step 3: implement `crates/ask-kernel/src/systems/use_item.rs`**

```rust
//! use verb — ONE verb, effects dispatched by block flags (never item ids).

use bevy_ecs::prelude::*;

use crate::balance;
use crate::components::{Health, Inventory, Matter};
use crate::events::{EventBuf, GameEvent};
use crate::f_info::{self, id};
use crate::grid::Grid;
use crate::systems::stable_id;

pub fn apply_use(world: &mut World, agent: Entity, slot: Option<usize>, dx: i32, dy: i32) {
    let eid = stable_id(world, agent);
    let matter = slot.and_then(|i| {
        world
            .get::<Inventory>(agent)
            .and_then(|inv| inv.slots.get(i).map(|s| (i, s.matter.clone())))
    });
    let Some((slot_i, matter)) = matter else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "bad_slot".into(),
            });
        return;
    };

    // flammable? (TREE feat block or wood resource)
    let flammable = match &matter {
        Matter::Terrain { feat } => *feat == id::TREE,
        Matter::Resource { resource } => *resource == crate::components::ResourceKind::Wood,
        _ => false,
    };
    // organic food? (grass/brake feat blocks)
    let organic = match &matter {
        Matter::Terrain { feat } => {
            let t = f_info::table();
            *feat == id::GRASS || *feat == id::BRAKE
                || t.get(*feat).map(|f| f.brake).unwrap_or(false)
        }
        _ => false,
    };

    if flammable {
        return use_ignite(world, agent, eid, slot_i, dx, dy);
    }
    if organic {
        return use_eat(world, agent, eid, slot_i, matter);
    }
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::ActionRejected {
            entity: eid,
            reason: "not_usable".into(),
        });
}

fn use_ignite(world: &mut World, agent: Entity, eid: u64, slot_i: usize, dx: i32, dy: i32) {
    let Some(pos) = world.get::<crate::components::Position>(agent).copied() else {
        return;
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let table = f_info::table();
    let ok = world
        .resource::<Grid>()
        .get(tx, ty)
        .and_then(|f| table.get(f))
        .map(|info| info.walk || crate::process_rules::is_flammable(info))
        .unwrap_or(false);
    if !ok {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "cannot_ignite_here".into(),
            });
        return;
    }
    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        if inv.take_one(slot_i).is_none() {
            return;
        }
    }
    world.resource_mut::<Grid>().set(tx, ty, id::FIRE);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::TerrainChanged {
            at: (tx, ty),
            from: 0,
            to: id::FIRE,
            cause: crate::process_rules::Cause::Fire,
        });
}

fn use_eat(world: &mut World, agent: Entity, eid: u64, slot_i: usize, matter: Matter) {
    let label = matter.label();
    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        if inv.take_one(slot_i).is_none() {
            return;
        }
    }
    let hp_now = {
        let Some(mut h) = world.get_mut::<Health>(agent) else {
            return;
        };
        h.hp = (h.hp + balance::REST_HEAL).min(h.max_hp);
        h.hp
    };
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::Consumed {
            entity: eid,
            label,
            hp: hp_now,
        });
}
```

`verbs.rs` registry, append after craft row:

```rust
        VerbSpec {
            verb: "use",
            priority: 14,
            doc: "use a pack block (slot=index): ignite flammable / eat organic",
            apply: |w, a, c| crate::systems::use_item::apply_use(w, a, c.slot, c.dx, c.dy),
        },
```

`systems/mod.rs`: `pub mod use_item;`

`interact.rs` discovery — in `list_at`, after the place-stack loop, add:

```rust
    // use: offer for flammable blocks (any target cell here) and organic ones (underfoot)
    if let Some(inv) = world.get::<Inventory>(agent) {
        for (si, stack) in inv.slots.iter().enumerate() {
            let (flammable, organic) = match &stack.matter {
                Matter::Terrain { feat } => (
                    *feat == crate::f_info::id::TREE,
                    *feat == crate::f_info::id::GRASS || *feat == crate::f_info::id::BRAKE,
                ),
                Matter::Resource { resource } => {
                    (*resource == crate::components::ResourceKind::Wood, false)
                }
                _ => (false, false),
            };
            let label = if flammable && !(dx == 0 && dy == 0) {
                Some(format!("ignite with {}", stack.matter.label()))
            } else if organic && dx == 0 && dy == 0 {
                Some(format!("eat {}", stack.matter.label()))
            } else {
                None
            };
            if let Some(label) = label {
                out.push(Interaction {
                    dx,
                    dy,
                    verb: "use".into(),
                    label,
                    target_id: None,
                    slot: Some(si),
                    recipe: None,
                });
            }
        }
    }
```

- [ ] **Step 4: run `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'` → PASS**

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/src/systems/use_item.rs crates/ask-kernel/src/systems/verbs.rs crates/ask-kernel/src/systems/interact.rs crates/ask-kernel/src/systems/mod.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "feat(verbs): use — one verb, effects dispatched by block flags (ignite/eat)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: 过程行为测试包 + 文档同步

**Files:**
- Modify: `crates/ask-kernel/tests/sim_rules.rs`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `.claude/skills/ask-sandbox/SKILL.md`

**Interfaces:**
- Consumes: all previous tasks
- Produces: spec §9 tests 3-8; doc updates

- [ ] **Step 1: write tests (append to sim_rules.rs)**

```rust
fn stage_grid(kw: &mut KernelWorld, fill: u16) -> (i32, i32) {
    let (w, h) = {
        let g = kw.world.resource::<Grid>();
        (g.width, g.height)
    };
    let mut grid = kw.world.resource_mut::<Grid>();
    for i in 0..grid.cells.len() {
        grid.cells[i] = fill;
    }
    (w, h)
}

fn run_process(kw: &mut KernelWorld, n: u64) {
    for _ in 0..n {
        *kw.world.resource_mut::<ask_kernel::world::TickCounter>() =
            ask_kernel::world::TickCounter(kw.tick() + 1);
        ask_kernel::systems::process_world(&mut kw.world);
    }
}

#[test]
fn fire_dies_on_water_and_monster_dies_in_fire() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 137;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage_grid(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for x in 1..(w - 1) {
            grid.set(x, 5, id::FLOOR);
            grid.set(x, 6, id::SHALLOW_WATER);
        }
        grid.set(1, 5, id::FIRE);
    }
    run_process(&mut kw, 40 * ask_kernel::balance::PROCESS_EVERY_N);
    // fire never crosses the water row
    let g = kw.world.resource::<Grid>();
    for x in 1..(w - 1) {
        assert_ne!(g.get(x, 6), Some(id::FIRE), "fire crossed water");
    }
    // monster on FIRE cell dies via lava flag
    let e = kw.world.spawn((
        Position { x: 1, y: 5 },
        Glyph('o'),
        Monster { race_id: 1, name: "fire rat".into(), color: 'r' },
        Health { hp: 3, max_hp: 3 },
        StableId(99921),
    )).id();
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        grid.set(1, 5, id::FIRE);
    }
    ask_kernel::systems::monster_move_to(&mut kw.world, e, 1, 5);
    assert!(kw.world.get::<Health>(e).is_none(), "monster must die in fire");
}

#[test]
fn water_thins_and_stays_bounded() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 139;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage_grid(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                grid.set(x, y, id::FLOOR);
            }
        }
        grid.set(w / 2, h / 2, id::DEEP_WATER);
    }
    run_process(&mut kw, 30 * ask_kernel::balance::PROCESS_EVERY_N);
    let g = kw.world.resource::<Grid>();
    let water_cells = g
        .cells
        .iter()
        .filter(|&&f| f == id::SHALLOW_WATER || f == id::DEEP_WATER)
        .count();
    assert!(water_cells > 1, "water never flowed");
    assert!(
        water_cells < (w * h) as usize / 2,
        "water unbounded: {water_cells}"
    );
}

#[test]
fn grass_needs_water_to_spread() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 149;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage_grid(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                grid.set(x, y, id::DIRT);
            }
        }
        grid.set(3, 3, id::GRASS);
    }
    run_process(&mut kw, 30 * ask_kernel::balance::PROCESS_EVERY_N);
    let g = kw.world.resource::<Grid>();
    let grass_dry = g.cells.iter().filter(|&&f| f == id::GRASS).count();
    assert_eq!(grass_dry, 1, "grass spread without water");
    // now add water near
    kw.world.resource_mut::<Grid>().set(4, 3, id::SHALLOW_WATER);
    run_process(&mut kw, 40 * ask_kernel::balance::PROCESS_EVERY_N);
    let g = kw.world.resource::<Grid>();
    let grass_wet = g.cells.iter().filter(|&&f| f == id::GRASS).count();
    assert!(grass_wet > 1, "grass never spread with water");
}

#[test]
fn lit_feat_glows_and_dies_out() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 151;
    let mut kw = KernelWorld::new(&cfg);
    stage_grid(&mut kw, id::GRANITE);
    kw.world.resource_mut::<Grid>().set(5, 5, id::FIRE);
    run_process(&mut kw, ask_kernel::balance::PROCESS_EVERY_N);
    let glow = kw.world.resource::<ask_kernel::vision::GlowMask>();
    assert!(glow.mask[(5 * 33 + 5) as usize], "FIRE must glow");
    // let it burn out, glow clears on next process tick after extinction
    run_process(&mut kw, 60 * ask_kernel::balance::PROCESS_EVERY_N);
    let glow = kw.world.resource::<ask_kernel::vision::GlowMask>();
    let fire_gone = kw.world.resource::<Grid>().get(5, 5) != Some(id::FIRE);
    if fire_gone {
        assert!(!glow.mask[(5 * 33 + 5) as usize], "glow must clear after fire dies");
    }
}
```

(If a test is probabilistically flaky at these tick counts, bump the loop cap; all rates are ≥ 2%/tick so 30-60 process ticks is generous.)

- [ ] **Step 2: run `cargo test -p ask-kernel --test sim_rules 2>&1 | tail -8` → all PASS**

- [ ] **Step 3: docs**

`docs/ARCHITECTURE.md` tick phases list — insert after "6. `check_deaths`":

```
7. `process_world` — fire/water/grass evolve (every PROCESS_EVERY_N ticks,
   rules in process_rules.rs; glow recomputed from base + LIT feats)
```

(renumber subsequent 7→8, 8→9, 9→10.)

rule 7 append:

```
   World processes follow the same law: fire consumes fuel and its ash rate
   is < 1 block per wood block; deep water produces slower (2%) than shallow
   water thins out (8%); trees never self-replicate (plant only).
```

`.claude/skills/ask-sandbox/SKILL.md` verbs table append:

```
| `use` | use pack block (slot): ignite flammable (wood/tree block) / eat organic (grass) |
```

and append a note in the Contract section:

```
8. The world moves on its own: fire spreads to plants/doors and dies out;
   water flows and thins; grass spreads near water. Fire hurts like lava —
   don't stand in it.
```

- [ ] **Step 4: run full suite + zero warnings**

`cargo test -p ask-kernel 2>&1 | grep -E 'test result|warning'` → green, no warnings.

- [ ] **Step 5: commit**

```bash
git add crates/ask-kernel/tests/sim_rules.rs docs/ARCHITECTURE.md .claude/skills/ask-sandbox/SKILL.md
git commit -m "test(process): behavior suite (fire/water/grass/glow) + docs

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Parallel dispatch（并行执行编排）

文件不相交验证：
- T1: `data/f_info.txt` + `f_info.rs` + `f_info_contract.rs`
- T2: `process_rules.rs`（新）+ `lib.rs`
- T3: `events.rs` + `static/render.js`
- T4: `systems/process.rs`（新）+ `systems/mod.rs` + `tick.rs` + `vision.rs` + `balance.rs` + `sim_rules.rs`
- T5: `systems/use_item.rs`（新）+ `verbs.rs` + `interact.rs` + `systems/mod.rs` + `sim_rules.rs`
- T6: `sim_rules.rs` + docs

冲突点：`systems/mod.rs`(T4/T5)、`sim_rules.rs`(T4/T5/T6)、`lib.rs`(T2/T4? 不——T4 不动 lib.rs)。处理：**wave 1 并行 T1+T2+T3**（零交集）；**wave 2 并行 T4+T5**（共享 systems/mod.rs 与 sim_rules.rs——两文件都是 append-only 区域，提示词里声明"只做 append/新增 mod 行，不重排既有内容"，冲突可控；若想零风险则 T4→T5 串行）；**wave 3：T6**（需 T1-T5 全部落地）。评审与实现并行：每个 wave 完成后并行派 reviewer。

## Self-Review

1. **Spec coverage:** spec §4 七行规则→T2；FIRE feat→T1；事件→T3；引擎+glow→T4；use→T5；§9 测试→T1/T4/T5/T6；文件结构→各任务 Files 一致。✅
2. **Placeholder scan:** 无 TBD；两处显式注明需实现者核对字段名（FeatInfo.walk/lava、lit flag 解析）——这是有意的前置声明，实现时以编译器为准。✅
3. **Type consistency:** `ProcessAction::NeighborAndSelf { neighbor, self_becomes }`（T2 定义/T4 使用）一致；`Cause` serde snake_case 与事件 payload 一致；`process_world` 命名与 tick/mod 引用一致；`recompute_glow` 与 vision.rs 定义一致。✅
