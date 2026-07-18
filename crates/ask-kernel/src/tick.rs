//! Phased simulation loop — Frog dungeon main loop structure.

use bevy_ecs::prelude::*;

use crate::actions::ActionQueue;
use crate::agents::{mock::MockPolicy, AgentPolicy};
use crate::components::Agent;
use crate::events::EventBuf;
use crate::systems::terrain::PendingLevelChange;
use crate::systems::{
    advance_tick_system, apply_actions_system, begin_tick_system, check_deaths, pickup_items,
    process_monsters, process_world,
};
use crate::view;
use crate::vision;
use crate::world::{KernelConfig, KernelWorld};

pub struct Sim {
    pub kernel: KernelWorld,
    policy: Box<dyn AgentPolicy>,
}

impl Sim {
    pub fn new(kernel: KernelWorld) -> Self {
        Self {
            kernel,
            policy: Box::new(MockPolicy),
        }
    }

    pub fn with_policy(kernel: KernelWorld, policy: Box<dyn AgentPolicy>) -> Self {
        Self { kernel, policy }
    }

    /// One full turn (Frog: player/monsters/world + game_turn++).
    pub fn step(&mut self) {
        let world = &mut self.kernel.world;

        begin_tick_system(world);

        let agents: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<Agent>>();
            q.iter(world).collect()
        };
        for agent in agents {
            let action = self.policy.decide(world, agent);
            world.resource_mut::<ActionQueue>().push(agent, action);
        }

        apply_actions_system(world);

        // pickup after agent move (same cell as items)
        pickup_items(&mut self.kernel.world);

        // frog process_monsters phase
        process_monsters(&mut self.kernel.world);

        // hp 0 → drop pack, respawn elsewhere (before any level rebuild)
        check_deaths(&mut self.kernel.world);

        // world processes: fire/water/grass evolve on their own
        process_world(&mut self.kernel.world);

        // Stairs: rebuild level if requested this tick
        let pending = self.kernel.world.remove_resource::<PendingLevelChange>();
        if let Some(p) = pending {
            let hut = self.kernel.world.resource::<KernelConfig>().hut_wood_cost;
            self.kernel.change_level(p.seed, p.depth, hut, 4, 4);
        }

        // Frog update_view after player/monsters settle
        vision::update_view(&mut self.kernel.world);
        vision::update_agent_memories(&mut self.kernel.world);

        // feedback: route this tick's events into per-agent inboxes
        // (push-time FOV — survives however long the agent thinks)
        crate::events::distribute_feedback(&mut self.kernel.world);

        advance_tick_system(&mut self.kernel.world);
    }

    pub fn run_steps(&mut self, n: u64, print_each: bool) {
        for _ in 0..n {
            self.step();
            if print_each {
                print!("{}", view::render(&mut self.kernel.world));
                let ev = self.kernel.world.resource_mut::<EventBuf>().drain();
                for e in ev.iter().take(5) {
                    println!("  evt: {e:?}");
                }
            } else {
                self.kernel.world.resource_mut::<EventBuf>().clear();
            }
        }
    }
}
