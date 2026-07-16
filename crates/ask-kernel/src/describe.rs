//! Entity description — ONE kind vocabulary, ONE serialization.
//!
//! Before this module, "what kind is this entity" and "how does it look as
//! JSON" were re-implemented in viewer.rs (5 blocks), inspect.rs, and
//! serve.rs — with subtly different answers (the unregistered agent was
//! "unknown" in one place, "agent" in another). Now there is exactly one
//! classifier and one brief JSON shape; projections enrich from it.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{
    Agent, AgentProfile, Building, Glyph, Health, Inventory, Item, Matter, Monster, Position,
    Resource, ResourceKind, StableId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Agent,
    Tree,
    Iron,
    Hut,
    Monster,
    Item,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Agent => "agent",
            EntityKind::Tree => "tree",
            EntityKind::Iron => "iron",
            EntityKind::Hut => "hut",
            EntityKind::Monster => "monster",
            EntityKind::Item => "item",
        }
    }

    /// The ONLY classifier. Agents are agents (marker component), profile
    /// or not — registration is an attribute, not a kind.
    pub fn classify(world: &World, e: Entity) -> Option<Self> {
        if world.get::<Agent>(e).is_some() {
            return Some(EntityKind::Agent);
        }
        if let Some(r) = world.get::<Resource>(e) {
            return Some(match r.kind {
                ResourceKind::Wood => EntityKind::Tree,
                ResourceKind::Iron => EntityKind::Iron,
            });
        }
        if world.get::<Building>(e).is_some() {
            return Some(EntityKind::Hut);
        }
        if world.get::<Monster>(e).is_some() {
            return Some(EntityKind::Monster);
        }
        if world.get::<Item>(e).is_some() {
            return Some(EntityKind::Item);
        }
        None
    }
}

/// Snapshot DTO for spectator payloads. Field shape is the web client's
/// contract — construct it exclusively via [`viewer_entity`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewerEntity {
    pub id: u64,
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub glyph: char,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wood: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iron: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<String>>,
    /// Structured pack slots (Matter stacks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Monster race template id (presentation catalog key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub race_id: Option<u16>,
    /// Object kind template id (presentation catalog key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind_id: Option<u16>,
}

/// The ONE ViewerEntity constructor (replaces five hand-rolled blocks).
pub fn viewer_entity(world: &World, e: Entity) -> Option<ViewerEntity> {
    let kind = EntityKind::classify(world, e)?;
    let sid = world.get::<StableId>(e)?;
    let p = world.get::<Position>(e)?;
    let glyph = world.get::<Glyph>(e).map(|g| g.0).unwrap_or('?');
    let mut v = ViewerEntity {
        id: sid.0,
        kind: kind.as_str().into(),
        x: p.x,
        y: p.y,
        glyph,
        wood: None,
        iron: None,
        amount: None,
        hp: None,
        max_hp: None,
        items: None,
        pack: None,
        name: None,
        race_id: None,
        kind_id: None,
    };
    match kind {
        EntityKind::Agent => {
            if let Some(inv) = world.get::<Inventory>(e) {
                v.wood = Some(inv.wood());
                v.iron = Some(inv.iron());
                v.items = Some(
                    inv.slots
                        .iter()
                        .map(|s| {
                            if s.qty > 1 {
                                format!("{}×{}", s.matter.label(), s.qty)
                            } else {
                                s.matter.label()
                            }
                        })
                        .collect(),
                );
                v.pack = Some(inv.to_api());
            }
            if let Some(h) = world.get::<Health>(e) {
                v.hp = Some(h.hp);
                v.max_hp = Some(h.max_hp);
            }
            v.name = world.get::<AgentProfile>(e).map(|pr| pr.name.clone());
        }
        EntityKind::Tree | EntityKind::Iron => {
            v.amount = world.get::<Resource>(e).map(|r| r.amount);
        }
        EntityKind::Hut => {
            v.name = Some("hut".into());
        }
        EntityKind::Monster => {
            let m = world.get::<Monster>(e).unwrap();
            v.name = Some(m.name.clone());
            v.race_id = Some(m.race_id);
            if let Some(h) = world.get::<Health>(e) {
                v.hp = Some(h.hp);
                v.max_hp = Some(h.max_hp);
            }
        }
        EntityKind::Item => {
            let it = world.get::<Item>(e).unwrap();
            v.name = Some(it.name());
            v.amount = Some(it.qty);
            if let Matter::Object { kind_id, .. } = &it.matter {
                v.kind_id = Some(*kind_id);
            }
        }
    }
    Some(v)
}

/// Brief JSON for agent-facing payloads (view entities, here/adjacent).
/// Same truth as [`viewer_entity`], shape optimized for LLM consumers.
pub fn entity_brief(world: &World, e: Entity) -> Option<serde_json::Value> {
    let v = viewer_entity(world, e)?;
    let mut j = serde_json::json!({
        "id": v.id,
        "kind": v.kind,
        "x": v.x,
        "y": v.y,
        "glyph": v.glyph.to_string(),
    });
    if let Some(n) = v.name {
        j["name"] = n.into();
    }
    if let Some(a) = v.amount {
        j["amount"] = a.into();
    }
    if let Some(hp) = v.hp {
        j["hp"] = hp.into();
        j["max_hp"] = v.max_hp.unwrap_or(hp).into();
    }
    if let Some(r) = v.race_id {
        j["race_id"] = r.into();
    }
    if let Some(k) = v.kind_id {
        j["kind_id"] = k.into();
    }
    Some(j)
}
