//! Minimal frog-like monster turn: wander / chase / contact attack.

use bevy_ecs::prelude::*;

use crate::components::{Agent, Health, Monster, Position, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::f_info;
use crate::grid::Grid;
use crate::systems::stable_id;
use crate::world::WorldSeed;

/// Process all monsters once per tick (after agent actions).
pub fn process_monsters_system(world: &mut World) {
    let seed = world.get_resource::<WorldSeed>().map(|s| s.0).unwrap_or(1);
    let tick = world
        .get_resource::<crate::world::TickCounter>()
        .map(|t| t.0)
        .unwrap_or(0);

    // agent position
    let agent = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
        q.iter(world).next().map(|(e, p)| (e, *p))
    };
    let Some((agent_e, agent_pos)) = agent else {
        return;
    };

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

        let dist = (pos.x - agent_pos.x).abs() + (pos.y - agent_pos.y).abs();

        // contact attack
        if dist == 0 || dist == 1 {
            // if adjacent, step onto agent cell = attack instead of move
            if dist == 1 {
                // try move onto agent → attack
                let dx = (agent_pos.x - pos.x).signum();
                let dy = (agent_pos.y - pos.y).signum();
                // prefer axis move
                let (mx, my) = if dx != 0 && dy != 0 {
                    if (tick as usize + i) % 2 == 0 {
                        (dx, 0)
                    } else {
                        (0, dy)
                    }
                } else {
                    (dx, dy)
                };
                let nx = pos.x + mx;
                let ny = pos.y + my;
                if nx == agent_pos.x && ny == agent_pos.y {
                    let damage = 2;
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
                            name,
                        });
                    continue;
                }
            } else if dist == 0 {
                let damage = 2;
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
                        name,
                    });
                continue;
            }
        }

        // chase if within 8, else wander
        let (dx, dy) = if dist <= 8 {
            step_toward(pos.x, pos.y, agent_pos.x, agent_pos.y)
        } else {
            // deterministic wander from seed+tick+id
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

        // don't walk into other monsters / blocked / closed doors
        if !world.resource::<Grid>().walkable(nx, ny) {
            continue;
        }
        if f_info::table().is_closed_door(world.resource::<Grid>().get(nx, ny).unwrap_or(0)) {
            continue;
        }
        // occupied by another monster?
        let blocked = {
            let mut q = world.query_filtered::<&Position, With<Monster>>();
            q.iter(world).any(|p| p.x == nx && p.y == ny)
        };
        if blocked {
            continue;
        }
        // if stepping onto agent → attack instead
        if nx == agent_pos.x && ny == agent_pos.y {
            let damage = 2;
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
                    name,
                });
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

fn step_toward(x: i32, y: i32, tx: i32, ty: i32) -> (i32, i32) {
    let dx = tx - x;
    let dy = ty - y;
    if dx.abs() >= dy.abs() && dx != 0 {
        (dx.signum(), 0)
    } else if dy != 0 {
        (0, dy.signum())
    } else {
        (0, 0)
    }
}
