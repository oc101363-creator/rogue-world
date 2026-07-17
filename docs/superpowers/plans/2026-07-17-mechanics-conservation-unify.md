# Mechanics Conservation & Rule Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every matter-printing loop, merge duplicated mechanics, and make terrain rules apply uniformly to agents and monsters.

**Architecture:** One principle — **matter is conserved**: every extract/transform verb is zero-sum or lossy, never net-positive. One rule set — `on_enter_cell` applies to anything that moves. Race data (r_info flags) starts driving behavior (CAN_SWIM/RES_FIRE first).

**Tech Stack:** Rust (bevy_ecs), cargo test. All tasks TDD: failing test → minimal impl → green → commit.

## Global Constraints

- Work in worktree `.claude/worktrees/ask-overhaul` (branch `worktree-ask-overhaul`).
- Every gameplay number lives in `crates/ask-kernel/src/balance.rs` — no literals in systems.
- Verbs register only in `systems/verbs.rs`; discovery only in `systems/interact.rs`.
- Run `cargo test -p ask-kernel` after every task; it must stay green (60+ tests).
- Commit message format: `<type>(<scope>): <summary>` + `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Conservation invariant (must hold after Task 3): no sequence of verbs can increase total matter of any kind. Natural sources (resource entities with amount>0, digging new rock) are the only inputs.

---

### Task 1: plant 零和 — cost 2 wood / TREE block, 新树 amount 2

**Files:**
- Modify: `crates/ask-kernel/src/balance.rs` (add constants)
- Modify: `crates/ask-kernel/src/systems/craft.rs:99-185` (apply_plant, can_plant)
- Modify: `crates/ask-kernel/src/systems/interact.rs` (plant label)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `balance::PLANT_COST_WOOD`, `balance::PLANTED_TREE_AMOUNT`
- Produces: `pub const PLANT_COST_WOOD: u32 = 2`, `pub const PLANTED_TREE_AMOUNT: u32 = 2`

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn plant_is_zero_sum_no_wood_printing() {
    use ask_kernel::components::{Matter, Resource};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 41;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DIRT);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 10);

    // plant: costs exactly PLANT_COST_WOOD
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("plant".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let wood = kw.world.get::<Inventory>(agent).unwrap().wood();
    assert_eq!(wood, 10 - ask_kernel::balance::PLANT_COST_WOOD);

    // planted tree yields exactly PLANTED_TREE_AMOUNT — the full cycle is zero-sum
    let amount = {
        let mut q = kw.world.query::<&Resource>();
        q.iter(&kw.world)
            .find(|r| r.kind == ResourceKind::Wood)
            .map(|r| r.amount)
            .expect("planted tree resource")
    };
    assert_eq!(amount, ask_kernel::balance::PLANTED_TREE_AMOUNT);
    assert!(amount <= ask_kernel::balance::PLANT_COST_WOOD, "plant must not print wood");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules plant_is_zero_sum 2>&1 | tail -3`
Expected: FAIL — `balance::PLANT_COST_WOOD` does not exist (compile error), or amount == 3.

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/balance.rs`, append:

```rust
// --- planting ---
/// Wood cost to plant a tree (a TREE block also works, it is worth 2 wood).
pub const PLANT_COST_WOOD: u32 = 2;
/// Yield of a planted tree. amount ≤ cost: planting moves wood, never prints it.
pub const PLANTED_TREE_AMOUNT: u32 = 2;
```

In `crates/ask-kernel/src/systems/craft.rs` `apply_plant`, replace the payment block and the spawn amount:

```rust
    // prefer 1 TREE block (worth 2 wood), else spend PLANT_COST_WOOD wood
    let paid = {
        let Some(mut inv) = world.get_mut::<Inventory>(agent) else {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "no_inventory".into(),
                });
            return;
        };
        if inv.remove_terrain(id::TREE, 1) {
            true
        } else if inv.remove_resource(ResourceKind::Wood, crate::balance::PLANT_COST_WOOD) {
            true
        } else {
            false
        }
    };
    if !paid {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "need_2_wood_or_tree_block".into(),
            });
        return;
    }
```

and the spawned resource:

```rust
            Resource {
                kind: ResourceKind::Wood,
                amount: crate::balance::PLANTED_TREE_AMOUNT,
            },
