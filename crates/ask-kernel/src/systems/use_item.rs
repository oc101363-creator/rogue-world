//! use verb — ONE verb, effects dispatched by block flags (never item ids).

use bevy_ecs::prelude::*;

use crate::balance;
use crate::components::{Health, Inventory, Matter, ResourceKind};
use crate::events::{EventBuf, GameEvent};
use crate::f_info::{self, id};
use crate::grid::Grid;
use crate::systems::stable_id;

pub fn apply_use(world: &mut World, agent: Entity, slot: Option<usize>, dx: i32, dy: i32) {
    let eid = stable_id(world, agent);
    let found = slot.and_then(|i| {
        world
            .get::<Inventory>(agent)
            .and_then(|inv| inv.slots.get(i).map(|s| (i, s.matter.clone())))
    });
    let Some((slot_i, matter)) = found else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "bad_slot".into(),
            });
        return;
    };

    let flammable = match &matter {
        Matter::Terrain { feat } => *feat == id::TREE,
        Matter::Resource { resource } => *resource == ResourceKind::Wood,
        _ => false,
    };
    let organic = match &matter {
        Matter::Terrain { feat } => *feat == id::GRASS || *feat == id::BRAKE,
        _ => false,
    };

    if flammable {
        use_ignite(world, agent, eid, slot_i, dx, dy);
        return;
    }
    if organic {
        use_eat(world, agent, eid, slot_i, matter);
        return;
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
    let prior = world.resource::<Grid>().get(tx, ty).unwrap_or(0);
    world.resource_mut::<Grid>().set(tx, ty, id::FIRE);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::TerrainChanged {
            at: (tx, ty),
            from: prior,
            to: id::FIRE,
            cause: crate::process_rules::Cause::Fire,
        });
}

fn use_eat(world: &mut World, agent: Entity, eid: u64, slot_i: usize, matter: Matter) {
    // confirm the body exists before the block leaves the pack
    if world.get::<Health>(agent).is_none() {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "no_health".into(),
            });
        return;
    }
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
    world.resource_mut::<EventBuf>().push(GameEvent::Consumed {
        entity: eid,
        label,
        hp: hp_now,
    });
}
