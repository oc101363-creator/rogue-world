use bevy_ecs::prelude::*;

use crate::components::{Building, Glyph, Inventory, Position, ResourceKind, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::grid::Grid;
use crate::systems::stable_id;
use crate::world::KernelConfig;

pub fn apply_build_hut(world: &mut World, agent: Entity) {
    let cost = world.resource::<KernelConfig>().hut_wood_cost;
    let Some(apos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let agent_id = stable_id(world, agent);

    if !world.resource::<Grid>().buildable(apos.x, apos.y) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::BuildFailed {
                entity: agent_id,
                reason: "not_buildable".into(),
            });
        return;
    }

    if crate::spatial::any_at(world, apos.x, apos.y, |w, e| w.get::<Building>(e).is_some()) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::BuildFailed {
                entity: agent_id,
                reason: "occupied".into(),
            });
        return;
    }

    let wood = world.get::<Inventory>(agent).map(|i| i.wood()).unwrap_or(0);
    if wood < cost {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::BuildFailed {
                entity: agent_id,
                reason: "not_enough_wood".into(),
            });
        return;
    }

    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        let _ = inv.remove_resource(ResourceKind::Wood, cost);
    }

    let id = crate::world::next_id(world);
    world.spawn((
        Position {
            x: apos.x,
            y: apos.y,
        },
        Glyph('H'),
        Building,
        StableId(id),
    ));

    world.resource_mut::<EventBuf>().push(GameEvent::Built {
        builder: agent_id,
        at: (apos.x, apos.y),
    });
}
