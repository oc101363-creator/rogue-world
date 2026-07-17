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

        let dest = world.resource::<Grid>().get(nx, ny).unwrap_or(0);
        let swim_ok = world
            .get::<Monster>(mon_e)
            .and_then(|m| crate::r_info::table().get(m.race_id))
            .map(|r| r.can_swim && dest == crate::f_info::id::DEEP_WATER)
            .unwrap_or(false);
        if !world.resource::<Grid>().walkable(nx, ny) && !swim_ok {
            continue;
        }
        if f_info::table().is_closed_door(world.resource::<Grid>().get(nx, ny).unwrap_or(0)) {
            continue;
        }
        if crate::spatial::any_at(world, nx, ny, |w, e| w.get::<Monster>(e).is_some()) {
            continue;
        }
        // stepping onto ANY agent's cell = attack that agent instead
        if let Some(&(victim_e, _)) = agents.iter().find(|(_, p)| p.x == nx && p.y == ny) {
            hit(world, mon_e, victim_e, &name);
            continue;
        }

        monster_move_to(world, mon_e, nx, ny);
    }
}

/// Move a monster onto (nx, ny) and apply THE SAME terrain rules agents get
/// (traps, lava, deep water). If the terrain kills it, despawn + event.
pub fn monster_move_to(world: &mut World, mon_e: Entity, nx: i32, ny: i32) {
    let Some(pos) = world.get::<Position>(mon_e).copied() else {
        return;
    };
    if let Some(mut p) = world.get_mut::<Position>(mon_e) {
        p.x = nx;
        p.y = ny;
    }
    let mid = stable_id(world, mon_e);
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::MonsterMoved {
            entity: mid,
            from: (pos.x, pos.y),
            to: (nx, ny),
        });

    let (can_swim, res_fire) = world
        .get::<Monster>(mon_e)
        .and_then(|m| crate::r_info::table().get(m.race_id))
        .map(|r| (r.can_swim, r.res_fire))
        .unwrap_or((false, false));
    let feat = world.resource::<Grid>().get(nx, ny).unwrap_or(0);
    let info = crate::f_info::table().get(feat);
    let lava_immune = res_fire && info.map(|f| f.lava).unwrap_or(false);
    let water_immune = can_swim && feat == crate::f_info::id::DEEP_WATER;
    if !lava_immune && !water_immune {
        crate::systems::terrain::on_enter_cell(world, mon_e, nx, ny);
    }

    // terrain death
    let dead = world.get::<Health>(mon_e).map(|h| h.hp <= 0).unwrap_or(false);
    if dead {
        let name = world
            .get::<Monster>(mon_e)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "monster".into());
        world.despawn(mon_e);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::MonsterKilled {
                entity: 0, // no killer — the world did it
                monster: mid,
                name,
                at: (nx, ny),
            });
    }
}

/// The ONE place a monster hits an agent.
/// Damage comes from the monster's race (r_info); balance is the fallback.
fn hit(world: &mut World, mon_e: Entity, agent_e: Entity, name: &str) {
    let damage = world
        .get::<Monster>(mon_e)
        .and_then(|m| crate::r_info::table().get(m.race_id))
        .and_then(|r| r.damage)
        .unwrap_or(balance::MONSTER_DAMAGE);
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
