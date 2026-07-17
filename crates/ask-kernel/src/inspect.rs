//! On-demand inspect/details for entities and cells.
//!
//! The snapshot only sends lightweight glyph/position hints.
//! When the player clicks a character, the frontend fetches rich details from
//! here — one query per click, no redundant data in the live snapshot.

use bevy_ecs::prelude::*;
use serde_json::json;

use crate::components::{
    AgentProfile, Glyph, Health, Inventory, Item, Matter, Monster, Position, Resource, ResourceKind,
    StableId,
};
use crate::f_info;
use crate::grid::Grid;
use crate::k_info;
use crate::r_info;

/// Return a JSON value describing the entity with this `StableId`, or `None`
/// if not found.
pub fn entity_info(world: &mut World, id: u64) -> Option<serde_json::Value> {
    let mut found: Option<(Entity, &Position, &Glyph)> = None;
    {
        let mut q = world.query::<(Entity, &StableId, &Position, &Glyph)>();
        for (e, sid, p, g) in q.iter(world) {
            if sid.0 == id {
                found = Some((e, p, g));
                break;
            }
        }
    }
    let (entity, pos, glyph) = found?;

    // ONE classifier (describe) — no per-projection kind logic drift.
    let kind = crate::describe::EntityKind::classify(world, entity)?;
    let mut base = json!({
        "id": id,
        "x": pos.x,
        "y": pos.y,
        "glyph": glyph.0.to_string(),
        "kind": kind.as_str(),
    });

    match kind {
        crate::describe::EntityKind::Agent => {
            if let Some(profile) = world.get::<AgentProfile>(entity) {
                base["name"] = profile.name.clone().into();
                base["purpose"] = profile.purpose.clone().into();
            }
            if let Some(inv) = world.get::<Inventory>(entity) {
                base["pack"] = inv.to_api().into();
            }
            if let Some(hp) = world.get::<Health>(entity) {
                base["hp"] = hp.hp.into();
                base["max_hp"] = hp.max_hp.into();
            }
        }
        crate::describe::EntityKind::Monster => {
            let mon = world.get::<Monster>(entity).unwrap();
            base["name"] = mon.name.clone().into();
            base["race_id"] = mon.race_id.into();
            if let Some(hp) = world.get::<Health>(entity) {
                base["hp"] = hp.hp.into();
                base["max_hp"] = hp.max_hp.into();
            }
            if let Some(race) = r_info::table().get(mon.race_id) {
                base["source"] = json!({
                    "id": race.id,
                    "name": race.name,
                    "glyph": race.glyph.to_string(),
                    "color": race.color.to_string(),
                });
            }
        }
        crate::describe::EntityKind::Item => {
            let item = world.get::<Item>(entity).unwrap();
            base["name"] = item.name().into();
            base["qty"] = item.qty.into();
            if let Matter::Object { kind_id, .. } = &item.matter {
                base["kind_id"] = (*kind_id).into();
                if let Some(obj) = k_info::table().get(*kind_id) {
                    base["source"] = json!({
                        "id": obj.id,
                        "name": obj.name,
                        "glyph": obj.glyph.to_string(),
                        "color": obj.color.to_string(),
                    });
                }
            }
        }
        crate::describe::EntityKind::Tree | crate::describe::EntityKind::Iron => {
            let res = world.get::<Resource>(entity).unwrap();
            base["resource"] = match res.kind {
                ResourceKind::Wood => "wood".into(),
                ResourceKind::Iron => "iron".into(),
            };
            base["amount"] = res.amount.into();
        }
        crate::describe::EntityKind::Hut => {
            base["name"] = "hut".into();
        }
    }
    Some(base)
}

/// Return JSON details for the terrain cell at `(x, y)`.
pub fn cell_info(world: &mut World, x: i32, y: i32) -> Option<serde_json::Value> {
    let grid = world.resource::<Grid>();
    let feat = grid.get(x, y)?;
    let info = f_info::table().get(feat)?;
    Some(json!({
        "x": x,
        "y": y,
        "feat_id": feat,
        "name": info.name,
        "glyph": info.glyph.to_string(),
        "color": info.color.to_string(),
        "flags": {
            "walk": info.walk,
            "wall": info.wall,
            "permanent": info.permanent,
            "door": info.door,
            "stairs": info.stairs,
            "trap": info.trap,
            "water": info.water,
            "lava": info.lava,
            "tree": info.tree,
            "less": info.less,
            "more": info.more,
            "los": info.los,
            "project": info.project,
            "remember": info.remember,
        }
    }))
}
