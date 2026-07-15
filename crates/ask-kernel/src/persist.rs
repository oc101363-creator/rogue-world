//! Whole-world save — Frog savefile idea via serde JSON.

use anyhow::{bail, Context, Result};
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::components::{
    Agent, Building, Glyph, Inventory, Position, Resource, ResourceKind, StableId,
};
use crate::feat::Feat;
use crate::grid::Grid;
use crate::world::{IdCounter, KernelConfig, KernelWorld, TickCounter};
use crate::actions::ActionQueue;
use crate::events::EventBuf;
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldSnapshot {
    pub tick: u64,
    pub width: i32,
    pub height: i32,
    pub hut_wood_cost: u32,
    pub id_counter: u64,
    pub cells: Vec<Feat>,
    pub entities: Vec<EntitySnap>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntitySnap {
    Agent {
        id: u64,
        x: i32,
        y: i32,
        wood: u32,
        iron: u32,
    },
    Tree {
        id: u64,
        x: i32,
        y: i32,
        amount: u32,
    },
    Iron {
        id: u64,
        x: i32,
        y: i32,
        amount: u32,
    },
    Hut {
        id: u64,
        x: i32,
        y: i32,
    },
}

pub fn capture(world: &mut World) -> WorldSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let id_counter = world.resource::<IdCounter>().0;
    let hut_wood_cost = world.resource::<KernelConfig>().hut_wood_cost;
    let (width, height, cells) = {
        let grid = world.resource::<Grid>();
        (grid.width, grid.height, grid.cells.clone())
    };

    let mut entities = Vec::new();

    let mut q_agent = world.query::<(&StableId, &Position, &Inventory, &Agent)>();
    for (id, p, inv, _) in q_agent.iter(world) {
        entities.push(EntitySnap::Agent {
            id: id.0,
            x: p.x,
            y: p.y,
            wood: inv.wood,
            iron: inv.iron,
        });
    }

    let mut q_res = world.query::<(&StableId, &Position, &Resource)>();
    for (id, p, r) in q_res.iter(world) {
        match r.kind {
            ResourceKind::Wood => entities.push(EntitySnap::Tree {
                id: id.0,
                x: p.x,
                y: p.y,
                amount: r.amount,
            }),
            ResourceKind::Iron => entities.push(EntitySnap::Iron {
                id: id.0,
                x: p.x,
                y: p.y,
                amount: r.amount,
            }),
        }
    }

    let mut q_b = world.query::<(&StableId, &Position, &Building)>();
    for (id, p, _) in q_b.iter(world) {
        entities.push(EntitySnap::Hut {
            id: id.0,
            x: p.x,
            y: p.y,
        });
    }

    entities.sort_by_key(|e| match e {
        EntitySnap::Agent { id, .. }
        | EntitySnap::Tree { id, .. }
        | EntitySnap::Iron { id, .. }
        | EntitySnap::Hut { id, .. } => *id,
    });

    WorldSnapshot {
        tick,
        width,
        height,
        hut_wood_cost,
        id_counter,
        cells,
        entities,
    }
}

pub fn restore(snap: WorldSnapshot) -> KernelWorld {
    let mut world = World::new();
    world.insert_resource(Grid {
        width: snap.width,
        height: snap.height,
        cells: snap.cells,
    });
    world.insert_resource(TickCounter(snap.tick));
    world.insert_resource(IdCounter(snap.id_counter));
    world.insert_resource(ActionQueue::default());
    world.insert_resource(EventBuf::default());
    world.insert_resource(KernelConfig {
        hut_wood_cost: snap.hut_wood_cost,
    });

    for e in snap.entities {
        match e {
            EntitySnap::Agent {
                id,
                x,
                y,
                wood,
                iron,
            } => {
                world.spawn((
                    Agent,
                    Position { x, y },
                    Glyph('A'),
                    Inventory { wood, iron },
                    StableId(id),
                ));
            }
            EntitySnap::Tree { id, x, y, amount } => {
                world.spawn((
                    Position { x, y },
                    Glyph('T'),
                    Resource {
                        kind: ResourceKind::Wood,
                        amount,
                    },
                    StableId(id),
                ));
            }
            EntitySnap::Iron { id, x, y, amount } => {
                world.spawn((
                    Position { x, y },
                    Glyph('I'),
                    Resource {
                        kind: ResourceKind::Iron,
                        amount,
                    },
                    StableId(id),
                ));
            }
            EntitySnap::Hut { id, x, y } => {
                world.spawn((Position { x, y }, Glyph('H'), Building, StableId(id)));
            }
        }
    }

    KernelWorld { world }
}

pub fn save_to_path(world: &mut World, path: impl AsRef<Path>) -> Result<()> {
    let snap = capture(world);
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string_pretty(&snap)?;
    fs::write(path, s)?;
    Ok(())
}

pub fn load_from_path(path: impl AsRef<Path>) -> Result<KernelWorld> {
    let s = fs::read_to_string(path).context("read save")?;
    let snap: WorldSnapshot = serde_json::from_str(&s).context("parse save")?;
    if snap.width < 3 || snap.height < 3 {
        bail!("invalid map size");
    }
    Ok(restore(snap))
}

/// Helper for tests: fresh default world.
pub fn new_default() -> KernelWorld {
    KernelWorld::new(&Config::default())
}
