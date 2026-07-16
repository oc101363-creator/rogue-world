//! Rule policy — stand-in until external Agent Gateway exists.
//! Uses Interact verbs discovered from the world (not hard-coded special actions).

use bevy_ecs::prelude::*;

use crate::actions::Action;
use crate::agents::AgentPolicy;
use crate::components::{Agent, Inventory, Position, Resource, ResourceKind};
use crate::grid::Grid;
use crate::systems::interact;
use crate::world::KernelConfig;

#[derive(Default)]
pub struct MockPolicy;

impl AgentPolicy for MockPolicy {
    fn decide(&mut self, world: &mut World, agent: Entity) -> Action {
        let Some(pos) = world.get::<Position>(agent).copied() else {
            return Action::Idle;
        };

        // Prefer productive underfoot verbs (avoid endless scoop/craft)
        let under = interact::list_at(world, agent, 0, 0);
        const PREFER: &[&str] = &["harvest", "pickup", "build", "plant"];
        if let Some(i) = PREFER
            .iter()
            .find_map(|v| under.iter().find(|o| o.verb == *v))
        {
            return Action::Interact {
                dx: 0,
                dy: 0,
                verb: Some(i.verb.clone()),
                slot: i.slot,
                recipe: i.recipe.clone(),
            };
        }

        // Move toward nearest tree
        let mut best: Option<(i32, i32, i32)> = None;
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
            let (dx2, dy2) = if dx != 0 {
                (
                    0,
                    if ty > pos.y {
                        1
                    } else if ty < pos.y {
                        -1
                    } else {
                        0
                    },
                )
            } else {
                (
                    if tx > pos.x {
                        1
                    } else if tx < pos.x {
                        -1
                    } else {
                        0
                    },
                    0,
                )
            };
            if dx2 != 0 || dy2 != 0 {
                let nx = pos.x + dx2;
                let ny = pos.y + dy2;
                if world.resource::<Grid>().walkable(nx, ny) {
                    return Action::Move { dx: dx2, dy: dy2 };
                }
            }
        }

        // If we have wood, try build underfoot when list_at didn't (e.g. cost edge)
        let _ = world.get::<Inventory>(agent);
        let _ = world.get_resource::<KernelConfig>();
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
