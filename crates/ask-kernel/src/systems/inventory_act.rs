//! Drop / pickup / rest — pack as Matter stacks.

use bevy_ecs::prelude::*;

use crate::components::{Glyph, Inventory, Item, Position, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

pub fn apply_drop_item(world: &mut World, agent: Entity, index: usize) {
    let eid = stable_id(world, agent);
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };

    let matter = {
        let Some(mut inv) = world.get_mut::<Inventory>(agent) else {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "no_inventory".into(),
                });
            return;
        };
        match inv.take_one(index) {
            Some(m) => m,
            None => {
                world
                    .resource_mut::<EventBuf>()
                    .push(GameEvent::ActionRejected {
                        entity: eid,
                        reason: "bad_item_index".into(),
                    });
                return;
            }
        }
    };

    let id = crate::world::next_id(world);
    let glyph = matter.glyph();
    let name = matter.label();
    world.spawn((
        Position { x: pos.x, y: pos.y },
        Glyph(glyph),
        Item { matter, qty: 1 },
        StableId(id),
    ));

    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::ItemDropped {
            entity: eid,
            item: id,
            name,
            at: (pos.x, pos.y),
        });
}

pub fn apply_pickup(world: &mut World, agent: Entity) {
    let Some(apos) = world.get::<Position>(agent).copied() else {
        return;
    };
    if crate::systems::items::pickup_at(world, agent, apos) == 0 {
        let eid = stable_id(world, agent);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "nothing_to_pickup".into(),
            });
    }
}

pub fn apply_rest(world: &mut World, agent: Entity) {
    let eid = stable_id(world, agent);
    let (healed, hp, max_hp) = {
        let Some(mut h) = world.get_mut::<crate::components::Health>(agent) else {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "no_health".into(),
                });
            return;
        };
        if h.hp >= h.max_hp {
            (0, h.hp, h.max_hp)
        } else {
            h.hp = (h.hp + crate::balance::REST_HEAL).min(h.max_hp);
            (crate::balance::REST_HEAL, h.hp, h.max_hp)
        }
    };
    world.resource_mut::<EventBuf>().push(GameEvent::Rested {
        entity: eid,
        healed,
        hp,
        max_hp,
    });
}
