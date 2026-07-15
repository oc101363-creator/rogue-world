//! Event buffer — Frog notice_stuff batching.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    TickStarted { tick: u64 },
    Moved {
        entity: u64,
        from: (i32, i32),
        to: (i32, i32),
    },
    MoveFailed {
        entity: u64,
        reason: String,
    },
    Harvested {
        entity: u64,
        kind: String,
        amount: u32,
        inventory_wood: u32,
        inventory_iron: u32,
    },
    ResourceDepleted {
        entity: u64,
    },
    Built {
        builder: u64,
        at: (i32, i32),
    },
    BuildFailed {
        entity: u64,
        reason: String,
    },
    ActionRejected {
        entity: u64,
        reason: String,
    },
}

#[derive(Resource, Default, Debug)]
pub struct EventBuf {
    pub events: Vec<GameEvent>,
}

impl EventBuf {
    pub fn push(&mut self, e: GameEvent) {
        self.events.push(e);
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn drain(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }
}
