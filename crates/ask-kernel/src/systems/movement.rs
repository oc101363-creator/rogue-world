use bevy_ecs::prelude::*;

use crate::components::Position;
use crate::events::{EventBuf, GameEvent};
use crate::grid::Grid;
use crate::systems::stable_id;

pub fn apply_move(world: &mut World, entity: Entity, dx: i32, dy: i32) {
    if dx.abs() + dy.abs() != 1 || dx.abs() > 1 || dy.abs() > 1 {
        let id = stable_id(world, entity);
        world.resource_mut::<EventBuf>().push(GameEvent::MoveFailed {
            entity: id,
            reason: "not_four_way".into(),
        });
        return;
    }

    let Some(pos) = world.get::<Position>(entity).copied() else {
        return;
    };
    let nx = pos.x + dx;
    let ny = pos.y + dy;
    let walkable = world.resource::<Grid>().walkable(nx, ny);
    let id = stable_id(world, entity);

    if !walkable {
        world.resource_mut::<EventBuf>().push(GameEvent::MoveFailed {
            entity: id,
            reason: "blocked".into(),
        });
        return;
    }

    if let Some(mut p) = world.get_mut::<Position>(entity) {
        p.x = nx;
        p.y = ny;
    }
    world.resource_mut::<EventBuf>().push(GameEvent::Moved {
        entity: id,
        from: (pos.x, pos.y),
        to: (nx, ny),
    });
}
