//! Player / external-agent intent bus (Frog: request_command → do_cmd → effect).
//!
//! External clients never mutate World; they only enqueue Actions.
//! Sim consumes them once per tick (last-write-wins per agent).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy_ecs::prelude::*;

use crate::actions::Action;
use crate::agents::mock::MockPolicy;
use crate::agents::AgentPolicy;
use crate::components::{Agent, StableId};

/// Shared queue of player intents. Safe to clone (Arc).
#[derive(Clone, Default)]
pub struct PlayerActionBus {
    inner: Arc<Mutex<BusInner>>,
}

#[derive(Default)]
struct BusInner {
    /// stable_id → pending action (overwritten if multiple submits before tick).
    pending: HashMap<u64, Action>,
    /// When true, agents without a pending action Idle (no MockPolicy).
    human_control: bool,
    /// Last accepted submit tick hint (for client ack).
    last_submit_tick: Option<u64>,
}

impl PlayerActionBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Submit an action for `agent_id`. `None` → first agent at consume time
    /// is handled via `take_for_entity` fallback key `0` (wildcard).
    pub fn submit(&self, agent_id: Option<u64>, action: Action, tick_hint: Option<u64>) {
        let mut g = self.inner.lock().expect("player bus");
        g.human_control = true;
        g.last_submit_tick = tick_hint;
        let key = agent_id.unwrap_or(0);
        g.pending.insert(key, action);
    }

    pub fn human_control(&self) -> bool {
        self.inner.lock().expect("player bus").human_control
    }

    pub fn set_human_control(&self, on: bool) {
        self.inner.lock().expect("player bus").human_control = on;
    }

    /// Take action for this stable id. Also checks wildcard key `0` (any/first agent).
    pub fn take_for(&self, stable_id: u64) -> Option<Action> {
        let mut g = self.inner.lock().expect("player bus");
        if let Some(a) = g.pending.remove(&stable_id) {
            // consume wildcard if it was meant for the only agent
            g.pending.remove(&0);
            return Some(a);
        }
        g.pending.remove(&0)
    }

    pub fn pending_count(&self) -> usize {
        self.inner.lock().expect("player bus").pending.len()
    }
}

/// Policy: player bus first, else Mock (or Idle when human_control).
pub struct BusPolicy {
    pub bus: PlayerActionBus,
    mock: MockPolicy,
    /// If true and not human_control, fall back to MockPolicy.
    pub allow_mock: bool,
}

impl BusPolicy {
    pub fn new(bus: PlayerActionBus, allow_mock: bool) -> Self {
        Self {
            bus,
            mock: MockPolicy,
            allow_mock,
        }
    }
}

impl AgentPolicy for BusPolicy {
    fn decide(&mut self, world: &mut World, agent: Entity) -> Action {
        use crate::components::AgentProfile;

        let sid = world.get::<StableId>(agent).map(|s| s.0).unwrap_or(0);

        if let Some(a) = self.bus.take_for(sid) {
            return a;
        }

        // Registered agents (have profile) only act when they submit.
        let registered = world.get::<AgentProfile>(agent).is_some();
        if registered {
            return Action::Idle;
        }

        if self.bus.human_control() {
            return Action::Idle;
        }

        if self.allow_mock {
            return self.mock.decide(world, agent);
        }

        let _: Option<&Agent> = world.get::<Agent>(agent);
        Action::Idle
    }
}
