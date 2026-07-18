//! Kernel world bootstrap — single source of truth (Frog: cave + entity lists).

use bevy_ecs::prelude::*;

use crate::actions::ActionQueue;
use crate::components::{
    Agent, AgentMailbox, AgentProfile, Glyph, Health, Inventory, Item, MessageCounter, Monster,
    Position, Resource, ResourceKind, StableId, VisionMemory,
};
use crate::config::Config;
use crate::events::EventBuf;
use crate::generate::generate_level;
use crate::grid::Grid;
use crate::vaults::{SpawnMon, SpawnObj};
use crate::vision;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct TickCounter(pub u64);

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct IdCounter(pub u64);

#[derive(Resource, Clone, Debug)]
pub struct KernelConfig {
    pub hut_wood_cost: u32,
}

/// Dungeon depth (0 = starting level). Stairs change this.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct Depth(pub u32);

/// World generation seed (mutated on level change).
#[derive(Resource, Debug, Clone, Copy)]
pub struct WorldSeed(pub u64);

/// Everything that makes an agent *themselves* across a level rebuild.
#[derive(Clone)]
struct AgentState {
    sid: u64,
    inv: Inventory,
    hp: Health,
    glyph: char,
    profile: Option<AgentProfile>,
    mailbox: AgentMailbox,
}

pub struct KernelWorld {
    pub world: World,
}

impl KernelWorld {
    pub fn new(cfg: &Config) -> Self {
        let cfg = cfg.clone();
        let level = generate_level(&cfg);

        let mut world = World::new();
        world.insert_resource(level.grid);
        world.insert_resource(TickCounter(0));
        world.insert_resource(IdCounter(0));
        world.insert_resource(MessageCounter(0));
        world.insert_resource(ActionQueue::default());
        world.insert_resource(EventBuf::default());
        world.insert_resource(KernelConfig {
            hut_wood_cost: cfg.hut_wood_cost,
        });
        world.insert_resource(Depth(0));
        world.insert_resource(WorldSeed(cfg.seed));

        let mut kw = Self { world };
        let sid = kw.next_id();
        kw.spawn_agent_full(
            None,
            'A',
            Inventory::default(),
            Health::default(),
            sid,
            level.agent,
        );
        kw.spawn_level_fill(
            &level.trees,
            &level.irons,
            &level.monsters,
            &level.items,
            &cfg,
        );
        // Frog update_view after level birth
        vision::install_and_update(&mut kw.world, Some(level.glow));
        kw
    }

    /// Rebuild grid/entities for a new seed (stairs).
    /// Every agent crosses over with body (pack/hp) AND identity
    /// (stable id, profile, glyph, mailbox). Vision memory resets: new map.
    pub fn change_level(
        &mut self,
        seed: u64,
        depth: u32,
        hut_wood_cost: u32,
        tree_amount: u32,
        iron_amount: u32,
    ) {
        // Preserve ALL agents, not just the first.
        let mut saved: Vec<AgentState> = {
            let mut q = self.world.query_filtered::<
                (
                    &StableId,
                    &Inventory,
                    &Health,
                    &Glyph,
                    Option<&AgentProfile>,
                    Option<&AgentMailbox>,
                ),
                With<Agent>,
            >();
            q.iter(&self.world)
                .map(|(sid, inv, hp, g, pr, mb)| AgentState {
                    sid: sid.0,
                    inv: inv.clone(),
                    hp: *hp,
                    glyph: g.0,
                    profile: pr.cloned(),
                    mailbox: mb.cloned().unwrap_or_default(),
                })
                .collect()
        };
        saved.sort_by_key(|a| a.sid);

        let mut cfg = Config::default();
        cfg.seed = seed;
        cfg.hut_wood_cost = hut_wood_cost;
        cfg.tree_amount = tree_amount;
        cfg.iron_amount = iron_amount;
        let level = generate_level(&cfg);

        // clear entities
        let ents: Vec<_> = self.world.iter_entities().map(|e| e.id()).collect();
        for e in ents {
            self.world.despawn(e);
        }

        self.world.insert_resource(level.grid);
        self.world.insert_resource(Depth(depth));
        self.world.insert_resource(WorldSeed(seed));
        // keep tick counter / event buf / id counter

        // id counter must stay ahead of every preserved stable id
        if let Some(max_sid) = saved.iter().map(|a| a.sid).max() {
            let mut c = self.world.resource_mut::<IdCounter>();
            if c.0 < max_sid {
                c.0 = max_sid;
            }
        }

        // Re-spawn the party: first agent on the arrival cell, the rest scatter.
        for (i, a) in saved.into_iter().enumerate() {
            let pos = if i == 0 {
                level.agent
            } else {
                self.random_free_cell(a.sid).unwrap_or(level.agent)
            };
            let e = self.spawn_agent_full(a.profile, a.glyph, a.inv, a.hp, a.sid, pos);
            if !a.mailbox.messages.is_empty() {
                self.world.entity_mut(e).insert(a.mailbox);
            }
        }
        self.spawn_level_fill(
            &level.trees,
            &level.irons,
            &level.monsters,
            &level.items,
            &cfg,
        );
        vision::install_and_update(&mut self.world, Some(level.glow));
    }

    /// Shared level population: resources, monsters, ground items.
    fn spawn_level_fill(
        &mut self,
        trees: &[(i32, i32)],
        irons: &[(i32, i32)],
        monsters: &[SpawnMon],
        items: &[SpawnObj],
        cfg: &Config,
    ) {
        for &(x, y) in trees {
            self.spawn_tree(x, y, cfg.tree_amount);
        }
        for &(x, y) in irons {
            self.spawn_iron(x, y, cfg.iron_amount);
        }
        for m in monsters {
            self.spawn_monster(m);
        }
        for o in items {
            self.spawn_item(o);
        }
    }

