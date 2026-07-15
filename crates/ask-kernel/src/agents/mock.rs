//! Rule policy — stand-in until external Agent Gateway exists.

use bevy_ecs::prelude::*;

use crate::actions::Action;
use crate::agents::AgentPolicy;
use crate::components::{Agent, Building, Inventory, Position, Resource, ResourceKind};
use crate::grid::Grid;
use crate::world::KernelConfig;

#[derive(Default)]
pub struct MockPolicy;

impl AgentPolicy for MockPolicy {
    fn decide(&mut self, world: &mut World, agent: Entity) -> Action {
        let Some(pos) = world.get::<Position>(agent).copied() else {
            return Action::Idle;
        };
        let wood = world
            .get::<Inventory>(agent)
            .map(|i| i.wood)
            .unwrap_or(0);
        let cost = world.resource::<KernelConfig>().hut_wood_cost;

        // Harvest if standing on resource
        {
            let mut q = world.query::<(&Position, &Resource)>();
            for (p, r) in q.iter(world) {
                if p.x == pos.x && p.y == pos.y && r.amount > 0 {
                    return Action::Harvest;
                }
            }
        }

        // Build hut if enough wood and no building here
        if wood >= cost {
            let mut has_b = false;
            let mut q = world.query::<(&Position, &Building)>();
            for (p, _) in q.iter(world) {
                if p.x == pos.x && p.y == pos.y {
                    has_b = true;
                    break;
                }
            }
            if !has_b && world.resource::<Grid>().buildable(pos.x, pos.y) {
                return Action::BuildHut;
            }
        }

        // Move toward nearest tree
        let mut best: Option<(i32, i32, i32)> = None; // dist, x, y
        {
            let mut q = world.query::<(&Position, &Resource)>();
            for (p, r) in q.iter(world) {
                if r.kind != ResourceKind::Wood || r.amount == 0 {
                    continue;
                }
                let d = (p.x - pos.x).abs() + (p.y - pos.y).abs();
                if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                    best = Some((d, p.x, p.y));
                }
            }
        }

        if let Some((_, tx, ty)) = best {
            let (dx, dy) = step_toward(pos.x, pos.y, tx, ty);
            let nx = pos.x + dx;
            let ny = pos.y + dy;
            if world.resource::<Grid>().walkable(nx, ny) {
                return Action::Move { dx, dy };
            }
            // try alternate axis
            let (dx2, dy2) = if dx != 0 {
                (0, if ty > pos.y { 1 } else if ty < pos.y { -1 } else { 0 })
            } else {
                (if tx > pos.x { 1 } else if tx < pos.x { -1 } else { 0 }, 0)
            };
            if dx2 != 0 || dy2 != 0 {
                let nx = pos.x + dx2;
                let ny = pos.y + dy2;
                if world.resource::<Grid>().walkable(nx, ny) {
                    return Action::Move { dx: dx2, dy: dy2 };
                }
            }
        }

        let _ = world; // keep agent marker used
        let _: Option<&Agent> = world.get::<Agent>(agent);
        Action::Idle
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