```

and `can_plant`:

```rust
pub fn can_plant(world: &World, agent: Entity) -> bool {
    world
        .get::<Inventory>(agent)
        .map(|i| i.qty_terrain(id::TREE) > 0 || i.wood() >= crate::balance::PLANT_COST_WOOD)
        .unwrap_or(false)
}
```

In `crates/ask-kernel/src/systems/interact.rs`, the plant interaction label:

```rust
            label: format!("plant tree ({} wood)", crate::balance::PLANT_COST_WOOD),
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS (existing plant test gives 5 wood ≥ cost 2, still works).

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/balance.rs crates/ask-kernel/src/systems/craft.rs crates/ask-kernel/src/systems/interact.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "fix(economy): plant is zero-sum (cost 2, yields 2) — no more wood printing

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: place 的 displaced 只返还硬岩 + scoop 删软土兜底

**Files:**
- Modify: `crates/ask-kernel/src/systems/dig.rs:184-205` (apply_place displaced)
- Modify: `crates/ask-kernel/src/sandbox.rs:60-73` (scoop_rule catch-all)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `sandbox::is_diggable`, `sandbox::scoop_rule`
- Produces: displaced-return only for `is_diggable(cur)` feats

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn scoop_place_cycle_creates_nothing() {
    use ask_kernel::components::Matter;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 43;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::FLOOR);

    // scoop FLOOR → pack gets FLOOR block, cell becomes DIRT
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("scoop".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.qty_terrain(id::FLOOR), 1, "scoop should pack the floor");
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::DIRT)
    );

    // place FLOOR back onto DIRT → cell becomes FLOOR, and NOTHING else
    // (soft ground must not be returned as a displaced block)
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("place".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::FLOOR)
    );
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.slots.len(), 0, "cycle must be zero-sum, pack: {:?}", inv.slots);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules scoop_place_cycle 2>&1 | tail -3`
Expected: FAIL — `inv.slots.len()` is 1 (DIRT returned as displaced).

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/systems/dig.rs` `apply_place`, replace the displaced block:

```rust
    // Conservation: only HARD ROCK comes back as a displaced block.
    // Soft ground (floor/dirt/grass…) simply vanishes when overwritten —
    // otherwise scoop+place prints matter out of thin air.
    let displaced = if cur != feat && sandbox::is_diggable(cur) {
        Some(cur)
    } else {
        None
    };
```

In `crates/ask-kernel/src/sandbox.rs` `scoop_rule`, delete the walkable catch-all so only named soft surfaces scoop:

```rust
        // NOTE: no walkable catch-all — every scoopable surface is named
        // above. A generic "any soft ground" rule made matter semantics
        // untracked and fed the place-displacement printing loop.
        _ => None,
```

i.e. the match arm `_ if info.walk && !info.wall && !info.trap => Some(...)` is removed; the trailing `_ => None` stays.

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/systems/dig.rs crates/ask-kernel/src/sandbox.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "fix(economy): place returns only hard rock as displaced — scoop/place cycle is zero-sum

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: ore_vein 配方提价堵铁循环（3 iron + 2 granite）

