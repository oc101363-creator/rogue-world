//! Event buffer — Frog notice_stuff batching.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Agent, EventInbox, StableId};

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
    /// World process transformed a cell (fire/water/growth).
    TerrainChanged {
        at: (i32, i32),
        from: u16,
        to: u16,
        cause: crate::process_rules::Cause,
    },
    /// Agent consumed an organic block for hp.
    Consumed {
        entity: u64,
        label: String,
        hp: i32,
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

/// Route this tick's EventBuf into every agent's personal EventInbox,
/// filtered by what THAT agent can perceive right now (push-time FOV:
/// you learn what happened where you were looking when it happened —
/// read-time filtering would leak or hide events as vision changes).
///
/// Runs once per tick (tick.rs, after vision settles). Cost is
/// events × agents × a 41×41 FOV — trivial at server scale.
pub fn distribute_feedback(world: &mut World) {
    if world.resource::<EventBuf>().events.is_empty() {
        return;
    }
    let tick = world.resource::<crate::world::TickCounter>().0;
    let events = world.resource::<EventBuf>().events.clone();
    let agents: Vec<(Entity, u64)> = {
        let mut q = world.query_filtered::<(Entity, &StableId), With<Agent>>();
        q.iter(world).map(|(e, s)| (e, s.0)).collect()
    };
    for (e, sid) in agents {
        let vis = crate::vision::compute_view_for_agents(world, &[e]);
        let perceivable: Vec<GameEvent> = events
            .iter()
            .filter(|ev| event_visible(ev, &vis, Some(sid)))
            .cloned()
            .collect();
        if perceivable.is_empty() {
            continue;
        }
        if let Some(mut inbox) = world.get_mut::<EventInbox>(e) {
            for ev in perceivable {
                inbox.push(tick, ev);
            }
        }
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
        // autonomous world edits: visible-only (memory cells must not become
        // a live intel feed — see VisionMemory remembered-terrain rule)
        GameEvent::TerrainChanged { at, .. } => vis.display_class(at.0, at.1) == 2,
        GameEvent::Consumed { entity, .. } => is_self(*entity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::VisionMap;

    #[test]
    fn terrain_changed_visibility_rules() {
        let mut vis = VisionMap::new(10, 10);
        vis.flags[(2 * 10 + 3) as usize] = crate::vision::F_VIEW | crate::vision::F_LITE;
        let ev = GameEvent::TerrainChanged {
            at: (3, 2),
            from: 96,
            to: 99,
            cause: crate::process_rules::Cause::Fire,
        };
        assert!(event_visible(&ev, &vis, None));
        let far = GameEvent::TerrainChanged {
            at: (8, 8),
            from: 96,
            to: 99,
            cause: crate::process_rules::Cause::Fire,
        };
        assert!(!event_visible(&far, &vis, None));
        let ate = GameEvent::Consumed { entity: 7, label: "GRASS".into(), hp: 5 };
        assert!(event_visible(&ate, &vis, Some(7)));
        assert!(!event_visible(&ate, &vis, Some(8)));
    }
}
