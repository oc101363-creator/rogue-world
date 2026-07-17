//! Death & respawn — frog "you die" → drop pack, wake up elsewhere.
//!
//! An agent whose hp reaches 0 drops every pack stack on the spot, then
//! respawns at a random free cell with full hp and an empty pack.
//! Monsters are slain via combat (combat.rs) or terrain (monster.rs);
//! this system is about keeping agents alive *as persistent identities*
//! across death.

use bevy_ecs::prelude::*;

use crate::components::{Agent, Glyph, Health, Inventory, Item, Position, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;
use crate::world::random_free_cell;

pub fn check_deaths_system(world: &mut World) {
    let dead: Vec<(Entity, Position)> = {
        let mut q = world.query_filtered::<(Entity, &Position, &Health), With<Agent>>();
        q.iter(world)
            .filter(|(_, _, h)| h.hp <= 0)
            .map(|(e, p, _)| (e, *p))
            .collect()
    };

    for (agent_e, pos) in dead {
        let aid = stable_id(world, agent_e);

        // drop the whole pack on the death cell
        let stacks = world
            .get::<Inventory>(agent_e)
            .map(|inv| inv.slots.clone())
            .unwrap_or_default();
        for s in stacks {
            let id = crate::world::next_id(world);
            let glyph = s.matter.glyph();
            world.spawn((
                Position { x: pos.x, y: pos.y },
                Glyph(glyph),
                Item {
                    matter: s.matter,
                    qty: s.qty,
                },
                StableId(id),
            ));
        }
        if let Some(mut inv) = world.get_mut::<Inventory>(agent_e) {
            inv.slots.clear();
        }

        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::AgentDied {
                entity: aid,
                at: (pos.x, pos.y),
            });

        // respawn elsewhere, full hp
        let dest = random_free_cell(world, aid).unwrap_or((pos.x, pos.y));
        if let Some(mut p) = world.get_mut::<Position>(agent_e) {
            p.x = dest.0;
            p.y = dest.1;
        }
        if let Some(mut h) = world.get_mut::<Health>(agent_e) {
            h.hp = h.max_hp;
        }
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::AgentRespawned {
                entity: aid,
                at: dest,
            });
    }
}
