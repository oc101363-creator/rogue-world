//! Viewer projection — glyphs + frog colors for web UI.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Agent, Building, Glyph, Inventory, Position, Resource, ResourceKind, StableId};
use crate::events::GameEvent;
use crate::feat::Feat;
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
    /// Terrain glyph rows
    pub tiles: Vec<String>,
    /// Parallel fg color CSS per cell (row-major strings of same length as tiles)
    pub tile_fg: Vec<Vec<String>>,
    pub tile_bg: Vec<Vec<String>>,
    pub entities: Vec<ViewerEntity>,
    pub map: String,
    pub focused_agent_id: Option<u64>,
    pub recent_events: Vec<GameEvent>,
}

fn feat_bg(f: Feat) -> &'static str {
    match f {
        Feat::DeepWater | Feat::ShallowWater => "#0a1a2a",
        Feat::DeepLava | Feat::ShallowLava => "#2a0a0a",
        Feat::Grass => "#0a1a0a",
        Feat::Dirt => "#1a140a",
        Feat::Permanent => "#1a1a22",
        Feat::Granite | Feat::GraniteOuter | Feat::Mountain => "#121212",
        Feat::MagmaVein | Feat::MagmaTreasure => "#1a1a14",
        Feat::QuartzVein | Feat::QuartzTreasure => "#181818",
        Feat::Rubble => "#161616",
        _ => "#0c100c",
    }
}

pub fn build_viewer_snapshot(world: &mut World, recent_events: &[GameEvent]) -> ViewerSnapshot {
    let tick = world.resource::<TickCounter>().0;
    let (width, height, tiles, tile_fg, tile_bg) = {
        let grid = world.resource::<Grid>();
        let w = grid.width;
        let h = grid.height;
        let mut tiles = Vec::with_capacity(h as usize);
        let mut tile_fg = Vec::with_capacity(h as usize);
        let mut tile_bg = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut row = String::with_capacity(w as usize);
            let mut fg = Vec::with_capacity(w as usize);
            let mut bg = Vec::with_capacity(w as usize);
            for x in 0..w {
                let f = grid.cells[(y * w + x) as usize];
                row.push(f.glyph());
                fg.push(f.color().css().to_string());
                bg.push(feat_bg(f).to_string());
            }
            tiles.push(row);
            tile_fg.push(fg);
            tile_bg.push(bg);
        }
        (w, h, tiles, tile_fg, tile_bg)
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
        tile_fg,
        tile_bg,
        entities,
        map,
        focused_agent_id,
        recent_events: recent_events.to_vec(),
    }
}
