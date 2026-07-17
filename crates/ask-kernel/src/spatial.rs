//! Spatial queries — "what's on this cell?" backed by a persistent index.
//!
//! `SpatialIndex` (a resource) maps cell → entities and maintains itself via
//! bevy **change detection**: persistent `Added<Position>` /
//! `Changed<Position>` / `RemovedComponents<Position>` readers pick up every
//! structural change since their last run, so the index is always fresh and
//! callers never think about maintenance. `at`/`find_at`/`any_at` behave
//! like plain lookups.

use bevy_ecs::prelude::*;
use bevy_ecs::component::ComponentId;
use bevy_ecs::event::EventCursor;
use bevy_ecs::removal_detection::RemovedComponentEntity;
use std::collections::HashMap;

use crate::components::Position;

#[derive(Resource)]
struct SpatialIndex {
    map: HashMap<(i32, i32), Vec<Entity>>,
    /// entity → cell (needed to evict on move/remove).
    by_entity: HashMap<Entity, (i32, i32)>,
    added: QueryState<(Entity, &'static Position), Added<Position>>,
    changed: QueryState<(Entity, &'static Position), Changed<Position>>,
    /// 'static cursor into the world's removed-component event log
    /// (RemovedComponents itself has lifetimes and can't live in a Resource).
    removed_cursor: EventCursor<RemovedComponentEntity>,
    position_component: ComponentId,
}

impl SpatialIndex {
    fn new(world: &mut World) -> Self {
        let position_component = world.register_component::<Position>();
        let mut idx = Self {
            map: HashMap::new(),
            by_entity: HashMap::new(),
            added: world.query_filtered::<(Entity, &Position), Added<Position>>(),
            changed: world.query_filtered::<(Entity, &Position), Changed<Position>>(),
            removed_cursor: EventCursor::default(),
            position_component,
        };
        idx.rebuild(world);
        idx
    }

    /// Full rebuild (first build, or as a safe fallback).
    fn rebuild(&mut self, world: &mut World) {
        self.map.clear();
        self.by_entity.clear();
        let mut q = world.query::<(Entity, &Position)>();
        for (e, p) in q.iter(world) {
            self.map.entry((p.x, p.y)).or_default().push(e);
            self.by_entity.insert(e, (p.x, p.y));
        }
    }

    fn evict(&mut self, e: Entity) {
        if let Some(cell) = self.by_entity.remove(&e) {
            if let Some(v) = self.map.get_mut(&cell) {
                v.retain(|&x| x != e);
                if v.is_empty() {
                    self.map.remove(&cell);
                }
            }
        }
    }

    /// Apply changes since the readers' last run.
    fn update(&mut self, world: &World) {
        let removed: Vec<Entity> = world
            .removed_components()
            .get(self.position_component)
            .map(|events| {
                self.removed_cursor
                    .read(events)
                    .map(|e| e.clone().into())
                    .collect()
            })
            .unwrap_or_default();
        for e in removed {
            self.evict(e);
        }

        let added: Vec<(Entity, (i32, i32))> = self
            .added
            .iter(world)
            .map(|(e, p)| (e, (p.x, p.y)))
            .collect();
        for (e, cell) in added {
            self.evict(e); // in case of id recycling
            self.by_entity.insert(e, cell);
            self.map.entry(cell).or_default().push(e);
        }

        let changed: Vec<(Entity, (i32, i32))> = self
            .changed
            .iter(world)
            .map(|(e, p)| (e, (p.x, p.y)))
            .collect();
        for (e, cell) in changed {
            if self.by_entity.get(&e).copied() == Some(cell) {
                continue;
            }
            self.evict(e);
            self.by_entity.insert(e, cell);
            self.map.entry(cell).or_default().push(e);
        }
    }
}

fn with_index<R>(world: &mut World, f: impl FnOnce(&HashMap<(i32, i32), Vec<Entity>>) -> R) -> R {
    let mut idx = world
        .remove_resource::<SpatialIndex>()
        .unwrap_or_else(|| SpatialIndex::new(world));
    idx.update(world);
    let r = f(&idx.map);
    world.insert_resource(idx);
    r
}

/// All entities standing on (x, y), any kind.
pub fn at(world: &mut World, x: i32, y: i32) -> Vec<Entity> {
    with_index(world, |m| m.get(&(x, y)).cloned().unwrap_or_default())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Agent, Glyph, StableId};

    fn spawn_at(world: &mut World, x: i32, y: i32) -> Entity {
        world
            .spawn((Position { x, y }, Glyph('@'), StableId(1)))
            .id()
    }

    #[test]
    fn index_tracks_spawn_move_despawn() {
        let mut world = World::new();
        let e = spawn_at(&mut world, 3, 4);

        assert_eq!(at(&mut world, 3, 4), vec![e]);
        // second query without changes: same answer, incremental update
        assert_eq!(at(&mut world, 3, 4), vec![e]);
        assert!(at(&mut world, 9, 9).is_empty());

        // fresh spawn lands in the index
        let e2 = spawn_at(&mut world, 5, 6);
        assert_eq!(at(&mut world, 5, 6), vec![e2]);

        // position edit through Mut is a Changed<>
        {
            let mut p = world.get_mut::<Position>(e).unwrap();
            p.x = 7;
            p.y = 8;
        }
        assert!(at(&mut world, 3, 4).is_empty());
        assert_eq!(at(&mut world, 7, 8), vec![e]);

        world.despawn(e);
        assert!(at(&mut world, 7, 8).is_empty());

        // predicate helpers
        let agent = world
            .spawn((Position { x: 1, y: 1 }, Agent, StableId(2)))
            .id();
        assert!(any_at(&mut world, 1, 1, |w, e| w.get::<Agent>(e).is_some()));
        assert_eq!(
            find_at(&mut world, 1, 1, |w, e| w.get::<Agent>(e).is_some()),
            Some(agent)
        );
    }

    #[test]
    fn index_survives_mass_despawn_and_respawn() {
        let mut world = World::new();
        let mut ents = Vec::new();
        for i in 0..10 {
            ents.push(spawn_at(&mut world, i, i));
        }
        assert_eq!(at(&mut world, 5, 5).len(), 1);
        for e in ents {
            world.despawn(e);
        }
        assert!(at(&mut world, 5, 5).is_empty());
        let n = spawn_at(&mut world, 5, 5);
        assert_eq!(at(&mut world, 5, 5), vec![n]);
    }
}
