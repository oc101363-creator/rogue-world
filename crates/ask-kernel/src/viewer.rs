//! Viewer projection — glyphs/colors from full f_info table + FOV fog-of-war.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::actions::Interaction;
use crate::art;
use crate::components::{
    Agent, AgentProfile, Building, Glyph, Health, Inventory, Item, Matter, Monster, Position,
    Resource, ResourceKind, StableId,
};
use crate::events::GameEvent;
use crate::f_info;
use crate::grid::Grid;
use crate::systems::interact;
use crate::view;
use crate::vision::VisionMap;
use crate::world::TickCounter;

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

/// Compact row-major FeatId grid for identity-first clients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatIdsPayload {
    /// Encoding tag: little-endian u16 bytes as standard base64.
    pub enc: &'static str,
    pub w: i32,
    pub h: i32,
    pub data: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewerSnapshot {
    pub r#type: &'static str,
    pub tick: u64,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<String>,
    /// Frog f_info color letters per cell (same shape as `tiles`) — legacy client path.
    pub tile_colors: Vec<String>,
    /// Per-cell visibility: ' ' unknown, 'm' memory (MARK only), 'v' visible (VIEW+lit).
    pub vision: Vec<String>,
    /// Identity grid for FS-HDG / material art (full map; client paints void via vision).
    pub feat_ids: FeatIdsPayload,
    /// Bumps when art catalog / overlay changes.
    pub catalog_version: u32,
    pub entities: Vec<ViewerEntity>,
    /// Interactions available to focused agent (underfoot + neighbors).
    pub interactions: Vec<Interaction>,
    pub map: String,
    pub focused_agent_id: Option<u64>,
    pub recent_events: Vec<GameEvent>,
}

/// Build a snapshot using the global union vision map (terminal / internal use).
pub fn build_viewer_snapshot(world: &mut World, recent_events: &[GameEvent]) -> ViewerSnapshot {
    let vis = world
        .get_resource::<VisionMap>()
        .cloned()
        .unwrap_or_else(|| {
            let g = world.resource::<Grid>();
            VisionMap::new(g.width, g.height)
        });
    build_viewer_snapshot_with(world, recent_events, &vis, None, None, true)
}

