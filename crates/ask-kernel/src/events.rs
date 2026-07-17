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

/// FOV-gate for event feeds: may `self_sid` learn of `ev` given visibility
/// `vis`? Rules: TickStarted is pure noise (never shown); entity-bearing
/// events are visible iff they concern the recipient; location-bearing
/// events are visible iff the place is visible/remembered; variants with
/// both pass on either.
pub fn event_visible(ev: &GameEvent, vis: &crate::vision::VisionMap, self_sid: Option<u64>) -> bool {
    let is_self = |e: u64| self_sid.map(|s| s == e).unwrap_or(false);
    let at_seen = |at: (i32, i32)| vis.display_class(at.0, at.1) > 0;
    match ev {
        GameEvent::TickStarted { .. } => false,
        GameEvent::Moved { entity, .. } => is_self(*entity),
        GameEvent::MoveFailed { entity, .. } => is_self(*entity),
        GameEvent::Harvested { entity, .. } => is_self(*entity),
        GameEvent::ResourceDepleted { .. } => false,
        GameEvent::Built { builder, at } => is_self(*builder) || at_seen(*at),
        GameEvent::BuildFailed { entity, .. } => is_self(*entity),
        GameEvent::ActionRejected { entity, .. } => is_self(*entity),
        GameEvent::TrapTriggered { entity, at, .. } => is_self(*entity) || at_seen(*at),
        GameEvent::TerrainDamage { entity, .. } => is_self(*entity),
        GameEvent::DoorOpened { at, .. } => at_seen(*at),
        GameEvent::DoorClosed { at, .. } => at_seen(*at),
        GameEvent::LevelChanged { entity, .. } => is_self(*entity),
        GameEvent::MonsterMoved { from, to, .. } => at_seen(*from) || at_seen(*to),
        GameEvent::MonsterAttacked { target, .. } => is_self(*target),
        GameEvent::PlayerAttacked { entity, at, .. } => is_self(*entity) || at_seen(*at),
        GameEvent::MonsterKilled { entity, at, .. } => is_self(*entity) || at_seen(*at),
        GameEvent::ItemPickedUp { entity, at, .. } => is_self(*entity) || at_seen(*at),
        GameEvent::ItemDropped { entity, at, .. } => is_self(*entity) || at_seen(*at),
        GameEvent::Rested { entity, .. } => is_self(*entity),
        GameEvent::Dug { at, .. } => at_seen(*at),
        GameEvent::Placed { at, .. } => at_seen(*at),
        GameEvent::Scooped { at, .. } => at_seen(*at),
        GameEvent::Crafted { entity, .. } => is_self(*entity),
        GameEvent::Planted { at, .. } => at_seen(*at),
        GameEvent::Deconstructed { at, .. } => at_seen(*at),
        GameEvent::AgentDied { entity, at } => is_self(*entity) || at_seen(*at),
        GameEvent::AgentRespawned { entity, at } => is_self(*entity) || at_seen(*at),
    }
}
