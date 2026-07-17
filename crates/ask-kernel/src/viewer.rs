//! Viewer projection — glyphs/colors from full f_info table + FOV fog-of-war.
//!
//! Entity construction lives in `describe` (one kind vocabulary, one shape);
//! this module only decides *who may see what* and packs the grid rows.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::actions::Interaction;
use crate::art;
use crate::components::{Agent, Building, Position, StableId};
use crate::describe::viewer_entity;
use crate::events::GameEvent;
use crate::f_info;
use crate::grid::Grid;
use crate::systems::interact;
use crate::vision::VisionMap;
use crate::world::TickCounter;

pub use crate::describe::ViewerEntity;

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
    /// Per-cell visibility: ' ' unknown, 'm' memory (MARK only), 'v' visible (VIEW+lit).
    pub vision: Vec<String>,
    /// Identity grid for FS-HDG / material art (masked: unseen cells are feat 0).
    pub feat_ids: FeatIdsPayload,
    /// Bumps when art catalog / overlay changes.
    pub catalog_version: u32,
    pub entities: Vec<ViewerEntity>,
    /// Interactions available to focused agent (underfoot + neighbors).
    pub interactions: Vec<Interaction>,
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
    build_viewer_snapshot_with(world, recent_events, &vis, None, None)
}

/// Build a snapshot for a specific vision map and optional agent allow-list.
///
/// * `allowed_agents` — if `Some(ids)`, only agent entities with those `StableId`s
///   are included in the snapshot.  This is the server-side gate that keeps
///   un-tracked agents out of a player's view.
/// * `focus_agent_id` — which agent's interactions are returned. Must be in
///   the allowed set; `None` (or a non-member) yields no interactions.
pub fn build_viewer_snapshot_with(
    world: &mut World,
    recent_events: &[GameEvent],
    vis: &VisionMap,
    allowed_agents: Option<&[u64]>,
    focus_agent_id: Option<u64>,
) -> ViewerSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let table = f_info::table();
    let catalog_version = art::catalog().catalog_version;
    let (width, height, tiles, vision_rows, feat_ids) = {
        let grid = world.resource::<Grid>();
        let w = grid.width;
        let h = grid.height;
        // FOV gate: cells the viewer may not see are masked to feat 0 (NONE).
        // tiles/vision rows already paint void there; feat_ids must not leak them.
        let masked: Vec<u16> = {
            let mut m = Vec::with_capacity(grid.cells.len());
            for (i, &c) in grid.cells.iter().enumerate() {
                let x = (i as i32) % w;
                let y = (i as i32) / w;
                m.push(match vis.display_class(x, y) {
                    0 => 0,                        // unknown: masked
                    1 => vis.feats[i],             // memory: remembered feat only
                    _ => c,                        // visible: live terrain
                });
            }
            m
        };
        let feat_ids = FeatIdsPayload {
            enc: "u16le_b64",
            w,
            h,
            data: art::encode_feat_ids_b64(&masked),
        };
        let mut tiles = Vec::with_capacity(h as usize);
        let mut vision_rows = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut row = String::with_capacity(w as usize);
            let mut vrow = String::with_capacity(w as usize);
            for x in 0..w {
                let class = vis.display_class(x, y);
                vrow.push(match class {
                    2 => 'v',
                    1 => 'm',
                    _ => ' ',
                });
                match class {
                    1 => {
                        // remembered terrain AS REMEMBERED (never live edits)
                        let id = vis.feats[(y * w + x) as usize];
                        let info = table.get(id);
                        row.push(info.map(|f| f.glyph).unwrap_or(' '));
                    }
                    2 => {
                        let id = grid.cells[(y * w + x) as usize];
                        let info = table.get(id);
                        row.push(info.map(|f| f.glyph).unwrap_or('?'));
                    }
                    _ => {
                        // unexplored — darkness glyph
                        row.push(' ');
                    }
                }
            }
            tiles.push(row);
            vision_rows.push(vrow);
        }
        (w, h, tiles, vision_rows, feat_ids)
    };

    let can_see = |x: i32, y: i32| -> bool { vis.is_visible(x, y) };

    // One position scan; per-entity kind/shape from `describe`.
    let positioned: Vec<(Entity, i32, i32)> = {
        let mut q = world.query::<(Entity, &Position)>();
        q.iter(world).map(|(e, p)| (e, p.x, p.y)).collect()
    };
    let mut entities = Vec::new();
    for (e, x, y) in positioned {
        let is_agent = world.get::<Agent>(e).is_some();
        if is_agent {
            // Own tracked agents always shown; others only if visible now.
            if let Some(allowed) = allowed_agents {
                let sid = world.get::<StableId>(e).map(|s| s.0).unwrap_or(0);
                if !allowed.contains(&sid) && !can_see(x, y) {
                    continue;
                }
            }
        } else if world.get::<Building>(e).is_some() {
            // buildings are terrain-ish: shown if visible OR remembered
            if !vis.is_visible(x, y) && !vis.is_mark(x, y) {
                continue;
            }
        } else if !can_see(x, y) {
            continue;
        }
        if let Some(v) = viewer_entity(world, e) {
            entities.push(v);
        }
    }
    entities.sort_by_key(|e| e.id);
    // Focus must come from the caller (their token). Falling back to "the
    // first agent in the snapshot" leaks that agent's options to any watcher.
    let focused_agent_id = focus_agent_id;

    // Interactions exist only for a legitimately focused agent (their token
    // is in the allowed set, or the internal unrestricted path). Dark
    // snapshots get none.
    let interactions = {
        let legit = focused_agent_id
            .map(|id| allowed_agents.map(|a| a.contains(&id)).unwrap_or(true))
            .unwrap_or(false);
        if !legit {
            Vec::new()
        } else {
            let agent_e = {
                let mut q = world.query_filtered::<(Entity, &StableId), With<Agent>>();
                let want = focused_agent_id;
                q.iter(world)
                    .find(|(_, sid)| want.map(|id| sid.0 == id).unwrap_or(false))
                    .map(|(e, _)| e)
            };
            agent_e
                .map(|e| interact::list_nearby(world, e))
                .unwrap_or_default()
        }
    };

    ViewerSnapshot {
        r#type: "snapshot",
        tick,
        width,
        height,
        tiles,
        vision: vision_rows,
        feat_ids,
        catalog_version,
        entities,
        interactions,
        focused_agent_id,
        recent_events: recent_events.to_vec(),
    }
}
