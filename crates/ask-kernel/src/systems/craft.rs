//! Craft / plant / deconstruct — pack recipes & world edits.

use bevy_ecs::prelude::*;

use crate::components::{
    Building, Glyph, Inventory, Matter, Position, Resource, ResourceKind, StableId,
};
use crate::events::{EventBuf, GameEvent};
use crate::f_info::id;
use crate::grid::Grid;
use crate::sandbox::{
    self, can_craft, expand_output, need_required, pack_view, recipe_by_id, recipes, RecipeNeed,
};
use crate::systems::stable_id;

fn consume_need(inv: &mut Inventory, need: &RecipeNeed) -> bool {
    let n = need_required(need);
    match *need {
        RecipeNeed::Wood(_) => inv.remove_resource(ResourceKind::Wood, n),
        RecipeNeed::Iron(_) => inv.remove_resource(ResourceKind::Iron, n),
        RecipeNeed::Terrain(feat, _) => inv.remove_terrain(feat, n),
        RecipeNeed::AnyRock(_) => sandbox::remove_any_rock(inv, n),
        RecipeNeed::AnyTerrain(_) => {
            inv.remove_matching(|m| matches!(m, Matter::Terrain { .. }), n)
        }
    }
}

pub fn apply_craft(world: &mut World, agent: Entity, recipe_id: &str) {
    let eid = stable_id(world, agent);
    let Some(recipe) = recipe_by_id(recipe_id) else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: format!("unknown_recipe:{recipe_id}"),
            });
        return;
    };

    let view = world
        .get::<Inventory>(agent)
        .map(|i| pack_view(&i.slots))
        .unwrap_or_default();
    if !can_craft(&view, recipe) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: format!("missing_ingredients:{}", recipe.id),
            });
        return;
    }

    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        for need in recipe.needs {
            if !consume_need(&mut inv, need) {
                world
                    .resource_mut::<EventBuf>()
                    .push(GameEvent::ActionRejected {
                        entity: eid,
                        reason: "craft_consume_failed".into(),
                    });
                return;
            }
        }
        for (matter, qty) in expand_output(&recipe.output) {
            inv.add(matter, qty);
        }
    }

    world.resource_mut::<EventBuf>().push(GameEvent::Crafted {
        entity: eid,
        recipe: recipe.id.into(),
        label: recipe.label(),
    });
}

/// Plant: place TREE feat from wood (1) or from Terrain TREE stack; spawn harvestable Resource.
pub fn apply_plant(world: &mut World, agent: Entity, dx: i32, dy: i32) {
    let eid = stable_id(world, agent);
    if !((dx == 0 && dy == 0) || dx.abs() + dy.abs() == 1) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "plant_range".into(),
            });
        return;
    }
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let Some(cur) = world.resource::<Grid>().get(tx, ty) else {
        return;
    };
    if let Err(r) = sandbox::can_place_on(cur, id::TREE, dx == 0 && dy == 0) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: r.into(),
            });
        return;
    }

    // prefer Terrain TREE stack, else spend 1 wood
    let paid = {
        let Some(mut inv) = world.get_mut::<Inventory>(agent) else {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "no_inventory".into(),
                });
            return;
        };
        if inv.remove_terrain(id::TREE, 1) {
            true
        } else if inv.remove_resource(ResourceKind::Wood, 1) {
            true
        } else {
            false
        }
    };
    if !paid {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "need_wood_or_tree_block".into(),
            });
        return;
    }

    world.resource_mut::<Grid>().set(tx, ty, id::TREE);

    // harvestable entity if none here
    let has = crate::spatial::any_at(world, tx, ty, |w, e| {
        w.get::<Resource>(e)
            .map(|r| r.kind == ResourceKind::Wood)
            .unwrap_or(false)
    });
    if !has {
        let idn = crate::world::next_id(world);
        world.spawn((
            Position { x: tx, y: ty },
            Glyph('T'),
            Resource {
                kind: ResourceKind::Wood,
                amount: 3,
            },
            StableId(idn),
        ));
    }

    world.resource_mut::<EventBuf>().push(GameEvent::Planted {
        entity: eid,
        at: (tx, ty),
    });
}

/// Deconstruct hut → wood back to pack.
pub fn apply_deconstruct(world: &mut World, agent: Entity, dx: i32, dy: i32) {
    let eid = stable_id(world, agent);
    if !((dx == 0 && dy == 0) || dx.abs() + dy.abs() == 1) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "deconstruct_range".into(),
            });
        return;
    }
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);

    let target = crate::spatial::find_at(world, tx, ty, |w, e| w.get::<Building>(e).is_some());
    let Some(be) = target else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "no_building".into(),
            });
        return;
    };

    let cost = world
        .get_resource::<crate::world::KernelConfig>()
        .map(|c| c.hut_wood_cost)
        .unwrap_or(3);

    world.despawn(be);
    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        inv.add(
            Matter::Resource {
                resource: ResourceKind::Wood,
            },
            cost,
        );
    }
    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::Deconstructed {
            entity: eid,
            at: (tx, ty),
            wood: cost,
        });
}

/// List craft options currently affordable.
pub fn list_crafts(world: &World, agent: Entity) -> Vec<(String, String)> {
    let view = world
        .get::<Inventory>(agent)
        .map(|i| pack_view(&i.slots))
        .unwrap_or_default();
    recipes()
        .iter()
        .filter(|r| can_craft(&view, r))
        .map(|r| (r.id.to_string(), r.label()))
        .collect()
}

pub fn can_plant(world: &World, agent: Entity) -> bool {
    world
        .get::<Inventory>(agent)
        .map(|i| i.qty_terrain(id::TREE) > 0 || i.wood() > 0)
        .unwrap_or(false)
}