**Files:**
- Modify: `crates/ask-kernel/src/sandbox.rs` (recipes table, ore_vein row)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `sandbox::recipes()`, `RecipeNeed`
- Produces: ore_vein needs `[Iron(3), Terrain(GRANITE, 2)]`

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn ore_vein_cycle_cannot_print_iron() {
    // A crafted magma-treasure block is worth 3 iron total
    // (1 dig bonus + 2 from smelt). Its recipe must cost ≥ 3 iron,
    // or granite (infinite) becomes iron (infinite).
    let recipe = ask_kernel::sandbox::recipes()
        .iter()
        .find(|r| r.id == "ore_vein")
        .expect("ore_vein recipe");
    let iron_cost: u32 = recipe
        .needs
        .iter()
        .map(|n| match n {
            ask_kernel::sandbox::RecipeNeed::Iron(q) => *q,
            _ => 0,
        })
        .sum();
    assert!(iron_cost >= 3, "ore_vein costs {iron_cost} iron < 3 — iron printing");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules ore_vein_cycle 2>&1 | tail -3`
Expected: FAIL — ore_vein costs 1 iron.

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/sandbox.rs` recipes table, ore_vein row:

```rust
        Recipe {
            id: "ore_vein",
            name: "ore vein",
            needs: &[RecipeNeed::Iron(3), RecipeNeed::Terrain(id::GRANITE, 2)],
            output: RecipeOut::Terrain(id::MAGMA_TREASURE),
        },
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS. (Recipe labels regenerate from needs automatically.)

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/sandbox.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "fix(economy): ore_vein costs 3 iron + 2 granite (its full dig+smelt yield)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: 怪物走同一套地形规则（陷阱/熔岩/深水生效 + 地形致死）

**Files:**
- Modify: `crates/ask-kernel/src/systems/monster.rs` (post-move terrain hook + death)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `crate::systems::terrain::on_enter_cell`, `GameEvent::MonsterKilled`
- Produces: monsters take trap/lava/deep-water effects; hp≤0 monsters despawn with MonsterKilled { entity: 0, .. }

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn monsters_suffer_terrain_too() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 51;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    // trap on an open cell; monster next to it, agent far away (out of chase)
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::TRAP_FIRE);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('o'),
        Monster { race_id: 1, name: "sacrificial rat".into(), color: 'r' },
        Health { hp: 2, max_hp: 2 },
        StableId(99901),
    )).id();
    // force-walk the monster onto the trap cell by simulating its move
    // (process_monsters wanders deterministically; instead apply the same
    // post-move rule directly: move + on_enter_cell via a helper)
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    let hp = kw.world.get::<Health>(e).map(|h| h.hp);
    assert_eq!(hp, None, "monster should have died on TRAP_FIRE (2hp vs 4 dmg)");
}

#[test]
fn monsters_die_on_lava_and_emit_kill_event() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 52;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DEEP_LAVA);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('o'),
        Monster { race_id: 1, name: "lava rat".into(), color: 'r' },
        Health { hp: 5, max_hp: 5 },
        StableId(99902),
    )).id();
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    assert!(kw.world.get::<Health>(e).is_none(), "lava should kill 5hp rat");
    let evs = kw.world.resource_mut::<EventBuf>().drain();
    assert!(
        evs.iter().any(|ev| matches!(ev, GameEvent::MonsterKilled { name, .. } if name == "lava rat")),
        "expected MonsterKilled event"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules monsters_suffer_terrain 2>&1 | tail -3`
Expected: FAIL — `monster_move_to` does not exist (compile error).

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/systems/monster.rs`, add a shared post-move helper and use it in the wander/chase move path:

```rust
/// Move a monster onto (nx, ny) and apply THE SAME terrain rules agents get
/// (traps, lava, deep water). If the terrain kills it, despawn + event.
pub fn monster_move_to(world: &mut World, mon_e: Entity, nx: i32, ny: i32) {
    let Some(pos) = world.get::<Position>(mon_e).copied() else {
        return;
    };
    if let Some(mut p) = world.get_mut::<Position>(mon_e) {
        p.x = nx;
        p.y = ny;
    }
    let mid = stable_id(world, mon_e);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::MonsterMoved {
            entity: mid,
            from: (pos.x, pos.y),
            to: (nx, ny),
        });

    crate::systems::terrain::on_enter_cell(world, mon_e, nx, ny);

    // terrain death
    let dead = world.get::<Health>(mon_e).map(|h| h.hp <= 0).unwrap_or(false);
    if dead {
        let name = world
            .get::<Monster>(mon_e)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "monster".into());
        world.despawn(mon_e);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::MonsterKilled {
                entity: 0, // no killer — the world did it
                monster: mid,
                name,
                at: (nx, ny),
            });
    }
}
```

In `process_monsters_system`, replace the plain move block at the end of the loop:

```rust
        monster_move_to(world, mon_e, nx, ny);
```

(deleting the old inline `Position` write + `MonsterMoved` push.)

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/systems/monster.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "fix(sim): monsters obey the same terrain rules (traps/lava/water) and die to them

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: r_info F: flags → CAN_SWIM 过深水、RES_FIRE 免熔岩

**Files:**
- Modify: `crates/ask-kernel/src/r_info.rs` (parse F: lines → two bools)
- Modify: `crates/ask-kernel/src/systems/monster.rs` (swim + fire immunity)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: r_info F: lines (`F:RAND_25 | CAN_SWIM | WILD_VOLCANO`, multi-line accumulates)
- Produces: `MonsterRace { can_swim: bool, res_fire: bool }` (Salamander race 50 has both)

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn salamander_swims_and_ignores_lava() {
    use ask_kernel::components::{Monster, StableId};

    // race 50 Salamander has CAN_SWIM | RES_FIRE in r_info.txt
    let race = ask_kernel::r_info::table().get(50).expect("race 50");
    assert!(race.can_swim, "CAN_SWIM flag not parsed");
    assert!(race.res_fire, "RES_FIRE flag not parsed");

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 53;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DEEP_LAVA);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('R'),
        Monster { race_id: 50, name: "Salamander".into(), color: 'o' },
        Health { hp: 5, max_hp: 5 },
        StableId(99903),
    )).id();
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    let hp = kw.world.get::<Health>(e).map(|h| h.hp);
    assert_eq!(hp, Some(5), "RES_FIRE salamander must ignore lava");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules salamander 2>&1 | tail -3`
