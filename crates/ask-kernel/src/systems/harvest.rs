use bevy_ecs::prelude::*;

use crate::components::{Inventory, Matter, Position, Resource, ResourceKind};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

pub fn apply_harvest(world: &mut World, agent: Entity) {
    let Some(apos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let agent_id = stable_id(world, agent);

    let target = crate::spatial::find_at(world, apos.x, apos.y, |w, e| {
        w.get::<Resource>(e).map(|r| r.amount > 0).unwrap_or(false)
    })
    .map(|e| (e, world.get::<Resource>(e).unwrap().kind));

    let Some((res_e, kind)) = target else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: agent_id,
                reason: "no_resource_here".into(),
            });
        return;
    };

    let depleted;
    {
        let mut r = world.get_mut::<Resource>(res_e).unwrap();
        r.amount -= 1;
        depleted = r.amount == 0;
    }

    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        inv.add(Matter::Resource { resource: kind }, 1);
    }
    let (wood, iron) = world
        .get::<Inventory>(agent)
        .map(|i| (i.wood(), i.iron()))
        .unwrap_or((0, 0));

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
    }
}
