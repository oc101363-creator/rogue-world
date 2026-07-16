//! Minimal frog-like monster turn: wander / chase / contact attack.
//!
//! Each monster targets the NEAREST agent (not just the first one spawned).

use bevy_ecs::prelude::*;

use crate::balance;
use crate::components::{Agent, Health, Monster, Position, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::f_info;
use crate::grid::Grid;
use crate::systems::{stable_id, step_toward};
use crate::world::WorldSeed;

/// Process all monsters once per tick (after agent actions).
pub fn process_monsters_system(world: &mut World) {
    let seed = world.get_resource::<WorldSeed>().map(|s| s.0).unwrap_or(1);
    let tick = world
        .get_resource::<crate::world::TickCounter>()
        .map(|t| t.0)
        .unwrap_or(0);

    let agents: Vec<(Entity, Position)> = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
        q.iter(world).map(|(e, p)| (e, *p)).collect()
    };
    if agents.is_empty() {
        return;
    }

    let monsters: Vec<(Entity, Position, String)> = {
        let mut q = world.query::<(Entity, &Position, &Monster)>();
        q.iter(world)
            .map(|(e, p, m)| (e, *p, m.name.clone()))
            .collect()
    };

    for (i, (mon_e, pos, name)) in monsters.into_iter().enumerate() {
        // skip if despawned mid-loop
        if world.get_entity(mon_e).is_err() {
            continue;
        }

        // nearest agent is the target
        let Some(&(agent_e, agent_pos)) = agents
            .iter()
            .min_by_key(|(_, p)| (p.x - pos.x).abs() + (p.y - pos.y).abs())
        else {
            continue;
        };
        let dist = (pos.x - agent_pos.x).abs() + (pos.y - agent_pos.y).abs();

        // contact: same cell or adjacent → attack, no move
        if dist <= 1 {
            hit(world, mon_e, agent_e, &name);
            continue;
        }

        // chase if within range, else deterministic wander
        let (dx, dy) = if dist <= balance::MONSTER_CHASE_RANGE {
            let (mut mx, mut my) = step_toward(pos.x, pos.y, agent_pos.x, agent_pos.y);
            // diagonal approach: alternate axis preference per monster/tick
            if (agent_pos.x - pos.x).signum() != 0
                && (agent_pos.y - pos.y).signum() != 0
                && (tick as usize + i) % 2 == 0
            {
                mx = 0;
                my = (agent_pos.y - pos.y).signum();
            }
            (mx, my)
        } else {
            let sid = world.get::<StableId>(mon_e).map(|s| s.0).unwrap_or(0);
            let r = seed
                .wrapping_add(tick)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(sid);
            match r % 5 {
                0 => (-1, 0),
                1 => (1, 0),
                2 => (0, -1),
                3 => (0, 1),
                _ => (0, 0),
            }
        };

        if dx == 0 && dy == 0 {
            continue;
        }
        let nx = pos.x + dx;
        let ny = pos.y + dy;

        if !world.resource::<Grid>().walkable(nx, ny) {
            continue;
        }
        if f_info::table().is_closed_door(world.resource::<Grid>().get(nx, ny).unwrap_or(0)) {
            continue;
        }
        let blocked = {
            let mut q = world.query_filtered::<&Position, With<Monster>>();
            q.iter(world).any(|p| p.x == nx && p.y == ny)
        };
        if blocked {
            continue;
        }
        // stepping onto ANY agent's cell = attack that agent instead
        if let Some(&(victim_e, _)) = agents.iter().find(|(_, p)| p.x == nx && p.y == ny) {
            hit(world, mon_e, victim_e, &name);
            continue;
        }

        let mid = stable_id(world, mon_e);
        if let Some(mut p) = world.get_mut::<Position>(mon_e) {
            p.x = nx;
            p.y = ny;
        }
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::MonsterMoved {
                entity: mid,
                from: (pos.x, pos.y),
                to: (nx, ny),
            });
    }
}

/// The ONE place a monster hits an agent.
fn hit(world: &mut World, mon_e: Entity, agent_e: Entity, name: &str) {
    let damage = balance::MONSTER_DAMAGE;
    if let Some(mut hp) = world.get_mut::<Health>(agent_e) {
        hp.damage(damage);
    }
    let thp = world.get::<Health>(agent_e).map(|h| h.hp).unwrap_or(0);
    let mid = stable_id(world, mon_e);
    let aid = stable_id(world, agent_e);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::MonsterAttacked {
            monster: mid,
            target: aid,
            damage,
            target_hp: thp,
            name: name.to_string(),
        });
}
