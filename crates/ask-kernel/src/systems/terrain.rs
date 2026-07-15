//! Terrain interactions — frog-like enter-cell / door / stairs effects.

use bevy_ecs::prelude::*;

use crate::components::{Health, Position};
use crate::events::{EventBuf, GameEvent};
use crate::f_info::{self, id};
use crate::grid::Grid;
use crate::systems::stable_id;
use crate::world::{Depth, WorldSeed};

/// After a successful move onto (x,y): traps, lava, deep water.
pub fn on_enter_cell(world: &mut World, entity: Entity, x: i32, y: i32) {
    let Some(feat) = world.resource::<Grid>().get(x, y) else {
        return;
    };
    let table = f_info::table();
    let info = table.get(feat);
    let eid = stable_id(world, entity);

    // --- traps (HIT_TRAP) ---
    if table.is_trap(feat) {
        let name = info.map(|f| f.name.clone()).unwrap_or_else(|| "trap".into());
        let damage = trap_damage(feat);
        if let Some(mut hp) = world.get_mut::<Health>(entity) {
            hp.damage(damage);
        }
        // clear trap → floor (frog often leaves mimic; we simplify to floor)
        world.resource_mut::<Grid>().set(x, y, id::FLOOR);
        let hp_now = world.get::<Health>(entity).map(|h| h.hp).unwrap_or(0);
        world.resource_mut::<EventBuf>().push(GameEvent::TrapTriggered {
            entity: eid,
            feat,
            name,
            damage,
            at: (x, y),
        });
        world.resource_mut::<EventBuf>().push(GameEvent::TerrainDamage {
            entity: eid,
            kind: "trap".into(),
            damage,
            hp: hp_now,
        });
        return;
    }

    // --- lava ---
    if info.map(|f| f.lava).unwrap_or(false) {
        let damage = if feat == id::DEEP_LAVA { 6 } else { 3 };
        if let Some(mut hp) = world.get_mut::<Health>(entity) {
            hp.damage(damage);
        }
        let hp_now = world.get::<Health>(entity).map(|h| h.hp).unwrap_or(0);
        world.resource_mut::<EventBuf>().push(GameEvent::TerrainDamage {
            entity: eid,
            kind: "lava".into(),
            damage,
            hp: hp_now,
        });
        return;
    }

    // --- deep water: mild damage (fatigue) ---
    if feat == id::DEEP_WATER {
        let damage = 1;
        if let Some(mut hp) = world.get_mut::<Health>(entity) {
            hp.damage(damage);
        }
        let hp_now = world.get::<Health>(entity).map(|h| h.hp).unwrap_or(0);
        world.resource_mut::<EventBuf>().push(GameEvent::TerrainDamage {
            entity: eid,
            kind: "deep_water".into(),
            damage,
            hp: hp_now,
        });
    }
}

fn trap_damage(feat: u16) -> i32 {
    match feat {
        id::TRAP_TRAPDOOR => 4,
        id::TRAP_PIT | id::TRAP_SPIKED_PIT => 3,
        id::TRAP_POISON_PIT | id::TRAP_POISON => 2,
        id::TRAP_FIRE | id::TRAP_ACID => 4,
        id::TRAP_TY_CURSE => 5,
        _ => 2,
    }
}

pub fn apply_open_door(world: &mut World, entity: Entity, dx: i32, dy: i32) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    let eid = stable_id(world, entity);
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let Some(feat) = world.resource::<Grid>().get(tx, ty) else {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: eid,
            reason: "oob".into(),
        });
        return;
    };
    if !f_info::table().is_closed_door(feat) {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: eid,
            reason: "not_closed_door".into(),
        });
        return;
    }
    world.resource_mut::<Grid>().set(tx, ty, id::OPEN_DOOR);
    world.resource_mut::<EventBuf>().push(GameEvent::DoorOpened {
        entity: eid,
        at: (tx, ty),
    });
}

pub fn apply_close_door(world: &mut World, entity: Entity, dx: i32, dy: i32) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    let eid = stable_id(world, entity);
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let Some(feat) = world.resource::<Grid>().get(tx, ty) else {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: eid,
            reason: "oob".into(),
        });
        return;
    };
    if !f_info::table().is_open_door(feat) {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: eid,
            reason: "not_open_door".into(),
        });
        return;
    }
    // don't close on top of another agent
    {
        let mut q = world.query::<&Position>();
        for p in q.iter(world) {
            if p.x == tx && p.y == ty {
                // self is ok if underfoot; other blockers reject
            }
        }
    }
    world.resource_mut::<Grid>().set(tx, ty, id::CLOSED_DOOR);
    world.resource_mut::<EventBuf>().push(GameEvent::DoorClosed {
        entity: eid,
        at: (tx, ty),
    });
}

/// Use stairs on current cell — regenerates a new level (frog depth change spirit).
pub fn apply_use_stairs(world: &mut World, entity: Entity, down: bool) {
    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    let eid = stable_id(world, entity);
    let Some(feat) = world.resource::<Grid>().get(pos.x, pos.y) else {
        return;
    };
    let table = f_info::table();
    let info = table.get(feat);
    let ok = if down {
        info.map(|f| f.more || f.stairs).unwrap_or(false)
            && (feat == id::DOWN_STAIR || info.map(|f| f.more).unwrap_or(false))
    } else {
        feat == id::UP_STAIR || info.map(|f| f.less).unwrap_or(false)
    };
    if !ok {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: eid,
            reason: if down {
                "not_down_stairs".into()
            } else {
                "not_up_stairs".into()
            },
        });
        return;
    }

    // depth + seed advance
    let depth = {
        let mut d = world.resource_mut::<Depth>();
        if down {
            d.0 = d.0.saturating_add(1);
        } else {
            d.0 = d.0.saturating_sub(1);
        }
        d.0
    };
    let seed = {
        let mut s = world.resource_mut::<WorldSeed>();
        s.0 = s.0.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_add(depth as u64);
        s.0
    };

    world.resource_mut::<EventBuf>().push(GameEvent::LevelChanged {
        entity: eid,
        down,
        depth,
        seed,
    });

    // Signal tick loop to rebuild (see tick.rs)
    world.insert_resource(PendingLevelChange { seed, depth });
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct PendingLevelChange {
    pub seed: u64,
    pub depth: u32,
}
