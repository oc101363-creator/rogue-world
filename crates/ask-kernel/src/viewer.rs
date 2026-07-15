//! Viewer projection — glyphs/colors from full f_info table.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Agent, Building, Glyph, Inventory, Position, Resource, ResourceKind, StableId};
use crate::events::GameEvent;
use crate::f_info;
use crate::grid::Grid;
use crate::view;
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewerSnapshot {
    pub r#type: &'static str,
    pub tick: u64,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<String>,
    /// Frog f_info color letters per cell (same shape as `tiles`) — client themes map these.
    pub tile_colors: Vec<String>,
    pub entities: Vec<ViewerEntity>,
    pub map: String,
    pub focused_agent_id: Option<u64>,
    pub recent_events: Vec<GameEvent>,
}

pub fn build_viewer_snapshot(world: &mut World, recent_events: &[GameEvent]) -> ViewerSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let table = f_info::table();
    let (width, height, tiles, tile_colors) = {
        let grid = world.resource::<Grid>();
        let w = grid.width;
        let h = grid.height;
        let mut tiles = Vec::with_capacity(h as usize);
        let mut tile_colors = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut row = String::with_capacity(w as usize);
            let mut colors = String::with_capacity(w as usize);
            for x in 0..w {
                let id = grid.cells[(y * w + x) as usize];
                let info = table.get(id);
                row.push(info.map(|f| f.glyph).unwrap_or('?'));
                colors.push(info.map(|f| f.color).unwrap_or('w'));
            }
            tiles.push(row);
            tile_colors.push(colors);
        }
        (w, h, tiles, tile_colors)
    };

    let mut entities = Vec::new();
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Inventory, &Agent)>();
        for (id, p, g, inv, _) in q.iter(world) {
            entities.push(ViewerEntity {
                id: id.0,
                kind: "agent".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: Some(inv.wood),
                iron: Some(inv.iron),
                amount: None,
            });
        }
    }
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Resource)>();
        for (id, p, g, r) in q.iter(world) {
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
            });
        }
    }
    {
        let mut q = world.query::<(&StableId, &Position, &Glyph, &Building)>();
        for (id, p, g, _) in q.iter(world) {
            entities.push(ViewerEntity {
                id: id.0,
                kind: "hut".into(),
                x: p.x,
                y: p.y,
                glyph: g.0,
                wood: None,
                iron: None,
                amount: None,
            });
        }
    }
    entities.sort_by_key(|e| e.id);
    let focused_agent_id = entities.iter().find(|e| e.kind == "agent").map(|e| e.id);
    let map = view::render(world);

    ViewerSnapshot {
        r#type: "snapshot",
        tick,
        width,
        height,
        tiles,
        tile_colors,
        entities,
        map,
        focused_agent_id,
        recent_events: recent_events.to_vec(),
    }
}
