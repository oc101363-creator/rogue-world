pub mod build;
pub mod harvest;
pub mod movement;
pub mod terrain;

use bevy_ecs::prelude::*;

use crate::actions::{Action, ActionQueue};
use crate::components::StableId;
use crate::events::{EventBuf, GameEvent};
use crate::world::TickCounter;

use self::build::apply_build_hut;
use self::harvest::apply_harvest;
use self::movement::apply_move;
use self::terrain::{apply_close_door, apply_open_door, apply_use_stairs};

/// Apply all queued actions (sorted by entity index for determinism).
pub fn apply_actions_system(world: &mut World) {
    let mut items = {
        let mut q = world.resource_mut::<ActionQueue>();
        let mut items = std::mem::take(&mut q.items);
        items.sort_by_key(|a| a.entity.to_bits());
        items
    };

    for item in items.drain(..) {
        if world.get_entity(item.entity).is_err() {
            continue;
        }
        match item.action {
            Action::Move { dx, dy } => apply_move(world, item.entity, dx, dy),
            Action::Harvest => apply_harvest(world, item.entity),
            Action::BuildHut => apply_build_hut(world, item.entity),
            Action::OpenDoor { dx, dy } => apply_open_door(world, item.entity, dx, dy),
            Action::CloseDoor { dx, dy } => apply_close_door(world, item.entity, dx, dy),
            Action::UseStairs { down } => apply_use_stairs(world, item.entity, down),
            Action::Idle => {}
        }
    }
}

pub fn begin_tick_system(world: &mut World) {
    let tick = world.resource::<TickCounter>().0;
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::TickStarted { tick });
    world.resource_mut::<ActionQueue>().clear();
}

pub fn advance_tick_system(world: &mut World) {
    world.resource_mut::<TickCounter>().0 += 1;
}

pub fn stable_id(world: &World, entity: Entity) -> u64 {
    world
        .get::<StableId>(entity)
        .map(|s| s.0)
        .unwrap_or(entity.to_bits())
}