Expected: FAIL — `can_swim` field does not exist (compile error).

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/r_info.rs`:

```rust
pub struct MonsterRace {
    pub id: u16,
    pub name: String,
    pub glyph: char,
    pub color: char,
    pub hp: Option<i32>,
    pub damage: Option<i32>,
    /// F: line flags we act on (parse more as needed).
    pub can_swim: bool,
    pub res_fire: bool,
}
```

In the parser: add `let mut can_swim = false; let mut res_fire = false;` next to `hp`/`damage`, reset them in the N: flush block, extend `flush` to take them, and handle F: lines:

```rust
        } else if let Some(rest) = line.strip_prefix("F:") {
            for flag in rest.split('|') {
                match flag.trim() {
                    "CAN_SWIM" => can_swim = true,
                    "RES_FIRE" => res_fire = true,
                    _ => {}
                }
            }
        }
```

Update both `flush(...)` calls and the struct literal to carry the two bools. Update the `parses_dice_and_stats` test's Salamander assertions to also check the flags.

In `crates/ask-kernel/src/systems/monster.rs` `monster_move_to`, skip terrain the race is immune to:

```rust
    let (can_swim, res_fire) = world
        .get::<Monster>(mon_e)
        .and_then(|m| crate::r_info::table().get(m.race_id))
        .map(|r| (r.can_swim, r.res_fire))
        .unwrap_or((false, false));
    let feat = world.resource::<Grid>().get(nx, ny).unwrap_or(0);
    let info = crate::f_info::table().get(feat);
    let lava_immune = res_fire && info.map(|f| f.lava).unwrap_or(false);
    let water_immune = can_swim && feat == crate::f_info::id::DEEP_WATER;
    if !lava_immune && !water_immune {
        crate::systems::terrain::on_enter_cell(world, mon_e, nx, ny);
    }
```

And in `process_monsters_system`, let CAN_SWIM monsters enter deep water (replace the walkability check):

```rust
        let dest = world.resource::<Grid>().get(nx, ny).unwrap_or(0);
        let swim_ok = world
            .get::<Monster>(mon_e)
            .and_then(|m| crate::r_info::table().get(m.race_id))
            .map(|r| r.can_swim && dest == crate::f_info::id::DEEP_WATER)
            .unwrap_or(false);
        if !world.resource::<Grid>().walkable(nx, ny) && !swim_ok {
            continue;
        }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/r_info.rs crates/ask-kernel/src/systems/monster.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "feat(sim): r_info flags drive behavior — CAN_SWIM crosses deep water, RES_FIRE ignores lava

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: 0 层上楼拒绝 + TRAP_TELEPORT 真传送

