use bevy_ecs::prelude::*;

use crate::components::{Inventory, Position, Resource, ResourceKind};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

pub fn apply_harvest(world: &mut World, agent: Entity) {
    let Some(apos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let agent_id = stable_id(world, agent);

    // Find resource on same cell
    let mut target: Option<(Entity, ResourceKind, u32)> = None;
    {
        let mut q = world.query::<(Entity, &Position, &Resource)>();
        for (e, p, r) in q.iter(world) {
            if p.x == apos.x && p.y == apos.y && r.amount > 0 {
                target = Some((e, r.kind, r.amount));
                break;
            }
        }
    }

    let Some((res_e, kind, amount)) = target else {
        world.resource_mut::<EventBuf>().push(GameEvent::ActionRejected {
            entity: agent_id,
            reason: "no_resource_here".into(),
        });
        return;
    };

    // Decrement resource
    let depleted;
    {
        let mut r = world.get_mut::<Resource>(res_e).unwrap();
        r.amount -= 1;
        depleted = r.amount == 0;
    }

    // Add to inventory
    let (wood, iron) = {
        let mut inv = world.get_mut::<Inventory>(agent).unwrap();
        match kind {
            ResourceKind::Wood => inv.wood += 1,
            ResourceKind::Iron => inv.iron += 1,
        }
        (inv.wood, inv.iron)
    };

    let kind_s = match kind {
        ResourceKind::Wood => "wood",
        ResourceKind::Iron => "iron",
    };
    world.resource_mut::<EventBuf>().push(GameEvent::Harvested {
        entity: agent_id,
        kind: kind_s.into(),
        amount: 1,
        inventory_wood: wood,
        inventory_iron: iron,
    });

    if depleted {
        let rid = stable_id(world, res_e);
        world.despawn(res_e);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ResourceDepleted { entity: rid });
        let _ = amount; // silence
    }
}