/// Build a snapshot for a specific vision map and optional agent allow-list.
///
/// * `allowed_agents` — if `Some(ids)`, only agent entities with those `StableId`s
///   are included in the snapshot.  This is the server-side gate that keeps
///   un-tracked agents out of a player's view.
/// * `focus_agent_id` — which agent's interactions are returned.  If `None`,
///   the first allowed agent is used.
/// * `include_map` — if true, include the full ASCII `map` string (internal/
///   terminal only).  Web snapshots should pass `false` to avoid leaking unseen
///   terrain.
pub fn build_viewer_snapshot_with(
    world: &mut World,
    recent_events: &[GameEvent],
    vis: &VisionMap,
    allowed_agents: Option<&[u64]>,
    focus_agent_id: Option<u64>,
    include_map: bool,
) -> ViewerSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let table = f_info::table();
    let catalog_version = art::catalog().catalog_version;
    let (width, height, tiles, tile_colors, vision_rows, feat_ids) = {
        let grid = world.resource::<Grid>();
        let w = grid.width;
        let h = grid.height;
        let feat_ids = FeatIdsPayload {
            enc: "u16le_b64",
            w,
            h,
            data: art::encode_feat_ids_b64(&grid.cells),
        };
        let mut tiles = Vec::with_capacity(h as usize);
        let mut tile_colors = Vec::with_capacity(h as usize);
        let mut vision_rows = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut row = String::with_capacity(w as usize);
            let mut colors = String::with_capacity(w as usize);
            let mut vrow = String::with_capacity(w as usize);
            for x in 0..w {
                let class = vis.display_class(x, y);
                vrow.push(match class {
                    2 => 'v',
                    1 => 'm',
                    _ => ' ',
                });
                match class {
                    0 => {
                        // unexplored — darkness glyph
                        row.push(' ');
                        colors.push('D');
                    }
                    1 | 2 => {
                        let id = grid.cells[(y * w + x) as usize];
                        let info = table.get(id);
                        row.push(info.map(|f| f.glyph).unwrap_or('?'));
                        colors.push(info.map(|f| f.color).unwrap_or('w'));
                    }
                    _ => {
                        row.push(' ');
                        colors.push('D');
                    }
                }
            }
            tiles.push(row);
            tile_colors.push(colors);
            vision_rows.push(vrow);
        }
        (w, h, tiles, tile_colors, vision_rows, feat_ids)
    };

    let can_see = |x: i32, y: i32| -> bool { vis.is_visible(x, y) };

    let mut entities = Vec::new();
    {
        let mut q = world.query::<(
            &StableId,
            &Position,
            &Glyph,
            &Inventory,
            &Health,
            Option<&AgentProfile>,
            &Agent,
        )>();
        for (id, p, g, inv, hp, profile, _) in q.iter(world) {
            if let Some(allowed) = allowed_agents {
                // Own tracked agents are always shown; any other agent is shown only if visible now.
                if !allowed.contains(&id.0) && !can_see(p.x, p.y) {
                    continue;
                }
            }
            let pack_labels: Vec<String> = inv
                .slots
                .iter()
                .map(|s| {
                    if s.qty > 1 {
                        format!("{}×{}", s.matter.label(), s.qty)
                    } else {
                        s.matter.label()
                    }
                })
                .collect();
            entities.push(ViewerEntity {
                id: id.0,
                kind: "agent".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: Some(inv.wood()),
                iron: Some(inv.iron()),
                amount: None,
                hp: Some(hp.hp),
                max_hp: Some(hp.max_hp),
                items: Some(pack_labels),
                pack: Some(inv.to_api()),
                name: profile.map(|pr| pr.name.clone()),
                race_id: None,
                kind_id: None,
            });
        }
    }

    // Only show non-agent entities on currently visible cells.
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Resource)>();
        for (id, p, g, r) in q.iter(world) {
            if !can_see(p.x, p.y) {
                continue;
            }
            let kind = match r.kind {
                ResourceKind::Wood => "tree",
                ResourceKind::Iron => "iron",
            };
            entities.push(ViewerEntity {
                id: id.0,
                kind: kind.into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: None,
                iron: None,
                amount: Some(r.amount),
                hp: None,
                max_hp: None,
                items: None,
                pack: None,
                name: None,
                race_id: None,
                kind_id: None,
            });
        }
    }
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Building)>();
        for (id, p, g, _) in q.iter(world) {
            // buildings: show if visible OR memorized (they're terrain-ish)
            if !vis.is_visible(p.x, p.y) && !vis.is_mark(p.x, p.y) {
                continue;
            }
            entities.push(ViewerEntity {
                id: id.0,
                kind: "hut".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
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
            });
        }
    }
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Monster, Option<&Health>)>();
        for (id, p, g, m, hp) in q.iter(world) {
            if !can_see(p.x, p.y) {
                continue;
            }
            entities.push(ViewerEntity {
                id: id.0,
                kind: "monster".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: None,
                iron: None,
                amount: None,
                hp: hp.map(|h| h.hp),
                max_hp: hp.map(|h| h.max_hp),
                items: None,
                pack: None,
                name: Some(m.name.clone()),
                race_id: Some(m.race_id),
                kind_id: None,
            });
        }
    }
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Item)>();
        for (id, p, g, it) in q.iter(world) {
            if !can_see(p.x, p.y) {
                continue;
            }
            let kind_id = match &it.matter {
                Matter::Object { kind_id, .. } => Some(*kind_id),
                _ => None,
            };
            entities.push(ViewerEntity {
                id: id.0,
                kind: "item".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: None,
                iron: None,
                amount: Some(it.qty),
                hp: None,
                max_hp: None,
                items: None,
                pack: None,
                name: Some(it.name()),
                race_id: None,
                kind_id,
            });
        }
    }
    entities.sort_by_key(|e| e.id);
    let focused_agent_id =
        focus_agent_id.or_else(|| entities.iter().find(|e| e.kind == "agent").map(|e| e.id));

    // Discover interactions for focused agent (data-driven options)
    let interactions = {
        let agent_e = {
            let mut q = world.query_filtered::<(Entity, &StableId), With<Agent>>();
            let want = focused_agent_id;
            q.iter(world)
                .find(|(_, sid)| want.map(|id| sid.0 == id).unwrap_or(true))
                .map(|(e, _)| e)
        };
        agent_e
            .map(|e| interact::list_nearby(world, e))
            .unwrap_or_default()
    };

    let map = if include_map {
        view::render(world)
    } else {
        String::new()
    };

    ViewerSnapshot {
        r#type: "snapshot",
        tick,
        width,
        height,
        tiles,
        tile_colors,
        vision: vision_rows,
        feat_ids,
        catalog_version,
        entities,
        interactions,
        map,
        focused_agent_id,
        recent_events: recent_events.to_vec(),
    }
}
