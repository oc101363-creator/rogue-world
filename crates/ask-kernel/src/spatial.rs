//! Spatial queries — "what's on this cell?" in ONE scan.
//!
//! Entities are few (hundreds), so a single linear pass over Position is the
//! right structure; the sin was doing it 4–20× per lookup. All "at cell"
//! questions go through here so a real index can later slot in behind
//! the same API without touching callers.

use bevy_ecs::prelude::*;

use crate::components::Position;

/// All entities standing on (x, y), any kind.
pub fn at(world: &mut World, x: i32, y: i32) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &Position)>();
    q.iter(world)
        .filter(|(_, p)| p.x == x && p.y == y)
        .map(|(e, _)| e)
        .collect()
}

/// First entity on (x, y) matching a component predicate, e.g.
/// `find_at(world, x, y, |w, e| w.get::<Monster>(e).is_some())`.
pub fn find_at(
    world: &mut World,
    x: i32,
    y: i32,
    pred: impl Fn(&World, Entity) -> bool,
) -> Option<Entity> {
    at(world, x, y).into_iter().find(|&e| pred(world, e))
}

/// Is any entity matching `pred` on (x, y)?
pub fn any_at(world: &mut World, x: i32, y: i32, pred: impl Fn(&World, Entity) -> bool) -> bool {
    find_at(world, x, y, pred).is_some()
}