    /// Spawn an agent with the complete bundle — the ONE place agent
    /// entities are assembled (identity + body + fresh vision memory).
    fn spawn_agent_full(
        &mut self,
        profile: Option<AgentProfile>,
        glyph: char,
        inv: Inventory,
        hp: Health,
        sid: u64,
        pos: (i32, i32),
    ) -> Entity {
        let (vw, vh) = {
            let g = self.world.resource::<Grid>();
            (g.width, g.height)
        };
        let mut e = self.world.spawn((
            Agent,
            AgentMailbox::new(),
            crate::components::EventInbox::default(),
            Position { x: pos.0, y: pos.1 },
            Glyph(glyph),
            inv,
            hp,
            VisionMemory::new(vw, vh),
            StableId(sid),
        ));
        if let Some(p) = profile {
            e.insert(p);
        }
        e.id()
    }

    fn spawn_tree(&mut self, x: i32, y: i32, amount: u32) -> Entity {
        let id = self.next_id();
        self.world
            .spawn((
                Position { x, y },
                Glyph('T'),
                Resource {
                    kind: ResourceKind::Wood,
                    amount,
                },
                StableId(id),
            ))
            .id()
    }

    fn spawn_iron(&mut self, x: i32, y: i32, amount: u32) -> Entity {
        let id = self.next_id();
        self.world
            .spawn((
                Position { x, y },
                Glyph('I'),
                Resource {
                    kind: ResourceKind::Iron,
                    amount,
                },
                StableId(id),
            ))
            .id()
    }

    fn spawn_monster(&mut self, m: &SpawnMon) -> Entity {
        let id = self.next_id();
        // Data-driven hp from r_info race; balance constant is the fallback.
        let hp = crate::r_info::table()
            .get(m.race_id)
            .and_then(|r| r.hp)
            .unwrap_or(crate::balance::MONSTER_HP);
        self.world
            .spawn((
                Position { x: m.x, y: m.y },
                Glyph(m.glyph),
                Monster {
                    race_id: m.race_id,
                    name: m.name.clone(),
                    color: m.color,
                },
                Health { hp, max_hp: hp },
                StableId(id),
            ))
            .id()
    }

    fn spawn_item(&mut self, o: &SpawnObj) -> Entity {
        let id = self.next_id();
        let matter = crate::components::Matter::Object {
            kind_id: o.kind_id,
            name: o.name.clone(),
        };
        self.world
            .spawn((
                Position { x: o.x, y: o.y },
                Glyph(o.glyph),
                Item { matter, qty: 1 },
                StableId(id),
            ))
            .id()
    }

    fn next_id(&mut self) -> u64 {
        next_id(&mut self.world)
    }

    pub fn tick(&self) -> u64 {
        self.world.resource::<TickCounter>().0
    }

    pub fn agent_entity(&mut self) -> Option<Entity> {
        let mut q = self.world.query_filtered::<Entity, With<Agent>>();
        q.iter(&self.world).next()
    }

    /// Random free (buildable, agent-unoccupied) cell anywhere on the map.
    fn random_free_cell(&mut self, mix: u64) -> Option<(i32, i32)> {
        random_free_cell(&mut self.world, mix)
    }

    /// Spawn a new registered agent on a **random** free floor cell (anywhere on map).
    pub fn spawn_agent(&mut self, name: String, purpose: String) -> Option<(u64, i32, i32)> {
        let id = self.next_id();
        let (x, y) = self.random_free_cell(id)?;
        let glyph = name
            .chars()
            .find(|c| c.is_ascii_alphabetic())
            .map(|c| c.to_ascii_uppercase())
            .unwrap_or('@');
        self.spawn_agent_full(
            Some(AgentProfile { name, purpose }),
            glyph,
            Inventory::default(),
            Health::default(),
            id,
            (x, y),
        );
        vision::update_view(&mut self.world);
        Some((id, x, y))
    }
}

/// Allocate the next stable id — THE one counter (systems: use this,
/// never increment IdCounter inline).
pub fn next_id(world: &mut World) -> u64 {
    let mut c = world.resource_mut::<IdCounter>();
    c.0 += 1;
    c.0
}

/// Random free (buildable, agent-unoccupied) cell anywhere on the map.
/// `mix` decorrelates repeated picks (agent id, tick, …).
/// Shared by spawn paths and the death/respawn system.
/// NOTE: only agents count as occupants — a teleport/respawn may share a
/// cell with a monster or item (roguelike-tolerable by design).
pub fn random_free_cell(world: &mut World, mix: u64) -> Option<(i32, i32)> {
    let occupied: Vec<(i32, i32)> = {
        let mut q = world.query_filtered::<&Position, With<Agent>>();
        q.iter(world).map(|p| (p.x, p.y)).collect()
    };
    let tick = world.resource::<TickCounter>().0;
    let seed = world
        .get_resource::<WorldSeed>()
        .map(|s| s.0)
        .unwrap_or(1);
    let rng_state = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(mix.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add(tick.wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add(0xA076_1D64_78BD_642F)
        | 1;

    let g = world.resource::<Grid>();
    let free: Vec<(i32, i32)> = (1..g.width - 1)
        .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            g.buildable(x, y) && !occupied.iter().any(|&(ox, oy)| ox == x && oy == y)
        })
        .collect();
    if free.is_empty() {
        return None;
    }
    // xorshift64*
    let mut x = rng_state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    let pick = (x.wrapping_mul(0x2545_F491_4F6C_DD1D) as usize) % free.len();
    Some(free[pick])
}
