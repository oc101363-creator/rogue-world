//! Event buffer — Frog notice_stuff batching.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameEvent {
    TickStarted {
        tick: u64,
    },
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
    TrapTriggered {
        entity: u64,
        feat: u16,
        name: String,
        damage: i32,
        at: (i32, i32),
    },
    TerrainDamage {
        entity: u64,
        kind: String,
        damage: i32,
        hp: i32,
    },
    DoorOpened {
        entity: u64,
        at: (i32, i32),
    },
    DoorClosed {
        entity: u64,
        at: (i32, i32),
    },
    LevelChanged {
        entity: u64,
        down: bool,
        depth: u32,
        seed: u64,
    },
    MonsterMoved {
        entity: u64,
        from: (i32, i32),
        to: (i32, i32),
    },
    MonsterAttacked {
        monster: u64,
        target: u64,
        damage: i32,
        target_hp: i32,
        name: String,
    },
    ItemPickedUp {
        entity: u64,
        item: u64,
        name: String,
        at: (i32, i32),
    },
    /// Player melee hit a monster.
    PlayerAttacked {
        entity: u64,
        target: u64,
        damage: i32,
        target_hp: i32,
        name: String,
        at: (i32, i32),
    },
    MonsterKilled {
        entity: u64,
        monster: u64,
        name: String,
        at: (i32, i32),
    },
    ItemDropped {
        entity: u64,
        item: u64,
        name: String,
        at: (i32, i32),
    },
    Rested {
        entity: u64,
        healed: i32,
        hp: i32,
        max_hp: i32,
    },
    Dug {
        entity: u64,
        at: (i32, i32),
        from_feat: u16,
        to_feat: u16,
    },
    Placed {
        entity: u64,
        at: (i32, i32),
        feat: u16,
    },
    Scooped {
        entity: u64,
        at: (i32, i32),
        from_feat: u16,
        to_feat: u16,
    },
    Crafted {
        entity: u64,
        recipe: String,
        label: String,
    },
    Planted {
        entity: u64,
        at: (i32, i32),
    },
    Deconstructed {
        entity: u64,
        at: (i32, i32),
        wood: u32,
    },
    /// Agent hp reached 0 — pack drops on the spot.
    AgentDied {
        entity: u64,
        at: (i32, i32),
    },
    /// Agent came back at a fresh cell with full hp (empty pack).
    AgentRespawned {
        entity: u64,
        at: (i32, i32),
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
