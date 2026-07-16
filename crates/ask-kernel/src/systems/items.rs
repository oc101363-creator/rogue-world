//! Pickup — manual (verb) and auto (same cell after move). ONE body of logic.

use bevy_ecs::prelude::*;

use crate::components::{Agent, Inventory, Item, Position};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

/// Pick up every ground item underfoot. Returns how many stacks moved.
/// THE one pickup implementation (verb handler and auto-pickup both call it).
pub fn pickup_at(world: &mut World, agent_e: Entity, apos: Position) -> usize {
    let picks: Vec<(Entity, String, crate::components::Matter, u32)> =
        crate::spatial::at(world, apos.x, apos.y)
            .into_iter()
            .filter(|&e| e != agent_e)
            .filter_map(|e| {
                world
                    .get::<Item>(e)
                    .map(|it| (e, it.name(), it.matter.clone(), it.qty))
            })
            .collect();
    let n = picks.len();
    for (item_e, name, matter, qty) in picks {
        if let Some(mut inv) = world.get_mut::<Inventory>(agent_e) {
            inv.add(matter, qty);
        }
        let aid = stable_id(world, agent_e);
        let iid = stable_id(world, item_e);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ItemPickedUp {
                entity: aid,
                item: iid,
                name,
                at: (apos.x, apos.y),
            });
        world.despawn(item_e);
    }
    n
}

pub fn pickup_items_system(world: &mut World) {
    let agents: Vec<(Entity, Position)> = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
        q.iter(world).map(|(e, p)| (e, *p)).collect()
    };
    for (agent_e, apos) in agents {
        pickup_at(world, agent_e, apos);
    }
}
