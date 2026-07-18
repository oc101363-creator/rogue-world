//! Whole-world save — cells are frog f_info ids; agents keep full Matter pack.
//!
//! Format v2: monsters, ground items, agent identity (profile/mailbox/glyph),
//! depth/seed, glow mask. v1 files (no `version` field) still load.

use anyhow::{bail, Context, Result};
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::actions::ActionQueue;
use crate::components::{
    Agent, AgentMailbox, AgentProfile, Building, Envelope, Glyph, Health, Inventory, Item, Matter,
    MessageCounter, Monster, Position, Resource, ResourceKind, StableId, Stack,
};
use crate::events::EventBuf;
use crate::f_info::FeatId;
use crate::grid::Grid;
use crate::world::{Depth, IdCounter, KernelConfig, KernelWorld, TickCounter, WorldSeed};

pub const SAVE_VERSION: u32 = 2;

#[derive(Serialize, Deserialize, Debug)]
pub struct WorldSnapshot {
    /// Format version. Absent in v1 files.
    #[serde(default)]
    pub version: u32,
    pub tick: u64,
    pub width: i32,
    pub height: i32,
    pub hut_wood_cost: u32,
    pub id_counter: u64,
    #[serde(default)]
    pub depth: u32,
    #[serde(default)]
    pub seed: u64,
    /// Room-light mask; empty in v1 (rebuilt dark).
    #[serde(default)]
    pub glow: Vec<bool>,
    pub cells: Vec<FeatId>,
    pub entities: Vec<EntitySnap>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntitySnap {
    Agent {
        id: u64,
        x: i32,
        y: i32,
        /// Full pack (preferred).
        #[serde(default)]
        pack: Vec<Stack>,
        /// Legacy fields for old saves.
        #[serde(default)]
        wood: u32,
        #[serde(default)]
        iron: u32,
        #[serde(default)]
        hp: Option<i32>,
        #[serde(default)]
        max_hp: Option<i32>,
        /// Identity (v2).
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        purpose: Option<String>,
        #[serde(default)]
        glyph: Option<char>,
        #[serde(default)]
        mailbox: Vec<Envelope>,
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
    Monster {
        id: u64,
        x: i32,
        y: i32,
        race_id: u16,
        name: String,
        color: char,
        glyph: char,
        hp: i32,
        max_hp: i32,
    },
    Item {
        id: u64,
        x: i32,
        y: i32,
        matter: Matter,
        qty: u32,
    },
}

pub fn capture(world: &mut World) -> WorldSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let id_counter = world.resource::<IdCounter>().0;
    let hut_wood_cost = world.resource::<KernelConfig>().hut_wood_cost;
    let depth = world.get_resource::<Depth>().map(|d| d.0).unwrap_or(0);
    let seed = world.get_resource::<WorldSeed>().map(|s| s.0).unwrap_or(0);
    let glow = world
        .get_resource::<crate::vision::GlowMask>()
        .map(|g| g.mask.clone())
        .unwrap_or_default();
    let (width, height, cells) = {
        let grid = world.resource::<Grid>();
        (grid.width, grid.height, grid.cells.clone())
    };

    let mut entities = Vec::new();

    let mut q_agent = world.query::<(
        &StableId,
        &Position,
        &Glyph,
        &Inventory,
        Option<&Health>,
        Option<&AgentProfile>,
        Option<&AgentMailbox>,
        &Agent,
    )>();
    for (id, p, g, inv, hp, profile, mailbox, _) in q_agent.iter(world) {
        entities.push(EntitySnap::Agent {
            id: id.0,
            x: p.x,
            y: p.y,
            pack: inv.slots.clone(),
            wood: inv.wood(),
            iron: inv.iron(),
            hp: hp.map(|h| h.hp),
            max_hp: hp.map(|h| h.max_hp),
            name: profile.map(|pr| pr.name.clone()),
            purpose: profile.map(|pr| pr.purpose.clone()),
            glyph: Some(g.0),
            mailbox: mailbox.map(|mb| mb.messages.clone()).unwrap_or_default(),
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

    let mut q_m = world.query::<(&StableId, &Position, &Glyph, &Monster, Option<&Health>)>();
    for (id, p, g, m, hp) in q_m.iter(world) {
        entities.push(EntitySnap::Monster {
            id: id.0,
            x: p.x,
            y: p.y,
            race_id: m.race_id,
            name: m.name.clone(),
            color: m.color,
            glyph: g.0,
            hp: hp.map(|h| h.hp).unwrap_or(crate::balance::MONSTER_HP),
            max_hp: hp.map(|h| h.max_hp).unwrap_or(crate::balance::MONSTER_HP),
        });
    }

    let mut q_i = world.query::<(&StableId, &Position, &Item)>();
    for (id, p, it) in q_i.iter(world) {
        entities.push(EntitySnap::Item {
            id: id.0,
            x: p.x,
            y: p.y,
            matter: it.matter.clone(),
            qty: it.qty,
        });
    }

    entities.sort_by_key(|e| match e {
        EntitySnap::Agent { id, .. }
        | EntitySnap::Tree { id, .. }
        | EntitySnap::Iron { id, .. }
        | EntitySnap::Hut { id, .. }
        | EntitySnap::Monster { id, .. }
        | EntitySnap::Item { id, .. } => *id,
    });

    WorldSnapshot {
        version: SAVE_VERSION,
        tick,
        width,
        height,
        hut_wood_cost,
        id_counter,
        depth,
        seed,
        glow,
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
    world.insert_resource(MessageCounter(0));
    // Channel metadata is ephemeral by design — a restore starts with an
    // empty ledger / operator inbox (same policy as EventInbox).
    world.insert_resource(crate::components::MessageLedger::default());
    world.insert_resource(crate::components::OperatorInbox::default());
    world.insert_resource(ActionQueue::default());
    world.insert_resource(EventBuf::default());
    world.insert_resource(KernelConfig {
        hut_wood_cost: snap.hut_wood_cost,
    });
    world.insert_resource(Depth(snap.depth));
    world.insert_resource(WorldSeed(snap.seed));

    for e in snap.entities {
        match e {
            EntitySnap::Agent {
                id,
                x,
                y,
                pack,
                wood,
                iron,
                hp,
                max_hp,
                name,
                purpose,
                glyph,
                mailbox,
            } => {
                let mut inv = Inventory { slots: pack };
                // legacy fallback
                if inv.slots.is_empty() {
                    if wood > 0 {
                        inv.add(
                            Matter::Resource {
                                resource: ResourceKind::Wood,
                            },
                            wood,
                        );
                    }
                    if iron > 0 {
                        inv.add(
                            Matter::Resource {
                                resource: ResourceKind::Iron,
                            },
                            iron,
                        );
                    }
                }
                let health = Health {
                    hp: hp.unwrap_or(crate::balance::AGENT_HP),
                    max_hp: max_hp.unwrap_or(crate::balance::AGENT_HP),
                };
                let mut e = world.spawn((
                    Agent,
                    Position { x, y },
                    Glyph(glyph.unwrap_or('A')),
                    inv,
                    health,
                    StableId(id),
                    crate::components::VisionMemory::new(snap.width, snap.height),
                ));
                if let Some(name) = name {
                    e.insert(AgentProfile {
                        name,
                        purpose: purpose.unwrap_or_default(),
                    });
                }
                if !mailbox.is_empty() {
                    e.insert(AgentMailbox { messages: mailbox });
                }
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
            EntitySnap::Monster {
                id,
                x,
                y,
                race_id,
                name,
                color,
                glyph,
                hp,
                max_hp,
            } => {
                world.spawn((
                    Position { x, y },
                    Glyph(glyph),
                    Monster {
                        race_id,
                        name,
                        color,
                    },
                    Health { hp, max_hp },
                    StableId(id),
                ));
            }
            EntitySnap::Item {
                id,
                x,
                y,
                matter,
                qty,
            } => {
                let glyph = matter.glyph();
                world.spawn((
                    Position { x, y },
                    Glyph(glyph),
                    Item { matter, qty },
                    StableId(id),
                ));
            }
        }
    }

    let glow = if snap.glow.len() == (snap.width * snap.height) as usize {
        Some(snap.glow)
    } else {
        None
    };
    crate::vision::install_and_update(&mut world, glow);
    KernelWorld { world }
}

pub fn save_to_path(world: &mut World, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let snap = capture(world);
    let json = serde_json::to_string_pretty(&snap).context("serialize")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn load_from_path(path: impl AsRef<Path>) -> Result<KernelWorld> {
    let path = path.as_ref();
    let s = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let snap: WorldSnapshot = serde_json::from_str(&s).context("parse world json")?;
    if snap.version > SAVE_VERSION {
        bail!("save version {} > supported {}", snap.version, SAVE_VERSION);
    }
    if snap.cells.len() != (snap.width * snap.height) as usize {
        bail!("cell count mismatch");
    }
    Ok(restore(snap))
}
