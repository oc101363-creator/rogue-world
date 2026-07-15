//! Kernel world bootstrap — single source of truth (Frog: cave + entity lists).

use bevy_ecs::prelude::*;

use crate::actions::ActionQueue;
use crate::components::{
    Agent, Glyph, Health, Inventory, Item, Monster, Position, Resource, ResourceKind, StableId,
};
use crate::vaults::{SpawnMon, SpawnObj};
use crate::config::Config;
use crate::events::EventBuf;
use crate::generate::generate_level;

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

pub struct KernelWorld {
    pub world: World,
}

impl KernelWorld {
    pub fn new(cfg: &Config) -> Self {
        let mut cfg = cfg.clone();
        let level = generate_level(&cfg);

        let mut world = World::new();
        world.insert_resource(level.grid);
        world.insert_resource(TickCounter(0));
        world.insert_resource(IdCounter(0));
        world.insert_resource(ActionQueue::default());
        world.insert_resource(EventBuf::default());
        world.insert_resource(KernelConfig {
            hut_wood_cost: cfg.hut_wood_cost,
        });
        world.insert_resource(Depth(0));
        world.insert_resource(WorldSeed(cfg.seed));

        let mut kw = Self { world };
        kw.spawn_level_entities(
            &cfg,
            level.agent,
            &level.trees,
            &level.irons,
            &level.monsters,
            &level.items,
        );
        kw
    }

    /// Rebuild grid/entities for a new seed (stairs). Keeps agent inventory/health.
    pub fn change_level(&mut self, seed: u64, depth: u32, hut_wood_cost: u32, tree_amount: u32, iron_amount: u32) {
        // preserve agent stats
        let saved = {
            let mut q = self.world.query::<(&Inventory, &Health, &StableId)>();
            q.iter(&self.world)
                .next()
                .map(|(inv, hp, id)| (inv.clone(), *hp, id.0))
        };

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
        // keep tick counter / event buf

        let (inv, hp, sid) = saved.unwrap_or((Inventory::default(), Health::default(), 1));
        {
            let mut c = self.world.resource_mut::<IdCounter>();
            if c.0 < sid {
                c.0 = sid;
            }
        }
        // re-spawn agent with saved stats after clear — handled via spawn helper below
        // temporarily store then spawn all
        let agent_pos = level.agent;
        let trees = level.trees.clone();
        let irons = level.irons.clone();
        let monsters = level.monsters.clone();
        let items = level.items.clone();

        self.world.spawn((
            Agent,
            Position {
                x: agent_pos.0,
                y: agent_pos.1,
            },
            Glyph('A'),
            inv,
            hp,
            StableId(sid),
        ));
        for (x, y) in trees {
            let id = self.next_id();
            self.world.spawn((
                Position { x, y },
                Glyph('T'),
                Resource {
                    kind: ResourceKind::Wood,
                    amount: tree_amount,
                },
                StableId(id),
            ));
        }
        for (x, y) in irons {
            let id = self.next_id();
            self.world.spawn((
                Position { x, y },
                Glyph('I'),
                Resource {
                    kind: ResourceKind::Iron,
                    amount: iron_amount,
                },
                StableId(id),
            ));
        }
        for m in monsters {
            let id = self.next_id();
            self.world.spawn((
                Position { x: m.x, y: m.y },
                Glyph(m.glyph),
                Monster {
                    race_id: m.race_id,
                    name: m.name,
                    color: m.color,
                },
                StableId(id),
            ));
        }
        for o in items {
            let id = self.next_id();
            self.world.spawn((
                Position { x: o.x, y: o.y },
                Glyph(o.glyph),
                Item {
                    kind_id: o.kind_id,
                    name: o.name,
                    color: o.color,
                },
                StableId(id),
            ));
        }
    }

    fn spawn_level_entities(
        &mut self,
        cfg: &Config,
        agent: (i32, i32),
        trees: &[(i32, i32)],
        irons: &[(i32, i32)],
        monsters: &[SpawnMon],
        items: &[SpawnObj],
    ) {
        let id = self.next_id();
        self.world.spawn((
            Agent,
            Position {
                x: agent.0,
                y: agent.1,
            },
            Glyph('A'),
            Inventory::default(),
            Health::default(),
            StableId(id),
        ));

        for &(x, y) in trees {
            let id = self.next_id();
            self.world.spawn((
                Position { x, y },
                Glyph('T'),
                Resource {
                    kind: ResourceKind::Wood,
                    amount: cfg.tree_amount,
                },
                StableId(id),
            ));
        }

        for &(x, y) in irons {
            let id = self.next_id();
            self.world.spawn((
                Position { x, y },
                Glyph('I'),
                Resource {
                    kind: ResourceKind::Iron,
                    amount: cfg.iron_amount,
                },
                StableId(id),
            ));
        }

        for m in monsters {
            let id = self.next_id();
            self.world.spawn((
                Position { x: m.x, y: m.y },
                Glyph(m.glyph),
                Monster {
                    race_id: m.race_id,
                    name: m.name.clone(),
                    color: m.color,
                },
                StableId(id),
            ));
        }

        for o in items {
            let id = self.next_id();
            self.world.spawn((
                Position { x: o.x, y: o.y },
                Glyph(o.glyph),
                Item {
                    kind_id: o.kind_id,
                    name: o.name.clone(),
                    color: o.color,
                },
                StableId(id),
            ));
        }
    }

    fn next_id(&mut self) -> u64 {
        let mut c = self.world.resource_mut::<IdCounter>();
        c.0 += 1;
        c.0
    }

    pub fn tick(&self) -> u64 {
        self.world.resource::<TickCounter>().0
    }

    pub fn agent_entity(&mut self) -> Option<Entity> {
        let mut q = self.world.query_filtered::<Entity, With<Agent>>();
        q.iter(&self.world).next()
    }
}
