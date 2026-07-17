//! Verb registry — THE one place verbs are defined.
//!
//! Discovery ("what can I do here?") is context computation and lives in
//! `interact::list_at`. Everything else about a verb — its name, priority,
//! docs, and how it executes — is this table. `apply_interact` dispatches
//! through it, the priority fallback order derives from it, and
//! `/api/catalog` is generated from it. Adding a verb = one row here +
//! discovery lines in list_at. Nothing else to sync.

use bevy_ecs::prelude::*;

use crate::events::{EventBuf, GameEvent};
use crate::systems::build::apply_build_hut;
use crate::systems::combat::apply_attack;
use crate::systems::craft::{apply_craft, apply_deconstruct, apply_plant};
use crate::systems::dig::{apply_dig, apply_place, apply_scoop};
use crate::systems::harvest::apply_harvest;
use crate::systems::inventory_act::apply_pickup;
use crate::systems::stable_id;
use crate::systems::terrain::{apply_close_door, apply_open_door, apply_use_stairs};

/// A resolved interaction call (target cell + optional parameters).
pub struct VerbCall {
    pub dx: i32,
    pub dy: i32,
    pub slot: Option<usize>,
    pub recipe: Option<String>,
}

pub struct VerbSpec {
    pub verb: &'static str,
    /// Fallback priority when the request names no verb (lower wins).
    pub priority: u8,
    /// One-line doc for /api/catalog.
    pub doc: &'static str,
    pub apply: fn(&mut World, Entity, &VerbCall),
}

fn reject(world: &mut World, agent: Entity, reason: &str) {
    let eid = stable_id(world, agent);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::ActionRejected {
            entity: eid,
            reason: reason.into(),
        });
}

fn underfoot_only(world: &mut World, agent: Entity, c: &VerbCall, verb: &str) -> bool {
    if c.dx != 0 || c.dy != 0 {
        reject(world, agent, &format!("{verb}_underfoot_only"));
        return false;
    }
    true
}

pub fn registry() -> &'static [VerbSpec] {
    &[
        VerbSpec {
            verb: "attack",
            priority: 0,
            doc: "melee an adjacent monster",
            apply: |w, a, c| apply_attack(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "harvest",
            priority: 1,
            doc: "take 1 from the resource underfoot",
            apply: |w, a, c| {
                if underfoot_only(w, a, c, "harvest") {
                    apply_harvest(w, a);
                }
            },
        },
        VerbSpec {
            verb: "pickup",
            priority: 2,
            doc: "pick up ground items underfoot",
            apply: |w, a, c| {
                if underfoot_only(w, a, c, "pickup") {
                    apply_pickup(w, a);
                }
            },
        },
        VerbSpec {
            verb: "open",
            priority: 3,
            doc: "open a closed door (adjacent)",
            apply: |w, a, c| apply_open_door(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "close",
            priority: 4,
            doc: "close an open door (adjacent, unoccupied)",
            apply: |w, a, c| apply_close_door(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "descend",
            priority: 5,
            doc: "take stairs down (underfoot)",
            apply: |w, a, _| apply_use_stairs(w, a, true),
        },
        VerbSpec {
            verb: "ascend",
            priority: 6,
            doc: "take stairs up (underfoot)",
            apply: |w, a, _| apply_use_stairs(w, a, false),
        },
        VerbSpec {
            verb: "dig",
            priority: 7,
            doc: "dig hard rock (adjacent) → pack, leaves rubble/floor",
            apply: |w, a, c| apply_dig(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "scoop",
            priority: 8,
            doc: "scoop soft surface → pack, leaves successor",
            apply: |w, a, c| apply_scoop(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "place",
            priority: 9,
            doc: "place a terrain block from pack (slot=index)",
            apply: |w, a, c| apply_place(w, a, c.dx, c.dy, c.slot),
        },
        VerbSpec {
            verb: "plant",
            priority: 10,
            doc: "plant a tree from wood/tree block",
            apply: |w, a, c| apply_plant(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "build",
            priority: 11,
            doc: "build a hut underfoot (wood cost)",
            apply: |w, a, _| apply_build_hut(w, a),
        },
        VerbSpec {
            verb: "deconstruct",
            priority: 12,
            doc: "tear down a hut → wood refund",
            apply: |w, a, c| apply_deconstruct(w, a, c.dx, c.dy),
        },
        VerbSpec {
            verb: "craft",
            priority: 13,
            doc: "transform pack via recipe (recipe=id)",
            apply: |w, a, c| match &c.recipe {
                Some(rid) => apply_craft(w, a, rid),
                None => reject(w, a, "craft_needs_recipe"),
            },
        },
        VerbSpec {
            verb: "use",
            priority: 14,
            doc: "use a pack block (slot=index): ignite flammable / eat organic",
            apply: |w, a, c| crate::systems::use_item::apply_use(w, a, c.slot, c.dx, c.dy),
        },
    ]
}

pub fn lookup(verb: &str) -> Option<&'static VerbSpec> {
    registry().iter().find(|v| v.verb == verb)
}

/// Verb names in fallback-priority order (for /api/catalog).
pub fn catalog_verbs() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<_> = registry()
        .iter()
        .map(|s| (s.verb, s.doc))
        .collect();
    v.sort_by_key(|(verb, _)| lookup(verb).map(|s| s.priority).unwrap_or(255));
    v
}

/// Pick the highest-priority verb among discovered options (no verb given).
pub fn pick_by_priority<'a>(
    options: &'a [crate::actions::Interaction],
) -> Option<&'a crate::actions::Interaction> {
    options.iter().min_by_key(|o| {
        lookup(&o.verb).map(|s| s.priority).unwrap_or(255)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Interaction;

    #[test]
    fn catalog_is_priority_ordered_and_complete() {
        let v = catalog_verbs();
        assert!(v.len() >= 14, "all verbs registered");
        let names: Vec<_> = v.iter().map(|(n, _)| *n).collect();
        assert_eq!(names[0], "attack");
        assert!(names.contains(&"craft"));
        // docs present
        assert!(v.iter().all(|(_, d)| !d.is_empty()));
        // strictly priority-ordered
        let prios: Vec<_> = names.iter().map(|n| lookup(n).unwrap().priority).collect();
        let mut sorted = prios.clone();
        sorted.sort();
        assert_eq!(prios, sorted);
    }

    #[test]
    fn priority_pick_prefers_attack_over_craft() {
        let opts = vec![
            Interaction {
                dx: 0,
                dy: 0,
                verb: "craft".into(),
                label: "craft door".into(),
                target_id: None,
                slot: None,
                recipe: Some("plank_door".into()),
            },
            Interaction {
                dx: 1,
                dy: 0,
                verb: "attack".into(),
                label: "attack rat".into(),
                target_id: Some(9),
                slot: None,
                recipe: None,
            },
        ];
        assert_eq!(pick_by_priority(&opts).map(|o| o.verb.as_str()), Some("attack"));
    }
}
