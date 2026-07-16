//! Unified Interact — options from cell ECS + terrain + pack + recipes.

use bevy_ecs::prelude::*;

use crate::actions::Interaction;
use crate::components::{Building, Inventory, Item, Matter, Monster, Position, Resource, StableId};
use crate::events::{EventBuf, GameEvent};
use crate::f_info;
use crate::grid::Grid;
use crate::sandbox;
use crate::systems::build::apply_build_hut;
use crate::systems::combat::apply_attack;
use crate::systems::craft::{apply_craft, apply_deconstruct, apply_plant, can_plant, list_crafts};
use crate::systems::dig::{apply_dig, apply_place, apply_scoop, is_diggable, is_scoopable};
use crate::systems::harvest::apply_harvest;
use crate::systems::inventory_act::apply_pickup;
use crate::systems::stable_id;
use crate::systems::terrain::{apply_close_door, apply_open_door, apply_use_stairs};
use crate::world::KernelConfig;

pub fn list_at(world: &mut World, agent: Entity, dx: i32, dy: i32) -> Vec<Interaction> {
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return Vec::new();
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let underfoot = dx == 0 && dy == 0;
    let mut out = Vec::new();

    // One spatial scan; dispatch on components per entity found.
    for e in crate::spatial::at(world, tx, ty) {
        let sid = world.get::<StableId>(e).map(|s| s.0);
        if let Some(m) = world.get::<Monster>(e) {
            out.push(Interaction {
                dx,
                dy,
                verb: "attack".into(),
                label: format!("attack {}", m.name),
                target_id: sid,
                slot: None,
                recipe: None,
            });
        }
        if let Some(r) = world.get::<Resource>(e) {
            if r.amount > 0 {
                let kind = match r.kind {
                    crate::components::ResourceKind::Wood => "wood",
                    crate::components::ResourceKind::Iron => "iron",
                };
                out.push(Interaction {
                    dx,
                    dy,
                    verb: "harvest".into(),
                    label: format!("harvest {kind} ({})", r.amount),
                    target_id: sid,
                    slot: None,
                    recipe: None,
                });
            }
        }
        if let Some(it) = world.get::<Item>(e) {
            out.push(Interaction {
                dx,
                dy,
                verb: "pickup".into(),
                label: format!("pick up {}", it.name()),
                target_id: sid,
                slot: None,
                recipe: None,
            });
        }
        if world.get::<Building>(e).is_some() {
            out.push(Interaction {
                dx,
                dy,
                verb: "deconstruct".into(),
                label: "deconstruct hut".into(),
                target_id: sid,
                slot: None,
                recipe: None,
            });
        }
    }

    let Some(feat) = world.resource::<Grid>().get(tx, ty) else {
        return out;
    };
    let table = f_info::table();
    let info = table.get(feat);

    if table.is_closed_door(feat) {
        out.push(Interaction {
            dx,
            dy,
            verb: "open".into(),
            label: "open door".into(),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }
    if table.is_open_door(feat) {
        out.push(Interaction {
            dx,
            dy,
            verb: "close".into(),
            label: "close door".into(),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }
    if info.map(|f| f.more).unwrap_or(false) {
        out.push(Interaction {
            dx,
            dy,
            verb: "descend".into(),
            label: "go down stairs".into(),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }
    if info.map(|f| f.less).unwrap_or(false) {
        out.push(Interaction {
            dx,
            dy,
            verb: "ascend".into(),
            label: "go up stairs".into(),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }
    if is_diggable(feat) && !underfoot {
        out.push(Interaction {
            dx,
            dy,
            verb: "dig".into(),
            label: format!("dig {}", info.map(|f| f.name.as_str()).unwrap_or("rock")),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }
    if is_scoopable(feat) {
        out.push(Interaction {
            dx,
            dy,
            verb: "scoop".into(),
            label: format!(
                "scoop {}",
                info.map(|f| f.name.as_str()).unwrap_or("surface")
            ),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }

    // place every terrain stack that can go here
    if let Some(inv) = world.get::<Inventory>(agent) {
        for (si, stack) in inv.slots.iter().enumerate() {
            if let Matter::Terrain { feat: pf } = &stack.matter {
                if sandbox::can_place_on(feat, *pf, underfoot).is_ok() {
                    out.push(Interaction {
                        dx,
                        dy,
                        verb: "place".into(),
                        label: format!("place {}", stack.matter.label()),
                        target_id: None,
                        slot: Some(si),
                        recipe: None,
                    });
                }
            }
        }
    }

    if can_plant(world, agent)
        && sandbox::can_place_on(feat, crate::f_info::id::TREE, underfoot).is_ok()
    {
        out.push(Interaction {
            dx,
            dy,
            verb: "plant".into(),
            label: "plant tree".into(),
            target_id: None,
            slot: None,
            recipe: None,
        });
    }

    if underfoot && world.resource::<Grid>().buildable(tx, ty) {
        let occupied = crate::spatial::any_at(world, tx, ty, |w, e| w.get::<Building>(e).is_some());
        let wood = world.get::<Inventory>(agent).map(|i| i.wood()).unwrap_or(0);
        let cost = world
            .get_resource::<KernelConfig>()
            .map(|c| c.hut_wood_cost)
            .unwrap_or(5);
        if !occupied && wood >= cost {
            out.push(Interaction {
                dx,
                dy,
                verb: "build".into(),
                label: format!("build hut ({cost} wood)"),
                target_id: None,
                slot: None,
                recipe: None,
            });
        }
    }

    // craft is underfoot-only (self)
    if underfoot {
        for (id, label) in list_crafts(world, agent) {
            out.push(Interaction {
                dx: 0,
                dy: 0,
                verb: "craft".into(),
                label,
                target_id: None,
                slot: None,
                recipe: Some(id),
            });
        }
    }

    out
}

pub fn list_nearby(world: &mut World, agent: Entity) -> Vec<Interaction> {
    let mut all = Vec::new();
    for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
        all.extend(list_at(world, agent, dx, dy));
    }
    // de-dupe craft noise: keep all (agent needs recipe list)
    all
}

pub fn apply_interact(
    world: &mut World,
    agent: Entity,
    dx: i32,
    dy: i32,
    verb: Option<String>,
    slot: Option<usize>,
    recipe: Option<String>,
) {
    let eid = stable_id(world, agent);

    // craft can use recipe without matching list scan
    if verb.as_deref() == Some("craft") {
        if let Some(rid) = recipe {
            apply_craft(world, agent, &rid);
            return;
        }
    }

    if !((dx == 0 && dy == 0) || dx.abs() + dy.abs() == 1) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "interact_range".into(),
            });
        return;
    }

    let options = list_at(world, agent, dx, dy);
    if options.is_empty() && verb.as_deref() != Some("craft") {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "nothing_to_do".into(),
            });
        return;
    }

    let chosen = if let Some(v) = verb.as_deref() {
        if v == "craft" {
            if let Some(ref rid) = recipe {
                options
                    .iter()
                    .find(|o| o.verb == "craft" && o.recipe.as_deref() == Some(rid))
                    .cloned()
                    .or_else(|| {
                        Some(Interaction {
                            dx,
                            dy,
                            verb: "craft".into(),
                            label: rid.clone(),
                            target_id: None,
                            slot: None,
                            recipe: Some(rid.clone()),
                        })
                    })
            } else {
                options.iter().find(|o| o.verb == "craft").cloned()
            }
        } else if v == "place" {
            if let Some(si) = slot {
                options
                    .iter()
                    .find(|o| o.verb == "place" && o.slot == Some(si))
                    .cloned()
                    .or_else(|| options.iter().find(|o| o.verb == "place").cloned())
            } else {
                options.iter().find(|o| o.verb == "place").cloned()
            }
        } else {
            options.iter().find(|o| o.verb == v).cloned()
        }
    } else if options.len() == 1 {
        Some(options[0].clone())
    } else {
        const ORDER: &[&str] = &[
            "attack",
            "harvest",
            "pickup",
            "open",
            "close",
            "descend",
            "ascend",
            "dig",
            "scoop",
            "place",
            "plant",
            "build",
            "deconstruct",
            "craft",
        ];
        ORDER
            .iter()
            .find_map(|v| options.iter().find(|o| o.verb == *v).cloned())
    };

    let Some(choice) = chosen else {
        let verbs: Vec<_> = options.iter().map(|o| o.verb.clone()).collect();
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: format!("ambiguous:{}", verbs.join(",")),
            });
        return;
    };

    let place_slot = slot.or(choice.slot);
    let craft_id = recipe.or(choice.recipe);

    match choice.verb.as_str() {
        "attack" => apply_attack(world, agent, dx, dy),
        "harvest" => {
            if dx != 0 || dy != 0 {
                world
                    .resource_mut::<EventBuf>()
                    .push(GameEvent::ActionRejected {
                        entity: eid,
                        reason: "harvest_underfoot_only".into(),
                    });
            } else {
                apply_harvest(world, agent);
            }
        }
        "pickup" => {
            if dx != 0 || dy != 0 {
                world
                    .resource_mut::<EventBuf>()
                    .push(GameEvent::ActionRejected {
                        entity: eid,
                        reason: "pickup_underfoot_only".into(),
                    });
            } else {
                apply_pickup(world, agent);
            }
        }
        "open" => apply_open_door(world, agent, dx, dy),
        "close" => apply_close_door(world, agent, dx, dy),
        "descend" => apply_use_stairs(world, agent, true),
        "ascend" => apply_use_stairs(world, agent, false),
        "dig" => apply_dig(world, agent, dx, dy),
        "scoop" => apply_scoop(world, agent, dx, dy),
        "place" => apply_place(world, agent, dx, dy, place_slot),
        "plant" => apply_plant(world, agent, dx, dy),
        "build" => apply_build_hut(world, agent),
        "deconstruct" => apply_deconstruct(world, agent, dx, dy),
        "craft" => {
            if let Some(rid) = craft_id {
                apply_craft(world, agent, &rid);
            } else {
                world
                    .resource_mut::<EventBuf>()
                    .push(GameEvent::ActionRejected {
                        entity: eid,
                        reason: "craft_needs_recipe".into(),
                    });
            }
        }
        other => {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: format!("unknown_verb:{other}"),
                });
        }
    }
}
