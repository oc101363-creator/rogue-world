//! Phased simulation loop — Frog dungeon main loop structure.

use bevy_ecs::prelude::*;

use crate::actions::ActionQueue;
use crate::agents::{mock::MockPolicy, AgentPolicy};
use crate::components::Agent;
use crate::events::EventBuf;
use crate::systems::{advance_tick_system, apply_actions_system, begin_tick_system};
use crate::view;
use crate::world::KernelWorld;

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

        // Phase 1 prep
        begin_tick_system(world);

        // Phase 1 collect — all agents
        let agents: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<Agent>>();
            q.iter(world).collect()
        };
        for agent in agents {
            let action = self.policy.decide(world, agent); // world: &mut for query API
            world.resource_mut::<ActionQueue>().push(agent, action);
        }

        // Phase 2 apply
        apply_actions_system(world);

        // Phase 3 world systems (empty MVP)

        // Phase 4 commit — events retained until drained by caller/view
        advance_tick_system(world);
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
