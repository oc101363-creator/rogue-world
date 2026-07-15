//! Action queue — Frog do_cmd → effect, deferred to tick apply phase.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Move { dx: i32, dy: i32 },
    Harvest,
    BuildHut,
    /// Open a closed/secret door on an adjacent cell (or underfoot).
    OpenDoor { dx: i32, dy: i32 },
    /// Close an open door adjacent / underfoot.
    CloseDoor { dx: i32, dy: i32 },
    /// Use stairs on current cell: true = down (>), false = up (<).
    UseStairs { down: bool },
    Idle,
}

#[derive(Clone, Debug)]
pub struct QueuedAction {
    pub entity: Entity,
    pub action: Action,
}

/// Per-tick intent buffer (Frog: commands collected before world settles).
#[derive(Resource, Default, Debug)]
pub struct ActionQueue {
    pub items: Vec<QueuedAction>,
}

impl ActionQueue {
    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn push(&mut self, entity: Entity, action: Action) {
        self.items.push(QueuedAction { entity, action });
    }
}
