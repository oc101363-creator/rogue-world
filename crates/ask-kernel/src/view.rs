//! ASCII projection for CLI.

use bevy_ecs::prelude::*;

use crate::components::{Agent, Glyph, Inventory, Position};
use crate::grid::Grid;
use crate::world::TickCounter;

pub fn render(world: &mut World) -> String {
    let grid = world.resource::<Grid>();
    let tick = world.resource::<TickCounter>().0;
    let w = grid.width as usize;
    let h = grid.height as usize;

    let mut chars: Vec<Vec<char>> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| grid.cells[y * w + x].glyph())
                .collect()
        })
        .collect();

    let mut non_agents = Vec::new();
    let mut agents = Vec::new();
    let mut q = world.query::<(&Position, &Glyph, Option<&Agent>)>();
    for (p, g, ag) in q.iter(world) {
        if ag.is_some() {
            agents.push((p.x, p.y, g.0));
        } else {
            non_agents.push((p.x, p.y, g.0));
        }
    }
    for (x, y, ch) in non_agents {
        if y >= 0 && x >= 0 && (y as usize) < h && (x as usize) < w {
            chars[y as usize][x as usize] = ch;
        }
    }
    for (x, y, ch) in agents {
        if y >= 0 && x >= 0 && (y as usize) < h && (x as usize) < w {
            chars[y as usize][x as usize] = ch;
        }
    }

    let mut inv_s = String::from("wood=? iron=?");
    let mut q2 = world.query_filtered::<&Inventory, With<Agent>>();
    if let Some(inv) = q2.iter(world).next() {
        inv_s = format!("wood={} iron={}", inv.wood, inv.iron);
    }

    let mut out = format!("t={tick} {inv_s}\n");
    for row in chars {
        out.push_str(&row.iter().collect::<String>());
        out.push('\n');
    }
    out
}
