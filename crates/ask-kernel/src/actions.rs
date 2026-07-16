//! Minimal action primitives. World options come from Interact discovery.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// The ONE step-range check (was copy-pasted into six files).
/// `allow_underfoot`: (0,0) targets self cell; otherwise must be 4-way adjacent.
pub fn check_step(dx: i32, dy: i32, allow_underfoot: bool) -> Result<(), &'static str> {
    if allow_underfoot && dx == 0 && dy == 0 {
        return Ok(());
    }
    if dx.abs() + dy.abs() == 1 {
        Ok(())
    } else if allow_underfoot {
        Err("underfoot or adjacent only")
    } else {
        Err("needs four-way unit step")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Move {
        dx: i32,
        dy: i32,
    },
    /// Use cell (dx,dy). verb from interactions[].
    /// slot: pack index (place). recipe: craft id (craft).
    Interact {
        dx: i32,
        dy: i32,
        #[serde(default)]
        verb: Option<String>,
        #[serde(default)]
        slot: Option<usize>,
        #[serde(default)]
        recipe: Option<String>,
    },
    Drop {
        index: usize,
    },
    Rest,
    Idle,
}

impl Action {
    pub fn catalog() -> serde_json::Value {
        serde_json::json!([
            {"type":"move","note":"four-way unit step"},
            {"type":"interact","note":"verbs: dig scoop place harvest plant build deconstruct craft open close attack pickup stairs…; recipe for craft; slot for place"},
            {"type":"drop","note":"drop pack slots[index] underfoot"},
            {"type":"rest","note":"heal 1 HP"},
            {"type":"idle","note":"wait"},
        ])
    }
}

#[derive(Clone, Debug)]
pub struct QueuedAction {
    pub entity: Entity,
    pub action: Action,
}

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Interaction {
    pub dx: i32,
    pub dy: i32,
    pub verb: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipe: Option<String>,
}
