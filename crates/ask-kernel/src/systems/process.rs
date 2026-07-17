//! World process engine — applies process_rules to the Grid every N ticks.
//! 只写简单规则，不写手工结局: this file knows HOW to run rules, never WHAT
//! the game is. New processes go in process_rules.rs, not here.

use bevy_ecs::prelude::*;

use crate::events::{EventBuf, GameEvent};
use crate::f_info::{self, FeatId};
use crate::grid::Grid;
use crate::process_rules::{self, CellCond, NeighborCond, ProcessAction};
use crate::world::{TickCounter, WorldSeed};

/// Deterministic per-cell-per-tick roll (0..100).
fn roll(seed: u64, tick: u64, idx: usize) -> u8 {
    let mut x = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick.wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((idx as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
        | 1;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % 100) as u8
}

fn cell_matches(cond: &CellCond, feat: FeatId) -> bool {
    match *cond {
        CellCond::FeatIs(f) => feat == f,
        CellCond::ShallowWater => feat == f_info::id::SHALLOW_WATER,
        CellCond::Grass => feat == f_info::id::GRASS,
    }
}

fn neighbor_matches(grid: &Grid, x: i32, y: i32, cond: &NeighborCond) -> Option<(i32, i32)> {
    let table = f_info::table();
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    match *cond {
        NeighborCond::None => None,
        NeighborCond::Flammable => dirs.iter().find_map(|&(dx, dy)| {
            grid.get(x + dx, y + dy)
                .and_then(|f| table.get(f))
                .filter(|info| process_rules::is_flammable(info))
                .map(|_| (x + dx, y + dy))
        }),
        NeighborCond::AnyFeat(want) => dirs.iter().find_map(|&(dx, dy)| {
            if grid.get(x + dx, y + dy) == Some(want) {
                Some((x + dx, y + dy))
            } else {
                None
            }
        }),
        NeighborCond::FlowTarget => dirs.iter().find_map(|&(dx, dy)| {
            grid.get(x + dx, y + dy)
                .and_then(|f| table.get(f))
                .filter(|info| process_rules::is_flow_target(info))
                .map(|_| (x + dx, y + dy))
        }),
        NeighborCond::DirtWithWaterNear => {
            let dirt = dirs.iter().find_map(|&(dx, dy)| {
                if grid.get(x + dx, y + dy) == Some(f_info::id::DIRT) {
                    Some((x + dx, y + dy))
                } else {
                    None
                }
            })?;
            let r: i32 = 3;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r || (dx == 0 && dy == 0) {
                        continue;
                    }
                    if grid
                        .get(x + dx, y + dy)
                        .and_then(|f| table.get(f))
                        .map(|info| info.water)
                        .unwrap_or(false)
                    {
                        return Some(dirt);
                    }
                }
            }
            None
        }
    }
}

pub fn process_world(world: &mut World) {
    let tick = world.resource::<TickCounter>().0;
    if tick % crate::balance::PROCESS_EVERY_N != 0 {
        return;
    }
    let seed = world.get_resource::<WorldSeed>().map(|s| s.0).unwrap_or(1);
    let (w, h, cells) = {
        let g = world.resource::<Grid>();
        (g.width, g.height, g.cells.clone())
    };
    let grid_snap = Grid {
        width: w,
        height: h,
        cells: cells.clone(),
    };
    let rules = process_rules::rules();
    let mut changed: Vec<(i32, i32, FeatId, FeatId, process_rules::Cause)> = Vec::new();
    let mut claimed: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for rule in rules {
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if claimed.contains(&idx) {
                    continue;
                }
                let feat = cells[idx];
                if !cell_matches(&rule.on, feat) {
                    continue;
                }
                let target = if matches!(rule.neighbors, NeighborCond::None) {
                    Some((x, y))
                } else {
                    neighbor_matches(&grid_snap, x, y, &rule.neighbors)
                };
                let Some((nx, ny)) = target else {
                    continue;
                };
                if roll(seed, tick, idx) >= rule.chance_pct {
                    continue;
                }
                match rule.action {
                    ProcessAction::NeighborBecomes(f) => {
                        claimed.insert((ny * w + nx) as usize);
                        changed.push((nx, ny, f, feat, rule.cause));
                    }
                    ProcessAction::SelfBecomes(f) => {
                        claimed.insert(idx);
                        changed.push((x, y, f, feat, rule.cause));
                    }
                    ProcessAction::SelfBecomesOneOf(list) => {
                        let r2 = roll(seed ^ 0xA5A5, tick, idx) as u32;
                        let mut acc = 0u32;
                        let mut pick = list[0].0;
                        for (f, wgt) in list {
                            acc += *wgt as u32;
                            if r2 < acc {
                                pick = *f;
                                break;
                            }
                        }
                        claimed.insert(idx);
                        changed.push((x, y, pick, feat, rule.cause));
                    }
                    ProcessAction::NeighborAndSelf {
                        neighbor,
                        self_becomes,
                    } => {
                        claimed.insert((ny * w + nx) as usize);
                        changed.push((nx, ny, neighbor, feat, rule.cause));
                        if let Some((sf, pct)) = self_becomes {
                            if roll(seed ^ 0x5A5A, tick, idx) < pct {
                                claimed.insert(idx);
                                changed.push((x, y, sf, feat, rule.cause));
                            }
                        }
                    }
                }
            }
        }
    }

    for (x, y, to, from, cause) in &changed {
        world.resource_mut::<Grid>().set(*x, *y, *to);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::TerrainChanged {
                at: (*x, *y),
                from: *from,
                to: *to,
                cause: *cause,
            });
    }

    crate::vision::recompute_glow(world);
}
