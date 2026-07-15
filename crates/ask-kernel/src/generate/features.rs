//! Post-room world features: lakes, rivers, trees, traps, destroy_level, maze.
//! Mirrors frog generate.c / streams.c / object2.c place_trap (structure).

use super::{Cave, Cell, Rng};
use crate::f_info::id;

/// Frog `destroy_level` — a few epicenters of rubble/chaos (streams.c).
pub fn destroy_level(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let n = rng.randint1(5);
    for _ in 0..n {
        let x1 = rng.rand_range(5, (w - 6).max(6));
        let y1 = rng.rand_range(5, (h - 6).max(6));
        destroy_area(feats, w, h, x1, y1, 15, rng);
    }
}

/// Frog destroy_area spirit: circle of rubble / pits / floor noise.
fn destroy_area(feats: &mut [u16], w: i32, h: i32, cx: i32, cy: i32, rad: i32, rng: &mut Rng) {
    for yy in (cy - rad)..=(cy + rad) {
        for xx in (cx - rad)..=(cx + rad) {
            if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                continue;
            }
            let d2 = (xx - cx) * (xx - cx) + (yy - cy) * (yy - cy);
            if d2 > rad * rad {
                continue;
            }
            let i = (yy * w + xx) as usize;
            if feats[i] == id::PERMANENT {
                continue;
            }
            // stomp stairs less often
            if matches!(feats[i], id::UP_STAIR | id::DOWN_STAIR) && rng.percent(80) {
                continue;
            }
            feats[i] = match rng.randint0(10) {
                0..=3 => id::RUBBLE,
                4..=5 => id::DARK_PIT,
                6 => id::FLOOR,
                7 => id::DIRT,
                _ => id::GRANITE,
            };
        }
    }
}

/// Frog place_trap / choose_random_trap — scatter trap feats on clean floors.
pub fn alloc_traps(feats: &mut [u16], w: i32, h: i32, count: i32, rng: &mut Rng) {
    let mut placed = 0;
    let mut attempts = 0;
    while placed < count && attempts < count * 40 {
        attempts += 1;
        let x = rng.rand_range(1, w - 1);
        let y = rng.rand_range(1, h - 1);
        let i = (y * w + x) as usize;
        // clean floor only
        if !matches!(feats[i], id::FLOOR | id::DIRT | id::GRASS) {
            continue;
        }
        let trap = id::TRAP_FEATS[rng.randint0(id::TRAP_FEATS.len() as i32) as usize];
        feats[i] = trap;
        placed += 1;
    }
}

/// Frog build_maze_vault spirit — spanning-tree maze in a rectangle (rooms.c).
/// Written into generation Cell grid before bake.
pub fn build_maze_region(cave: &mut Cave, x1: i32, y1: i32, x2: i32, y2: i32, rng: &mut Rng) {
    if x2 - x1 < 5 || y2 - y1 < 5 {
        return;
    }
    // fill with solid, carve outer as outer wall, interior solid for maze walls
    for y in y1..=y2 {
        for x in x1..=x2 {
            if y == y1 || y == y2 || x == x1 || x == x2 {
                cave.set(x, y, Cell::Outer);
            } else {
                cave.set(x, y, Cell::Solid);
            }
        }
    }

    // odd-cell DFS maze (classic roguelike)
    let mut stack = vec![(x1 + 1, y1 + 1)];
    cave.set(x1 + 1, y1 + 1, Cell::Room);
    while let Some(&(cx, cy)) = stack.last() {
        let mut dirs = [(2, 0), (-2, 0), (0, 2), (0, -2)];
        // shuffle
        for i in (1..4).rev() {
            let j = rng.randint0((i + 1) as i32) as usize;
            dirs.swap(i, j);
        }
        let mut carved = false;
        for (dx, dy) in dirs {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx <= x1 || ny <= y1 || nx >= x2 || ny >= y2 {
                continue;
            }
            if cave.get(nx, ny) != Cell::Solid {
                continue;
            }
            cave.set(cx + dx / 2, cy + dy / 2, Cell::Room);
            cave.set(nx, ny, Cell::Room);
            stack.push((nx, ny));
            carved = true;
            break;
        }
        if !carved {
            stack.pop();
        }
    }
}

/// Optional full-level maze mode (frog DF1_MAZE / build_maze_vault driver).
pub fn maybe_maze_level(cave: &mut Cave, rng: &mut Rng) -> bool {
    // rarer: maze is rock-heavy; open map target prefers rooms
    if !rng.percent(3) {
        return false;
    }
    let margin = 4;
    build_maze_region(
        cave,
        margin,
        margin,
        cave.w - 1 - margin,
        cave.h - 1 - margin,
        rng,
    );
    true
}

/// Embed a maze vault inside an existing room region (additional frog room flavor).
pub fn stamp_maze_vault(cave: &mut Cave, rooms: &[super::rooms::Room], rng: &mut Rng) {
    if rooms.is_empty() || !rng.percent(25) {
        return;
    }
    let r = &rooms[rng.randint0(rooms.len() as i32) as usize];
    if r.x2 - r.x1 < 9 || r.y2 - r.y1 < 9 {
        return;
    }
    build_maze_region(cave, r.x1, r.y1, r.x2, r.y2, rng);
}
