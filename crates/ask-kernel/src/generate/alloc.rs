//! Frog `_cave_gen_monsters` / `_cave_gen_objects` / trap-room fill.
//! Places extra MON/OBJ beyond template L: directives.

use super::Rng;
use crate::f_info::id;
use crate::grid::Grid;
use crate::k_info;
use crate::r_info;
use crate::vaults::{SpawnMon, SpawnObj};
use crate::world::Depth;

/// Frog: usually ~14 base + depth noise, scaled for small levels.
pub fn alloc_monsters(grid: &Grid, depth: u32, rng: &mut Rng, out: &mut Vec<SpawnMon>) {
    let table = r_info::table();
    if table.count() == 0 {
        return;
    }
    let base = 14i32;
    let mut ct = base + rng.randint1((depth as i32 / 2).max(1) + 3);
    // scale by map area vs frog max 66*198
    let area = (grid.width * grid.height) as i32;
    let max_area = 66 * 198;
    ct = (ct * area / max_area).max(8);
    if depth >= 50 {
        ct = ct * 3 / 5;
    }
    ct = ct.min(80);

    let mut floors = floor_cells(grid);
    shuffle(&mut floors, rng);

    let mut placed = 0;
    let mut fi = 0;
    while placed < ct && fi < floors.len() {
        let (x, y) = floors[fi];
        fi += 1;
        // skip if already a spawn here
        if out.iter().any(|m| m.x == x && m.y == y) {
            continue;
        }
        let Some(race) = table.pick_any(rng.next_u64() as usize) else {
            break;
        };
        out.push(SpawnMon {
            x,
            y,
            race_id: race.id,
            glyph: race.glyph,
            color: race.color,
            name: race.name.clone(),
        });
        placed += 1;
    }
}

/// Frog alloc_object spirit: room + corridor items/gold/food.
pub fn alloc_objects(grid: &Grid, depth: u32, rng: &mut Rng, out: &mut Vec<SpawnObj>) {
    let table = k_info::table();
    if table.count() == 0 {
        return;
    }
    // DUN_AMT_ROOM ~7, DUN_AMT_ITEM ~3, gold ~3 + noise
    let mut ct = 7 + 3 + rng.randint1(5) + (depth as i32 / 5);
    let area = (grid.width * grid.height) as i32;
    ct = (ct * area / (66 * 198)).max(6).min(60);

    let mut floors = floor_cells(grid);
    shuffle(&mut floors, rng);

    let mut placed = 0;
    let mut fi = 0;
    while placed < ct && fi < floors.len() {
        let (x, y) = floors[fi];
        fi += 1;
        if out.iter().any(|o| o.x == x && o.y == y) {
            continue;
        }
        let kind = if rng.percent(20) {
            table
                .find_name_contains("gold")
                .or_else(|| table.pick_any(rng.next_u64() as usize))
        } else if depth <= 15 && rng.percent(15) {
            table
                .find_name_contains("light")
                .or_else(|| table.find_name_contains("torch"))
                .or_else(|| table.pick_any(rng.next_u64() as usize))
        } else if rng.percent(20) {
            table
                .find_name_contains("food")
                .or_else(|| table.find_name_contains("ration"))
                .or_else(|| table.pick_any(rng.next_u64() as usize))
        } else {
            table.pick_any(rng.next_u64() as usize)
        };
        let Some(kind) = kind else { break };
        out.push(SpawnObj {
            x,
            y,
            kind_id: kind.id,
            glyph: kind.glyph,
            color: kind.color,
            name: kind.name.clone(),
        });
        placed += 1;
    }
}

fn floor_cells(grid: &Grid) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for y in 1..grid.height - 1 {
        for x in 1..grid.width - 1 {
            if grid.buildable(x, y) {
                v.push((x, y));
            }
        }
    }
    v
}

fn shuffle(v: &mut [(i32, i32)], rng: &mut Rng) {
    for i in (1..v.len()).rev() {
        let j = rng.randint0((i + 1) as i32) as usize;
        v.swap(i, j);
    }
}

/// Stamp many traps into a rectangular room interior (build_type14 spirit).
pub fn fill_trap_room(
    feats: &mut [u16],
    w: i32,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    rng: &mut Rng,
) {
    let traps = id::TRAP_FEATS;
    for y in (y1 + 1)..y2 {
        for x in (x1 + 1)..x2 {
            if x <= 0 || y <= 0 || x >= w - 1 {
                continue;
            }
            let i = (y * w + x) as usize;
            if feats[i] != id::FLOOR && feats[i] != id::DIRT && feats[i] != id::GRASS {
                continue;
            }
            if rng.percent(35) {
                feats[i] = traps[rng.randint0(traps.len() as i32) as usize];
            }
        }
    }
    // special center trap
    let cx = (x1 + x2) / 2;
    let cy = (y1 + y2) / 2;
    let i = (cy * w + cx) as usize;
    feats[i] = if rng.percent(50) {
        id::TRAP_FIRE
    } else {
        id::TRAP_TY_CURSE
    };
}

/// Density helper used by callers with Depth resource.
pub fn depth_amt(depth: u32) -> i32 {
    (depth as i32 / 2).max(1) + 3
}

pub fn _use_depth_resource(d: &Depth) -> u32 {
    d.0
}
