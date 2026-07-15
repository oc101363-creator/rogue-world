//! Kernel world bootstrap — single source of truth (Frog: cave + entity lists).

use bevy_ecs::prelude::*;

use crate::actions::ActionQueue;
use crate::components::{
    Agent, Glyph, Inventory, Position, Resource, ResourceKind, StableId,
};
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

pub struct KernelWorld {
    pub world: World,
}

impl KernelWorld {
    pub fn new(cfg: &Config) -> Self {
        let level = generate_level(cfg);

        let mut world = World::new();
        world.insert_resource(level.grid);
        world.insert_resource(TickCounter(0));
        world.insert_resource(IdCounter(0));
        world.insert_resource(ActionQueue::default());
        world.insert_resource(EventBuf::default());
        world.insert_resource(KernelConfig {
            hut_wood_cost: cfg.hut_wood_cost,
        });

        let mut kw = Self { world };

        let id = kw.next_id();
        kw.world.spawn((
            Agent,
            Position {
                x: level.agent.0,
                y: level.agent.1,
            },
            Glyph('A'),
            Inventory::default(),
            StableId(id),
        ));

        for (x, y) in level.trees {
            let id = kw.next_id();
            kw.world.spawn((
                Position { x, y },
                Glyph('T'),
                Resource {
                    kind: ResourceKind::Wood,
                    amount: cfg.tree_amount,
                },
                StableId(id),
            ));
        }

        for (x, y) in level.irons {
            let id = kw.next_id();
            kw.world.spawn((
                Position { x, y },
                Glyph('I'),
                Resource {
                    kind: ResourceKind::Iron,
                    amount: cfg.iron_amount,
                },
                StableId(id),
            ));
        }

        kw
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
