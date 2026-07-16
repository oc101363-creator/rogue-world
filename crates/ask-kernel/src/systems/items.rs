//! Auto-pickup when agent shares a cell with an Item.

use bevy_ecs::prelude::*;

use crate::components::{Agent, Inventory, Item, Position};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

pub fn pickup_items_system(world: &mut World) {
    let agents: Vec<(Entity, Position)> = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
        q.iter(world).map(|(e, p)| (e, *p)).collect()
    };

    for (agent_e, apos) in agents {
        let picks: Vec<(Entity, String, crate::components::Matter, u32)> = crate::spatial::at(
            world, apos.x, apos.y,
        )
        .into_iter()
        .filter_map(|e| {
            world
                .get::<Item>(e)
                .map(|it| (e, it.name(), it.matter.clone(), it.qty))
        })
        .collect();
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
    }
}