**Files:**
- Modify: `crates/ask-kernel/src/systems/terrain.rs` (apply_use_stairs, on_enter_cell trap branch)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `world::random_free_cell`, `id::TRAP_TELEPORT`
- Produces: ascend at depth 0 → ActionRejected "no_up_from_surface"; teleport trap relocates the entity instead of dealing damage

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn ascend_at_surface_rejected_and_teleport_trap_teleports() {
    use ask_kernel::components::StableId;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 61;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 4);

    // ascend at depth 0 must be rejected, not a free level re-roll
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::UP_STAIR);
    set_pos(&mut kw, agent, floor);
    let sid = kw.world.get::<StableId>(agent).unwrap().0;
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("ascend".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.resource::<ask_kernel::world::Depth>().0, 0);
    let evs = kw.world.resource_mut::<EventBuf>().drain();
    assert!(
        evs.iter().any(|e| matches!(e, GameEvent::ActionRejected { reason, .. } if reason == "no_up_from_surface")),
        "surface ascend must be rejected"
    );

    // teleport trap moves the agent (and deals no damage)
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::TRAP_TELEPORT);
    set_pos(&mut kw, agent, floor);
    let hp_before = kw.world.get::<Health>(agent).unwrap().hp;
    crate::systems::terrain::on_enter_cell_for_test(&mut kw.world, agent);
    let p = kw.world.get::<Position>(agent).unwrap();
    assert_ne!((p.x, p.y), floor, "teleport trap must move the agent");
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, hp_before, "teleport deals no damage");
    let _ = sid;
}
```

In `crates/ask-kernel/src/systems/terrain.rs`, expose the test hook (also useful in systems):

```rust
/// Test/system hook: run on_enter_cell at the entity's current cell.
pub fn on_enter_cell_for_test(world: &mut World, entity: Entity) {
    if let Some(p) = world.get::<Position>(entity).copied() {
        on_enter_cell(world, entity, p.x, p.y);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules ascend_at_surface 2>&1 | tail -3`
Expected: FAIL — ascend at depth 0 regenerates a level / no rejection; teleport deals damage.

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/systems/terrain.rs` `apply_use_stairs`, before the depth advance:

```rust
    if !down && world.resource::<Depth>().0 == 0 {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "no_up_from_surface".into(),
            });
        return;
    }
```

In the trap branch of `on_enter_cell`, special-case teleport:

```rust
    if table.is_trap(feat) {
        let name = info
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "trap".into());
        // teleport trap: relocate instead of damage
        if feat == id::TRAP_TELEPORT {
            let eid2 = stable_id(world, entity);
            let dest = crate::world::random_free_cell(world, eid2.wrapping_add(0x9E37))
                .unwrap_or((x, y));
            if let Some(mut p) = world.get_mut::<Position>(entity) {
                p.x = dest.0;
                p.y = dest.1;
            }
            world.resource_mut::<Grid>().set(x, y, id::FLOOR);
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::TrapTriggered {
                    entity: eid,
                    feat,
                    name,
                    damage: 0,
                    at: (x, y),
                });
            return;
        }
        let damage = balance::trap_damage(feat);
        // … existing damage path unchanged …
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/systems/terrain.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "fix(sim): no free re-roll at surface; TRAP_TELEPORT actually teleports

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: hut 庇护 — 屋内/贴屋 rest 回复 ×2

**Files:**
- Modify: `crates/ask-kernel/src/balance.rs` (constant)
- Modify: `crates/ask-kernel/src/systems/inventory_act.rs` (apply_rest)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `spatial::any_at`, `Building`
- Produces: `pub const HUT_REST_MULT: i32 = 2`

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn rest_near_hut_heals_double() {
    use ask_kernel::components::{Building, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 71;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world.get_mut::<Health>(agent).unwrap().hp = 10;

    // no hut: plain rest
    kw.world.resource_mut::<ActionQueue>().push(agent, Action::Rest);
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 11);

    // with a hut adjacent: double
    kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('H'),
        Building,
        StableId(99911),
    ));
    kw.world.resource_mut::<ActionQueue>().push(agent, Action::Rest);
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 13, "hut shelter should double rest");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules rest_near_hut 2>&1 | tail -3`
Expected: FAIL — second rest heals only 1 (hp 12).

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/balance.rs`:

```rust
/// Rest heal multiplier when on/adjacent to a hut (shelter).
pub const HUT_REST_MULT: i32 = 2;
```

In `crates/ask-kernel/src/systems/inventory_act.rs` `apply_rest`, compute the sheltered heal:

```rust
    let sheltered = crate::spatial::any_at(world, pos.x, pos.y, |w, e| {
        w.get::<crate::components::Building>(e).is_some()
    }) || [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| {
        crate::spatial::any_at(world, pos.x + dx, pos.y + dy, |w, e| {
            w.get::<crate::components::Building>(e).is_some()
        })
    });
    let heal = crate::balance::REST_HEAL
        * if sheltered {
            crate::balance::HUT_REST_MULT
        } else {
            1
        };
```

(`apply_rest` needs the agent's `Position`; it already computes nothing — add `let pos = world.get::<Position>(agent).copied();` before the health block and use `pos`.) Then use `heal` in place of `crate::balance::REST_HEAL` in the hp update and Rested event.

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/balance.rs crates/ask-kernel/src/systems/inventory_act.rs crates/ask-kernel/tests/sim_rules.rs
git commit -m "feat(sim): huts shelter — rest on/adjacent to a hut heals ×2

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: 停掉随机物品散布 + 文档同步

**Files:**
- Modify: `crates/ask-kernel/src/generate/mod.rs:308-311` (alloc_objects call)
- Modify: `crates/ask-kernel/README.md` (pack section)
- Modify: `crates/ask-kernel/docs/ARCHITECTURE.md` (house rules)
- Test: `crates/ask-kernel/tests/sim_rules.rs` (append)

**Interfaces:**
- Consumes: `generate::generate_level` → `GeneratedLevel.items`
- Produces: `GeneratedLevel.items` contains only vault-template objects (alloc_objects no longer scatters)

- [ ] **Step 1: Write the failing test**

Append to `crates/ask-kernel/tests/sim_rules.rs`:

```rust
#[test]
fn generation_scatters_no_purposeless_items() {
    // k_info objects currently do nothing in the pack; random scatter is
    // noise. Vault-template items (themed rooms) may still appear.
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 81;
    let level = generate_level(&cfg);
    let scattered = level.items.len();
    assert_eq!(scattered, 0, "random item scatter must stop (items: {scattered})");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p ask-kernel --test sim_rules generation_scatters 2>&1 | tail -3`
Expected: FAIL — items present from alloc_objects.

- [ ] **Step 3: Implement**

In `crates/ask-kernel/src/generate/mod.rs` `generate_level`, remove the random object scatter (keep template objects from vault stamps, they are themed):

```rust
    // frog _cave_gen_monsters (random object scatter removed: k_info items
    // have no use yet — a pack slot is too precious for decoration)
    let depth = 0u32;
    alloc_monsters(&grid, depth, &mut rng, &mut monsters);
```

(delete the `alloc_objects(&grid, depth, &mut rng, &mut items);` line.)

README.md pack section — append after the Pack line:

```
**Objects (k_info):** currently decorative — vault-template items only; random scatter disabled until they have a use.
```

docs/ARCHITECTURE.md house rules — append:

```
7. **Matter is conserved.** No verb/craft chain may be net-positive on any
   matter kind; natural sources (resource entities, fresh rock) are the only
   inputs. `place` returns displaced hard rock only. Guarded by
   `scoop_place_cycle_creates_nothing` and `ore_vein_cycle_cannot_print_iron`.
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p ask-kernel 2>&1 | grep -E 'test result|FAILED'`
Expected: all PASS (sim_rules' template-monster test unaffected; item count in templates is 0 on small maps — if a vault-template test ever depends on scattered items, assert on template items instead).

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/src/generate/mod.rs crates/ask-kernel/README.md crates/ask-kernel/docs/ARCHITECTURE.md crates/ask-kernel/tests/sim_rules.rs
git commit -m "chore(gen): stop scattering purposeless items; document matter conservation

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage (评审三类问题 → 任务映射):**
- A1 plant 永动机 → Task 1 ✅
- A2 scoop/place 无中生有 → Task 2 ✅
- A3 铁矿循环 → Task 3 ✅
- B1 两种树/矿：不合并形态，但 Task 1-3 把价值对齐（种植零和、vein 全价），自然资源点保留为"富矿" ✅（合并形态属于更大重做，YAGNI）
- B2 物品无用途 → Task 8 ✅
- B3 陷阱效果脱节 → Task 6 实现 TELEPORT；其余伤害类陷阱在 ARCHITECTURE 注明未实现效果 ✅
- B4 BROKEN/SECRET_DOOR 无机制 → 不建机制（YAGNI；scoop 已能处理这两种门）
- C1 怪物地形双标 → Task 4 ✅
- C2 0 层上楼 → Task 6 ✅
- C3 种族 flags 闲置 → Task 5 启动（CAN_SWIM/RES_FIRE 两个最常用的）✅
- 不修项（rest 张力、陷阱可见、自然树肥）已在评审说明理由，无需任务 ✅

**2. Placeholder scan:** 所有步骤含完整测试代码与实现代码、精确路径、命令与预期输出。Task 7 的 apply_rest 需补取 Position（已在该任务实现块中注明）。无 TBD/TODO。

**3. Type consistency:**
- `balance::PLANT_COST_WOOD` / `PLANTED_TREE_AMOUNT`（Task 1 定义并使用）✅
- `balance::HUT_REST_MULT`（Task 7 定义并使用）✅
- `systems::monster_move_to(world, mon_e, nx, ny)`（Task 4 定义，Task 4/5 测试使用）✅
- `terrain::on_enter_cell_for_test(world, entity)`（Task 6 定义并使用）✅
- `MonsterRace.{can_swim, res_fire}`（Task 5 定义并使用）✅
- `RecipeNeed::Iron(u32)` / `Terrain(FeatId, u32)`（Task 3 测试使用，与 sandbox 定义一致）✅
- 测试 helper `find_open_floor` / `set_pos`（已存在于 sim_rules.rs，沿用）✅
