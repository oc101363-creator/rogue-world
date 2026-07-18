//! Agent-local view — built straight from the world, NOT from a spectator
//! snapshot. This is the primary agent API payload: `self / view / can /
//! inbox / events`. The old path encoded the full map to base64 and decoded
//! it back to cut a window; this module just reads the grid directly.

use bevy_ecs::prelude::*;
use serde_json::json;

use crate::components::{
    AgentMailbox, AgentProfile, Health, Inventory, Position, StableId,
};
use crate::describe::entity_brief;
use crate::f_info;
use crate::grid::Grid;
use crate::systems::interact;
use crate::vision::{self, MAX_SIGHT};
use crate::world::TickCounter;

/// Canonical GET /api/view payload for one agent.
/// Returns None if the entity lacks the agent essentials.
pub fn build_agent_view(world: &mut World, agent: Entity) -> Option<serde_json::Value> {
    let pos = *world.get::<Position>(agent)?;
    let sid = world.get::<StableId>(agent)?.0;
    let tick = world.resource::<TickCounter>().0;

    // --- view window (FOV + memory, centered on agent) ---
    let vis = vision::compute_view_for_agents(world, &[agent]);
    let (grid_w, grid_h, cells) = {
        let g = world.resource::<Grid>();
        (g.width, g.height, g.cells.clone())
    };
    let table = f_info::table();
    let r = MAX_SIGHT;
    let side = 2 * r + 1;
    let x0 = pos.x - r;
    let y0 = pos.y - r;

    let mut map_rows: Vec<String> = Vec::with_capacity(side as usize);
    let mut vis_rows: Vec<String> = Vec::with_capacity(side as usize);
    let mut landmarks: Vec<serde_json::Value> = Vec::new();

    for vy in 0..side {
        let mut mrow = String::with_capacity(side as usize);
        let mut vrow = String::with_capacity(side as usize);
        let wy = y0 + vy;
        for vx in 0..side {
            let wx = x0 + vx;
            if wy < 0 || wx < 0 || wy >= grid_h || wx >= grid_w {
                mrow.push(' ');
                vrow.push(' ');
                continue;
            }
            let class = vis.display_class(wx, wy);
            vrow.push(match class {
                2 => 'v',
                1 => 'm',
                _ => ' ',
            });
            if class == 0 {
                mrow.push(' ');
                continue;
            }
            // memory cells show the REMEMBERED feat (never live edits)
            let feat_id = if class == 1 {
                vis.feats[(wy * grid_w + wx) as usize]
            } else {
                cells[(wy * grid_w + wx) as usize]
            };
            let info = table.get(feat_id);
            let glyph = info.map(|f| f.glyph).unwrap_or('?');
            mrow.push(glyph);

            // landmarks: currently-visible, non-trivial terrain (not self cell)
            if class == 2 && !(wx == pos.x && wy == pos.y) {
                let interesting = info
                    .map(|f| {
                        f.wall
                            || f.water
                            || f.lava
                            || f.door
                            || f.stairs
                            || f.trap
                            || f.tree
                            || (f.name != "FLOOR" && f.name != "NONE" && f.name != "INVIS")
                    })
                    .unwrap_or(glyph != '.' && glyph != ' ');
                if interesting {
                    landmarks.push(json!({
                        "x": wx,
                        "y": wy,
                        "dx": wx - pos.x,
                        "dy": wy - pos.y,
                        "feat_id": feat_id,
                        "name": info.map(|f| f.name.clone()).unwrap_or_else(|| "?".into()),
                        "glyph": glyph.to_string(),
                    }));
                }
            }
        }
        map_rows.push(mrow);
        vis_rows.push(vrow);
    }

    // Cap landmarks for LLM-friendly payloads (nearest first).
    landmarks.sort_by_key(|v| {
        let dx = v["dx"].as_i64().unwrap_or(0).abs();
        let dy = v["dy"].as_i64().unwrap_or(0).abs();
        dx + dy
    });
    landmarks.truncate(80);

    // --- entities: visible now, inside the window, excluding self ---
    let positioned: Vec<(Entity, i32, i32)> = {
        let mut q = world.query::<(Entity, &Position)>();
        q.iter(world).map(|(e, p)| (e, p.x, p.y)).collect()
    };
    let mut entities: Vec<serde_json::Value> = Vec::new();
    for (e, x, y) in positioned {
        if e == agent {
            continue;
        }
        if x < x0 || y < y0 || x >= x0 + side || y >= y0 + side {
            continue;
        }
        if !vis.is_visible(x, y) {
            continue;
        }
        if let Some(mut j) = entity_brief(world, e) {
            j["dx"] = (x - pos.x).into();
            j["dy"] = (y - pos.y).into();
            entities.push(j);
        }
    }
    entities.sort_by_key(|j| j["id"].as_u64().unwrap_or(0));

    // --- self ---
    let inv = world.get::<Inventory>(agent);
    let hp = world.get::<Health>(agent);
    let name = world.get::<AgentProfile>(agent).map(|p| p.name.clone());
    let self_body = json!({
        "id": sid,
        "name": name,
        "x": pos.x,
        "y": pos.y,
        "hp": hp.map(|h| h.hp),
        "max_hp": hp.map(|h| h.max_hp),
        "wood": inv.map(|i| i.wood()).unwrap_or(0),
        "iron": inv.map(|i| i.iron()).unwrap_or(0),
        "items": inv.map(|i| i.slots.iter().map(|s| {
            if s.qty > 1 { format!("{}×{}", s.matter.label(), s.qty) } else { s.matter.label() }
        }).collect::<Vec<_>>()).unwrap_or_default(),
        "pack": inv.map(|i| i.to_api()).unwrap_or_default(),
    });

    // --- can ---
    let interactions = interact::list_nearby(world, agent);
    let underfoot_class = vis.display_class(pos.x, pos.y);
    let underfoot_glyph = {
        let f = cells[(pos.y * grid_w + pos.x) as usize];
        table.get(f).map(|i| i.glyph).unwrap_or('?')
    };
    let here: Vec<serde_json::Value> = crate::spatial::at(world, pos.x, pos.y)
        .into_iter()
        .filter(|&e| e != agent)
        .filter_map(|e| entity_brief(world, e))
        .collect();
    let mut adjacent: Vec<serde_json::Value> = Vec::new();
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        for e in crate::spatial::at(world, pos.x + dx, pos.y + dy) {
            if e == agent {
                continue;
            }
            if let Some(mut j) = entity_brief(world, e) {
                j["dx"] = dx.into();
                j["dy"] = dy.into();
                adjacent.push(j);
            }
        }
    }

    // --- events: drain THIS agent's feedback inbox (consume-on-read) ---
    // Push-time FOV filtering already happened in distribute_feedback, so
    // everything here is learnable by this agent — and it survives however
    // long the agent took to come back. Each entry carries its tick stamp.
    let filtered_events: Vec<serde_json::Value> = world
        .get_mut::<crate::components::EventInbox>(agent)
        .map(|mut inbox| inbox.drain())
        .unwrap_or_default()
        .into_iter()
        .map(|(t, ev)| {
            let mut j = serde_json::to_value(&ev).unwrap_or_default();
            j["tick"] = t.into();
            j
        })
        .collect();

    // --- inbox (consumed on read) ---
    let inbox: Vec<serde_json::Value> = {
        let unread_info: Vec<(u64, String, String, u64)> = world
            .get::<AgentMailbox>(agent)
            .map(|mb| {
                mb.unread()
                    .iter()
                    .map(|env| (env.id, env.from.clone(), env.text.clone(), env.sent_tick))
                    .collect()
            })
            .unwrap_or_default();
        if !unread_info.is_empty() {
            let ids: Vec<u64> = unread_info.iter().map(|m| m.0).collect();
            if let Some(mut mb) = world.get_mut::<AgentMailbox>(agent) {
                mb.mark_read(&ids);
            }
        }
        unread_info
            .into_iter()
            .map(|(id, from, text, sent_tick)| {
                json!({ "id": id, "from": from, "text": text, "sent_tick": sent_tick })
            })
            .collect()
    };

    Some(json!({
        "ok": true,
        "tick": tick,
        "self": self_body,
        "view": {
            "ox": pos.x,
            "oy": pos.y,
            "r": r,
            "w": side,
            "h": side,
            "map": map_rows,
            "vision": vis_rows,
            "entities": entities,
            "landmarks": landmarks,
        },
        "can": {
            "interactions": interactions,
            "underfoot": {
                "glyph": underfoot_glyph,
                "vision": match underfoot_class { 2 => "v", 1 => "m", _ => " " },
            },
            "here": here,
            "adjacent": adjacent,
        },
        "inbox": inbox,
        "events": filtered_events,
    }))
}
