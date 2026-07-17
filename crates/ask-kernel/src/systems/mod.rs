pub mod build;
pub mod combat;
pub mod craft;
pub mod death;
pub mod dig;
pub mod harvest;
pub mod interact;
pub mod inventory_act;
pub mod items;
pub mod monster;
pub mod movement;
pub mod terrain;
pub mod verbs;

use bevy_ecs::prelude::*;

use crate::actions::{Action, ActionQueue};
use crate::components::StableId;
use crate::events::{EventBuf, GameEvent};
use crate::world::TickCounter;

use self::interact::apply_interact;
use self::inventory_act::{apply_drop_item, apply_rest};
use self::movement::apply_move;

pub use self::death::check_deaths_system as check_deaths;
pub use self::items::pickup_items_system as pickup_items;
pub use self::monster::monster_move_to;
pub use self::monster::process_monsters_system as process_monsters;

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
            Action::Interact {
                dx,
                dy,
                verb,
                slot,
                recipe,
            } => apply_interact(world, item.entity, dx, dy, verb, slot, recipe),
            Action::Drop { index } => apply_drop_item(world, item.entity, index),
            Action::Rest => apply_rest(world, item.entity),
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

/// Greedy 4-way unit step toward a target (shared by mock policy & monsters).
pub fn step_toward(x: i32, y: i32, tx: i32, ty: i32) -> (i32, i32) {
    let dx = tx - x;
    let dy = ty - y;
    if dx.abs() >= dy.abs() && dx != 0 {
        (dx.signum(), 0)
    } else if dy != 0 {
        (0, dy.signum())
    } else {
        (0, 0)
    }
}
