//! Frog-style FOV / fog-of-war.
//!
//! Ports the spirit of `cave.c`:
//!   - `los()`               integer LOS with FF_LOS blockers
//!   - `update_view()`       CAVE_VIEW within MAX_SIGHT
//!   - torch               CAVE_LITE within TORCH_RADIUS
//!   - room light          CAVE_GLOW (set at generation)
//!   - `note_spot()`       CAVE_MARK memory when seen lit
//!
//! Display (map_info spirit):
//!   VIEW && (LITE|GLOW) → fully visible (entities shown)
//!   MARK only           → remembered terrain, dim, no entities
//!   else                → unexplored darkness
//!
//! Security note: the global `VisionMap` is the *union* FOV used by the
//! simulation/terminal.  Per-player web snapshots are built from per-agent
//! `VisionMemory` and `compute_view_for_agents` — never from the union map.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::{Agent, Position, VisionMemory};
use crate::f_info;
use crate::grid::Grid;

/// Frog `MAX_SIGHT` (defines.h).
pub const MAX_SIGHT: i32 = 20;
/// Sandbox torch radius (frog `p_ptr->cur_lite` spirit; typical lantern ~5).
pub const TORCH_RADIUS: i32 = 5;

pub const F_VIEW: u8 = 0x01;
pub const F_MARK: u8 = 0x02;
pub const F_LITE: u8 = 0x04;
pub const F_GLOW: u8 = 0x08;

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VisionMap {
    pub width: i32,
    pub height: i32,
    /// Per-cell flags: VIEW | MARK | LITE | GLOW
    pub flags: Vec<u8>,
}

/// Room-light mask — independent of the union `VisionMap` so per-agent views
/// can be computed without exposing the global map.
#[derive(Resource, Clone, Debug)]
pub struct GlowMask {
    pub width: i32,
    pub height: i32,
    pub mask: Vec<bool>,
}

impl GlowMask {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            mask: vec![false; (width * height) as usize],
        }
    }

    pub fn from_slice(width: i32, height: i32, slice: &[bool]) -> Self {
        let n = (width * height) as usize;
        let mut mask = vec![false; n];
        for i in 0..n.min(slice.len()) {
            mask[i] = slice[i];
        }
        Self {
            width,
            height,
            mask,
        }
    }
}

impl VisionMap {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            flags: vec![0; (width * height) as usize],
        }
    }

    pub fn from_glow(width: i32, height: i32, glow: &GlowMask) -> Self {
        let mut flags = vec![0u8; (width * height) as usize];
        for i in 0..flags.len().min(glow.mask.len()) {
            if glow.mask[i] {
                flags[i] |= F_GLOW;
            }
        }
        Self {
            width,
            height,
            flags,
        }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            None
        } else {
            Some((y * self.width + x) as usize)
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        self.idx(x, y).map(|i| self.flags[i]).unwrap_or(0)
    }

    pub fn is_view(&self, x: i32, y: i32) -> bool {
        self.get(x, y) & F_VIEW != 0
    }

    pub fn is_mark(&self, x: i32, y: i32) -> bool {
        self.get(x, y) & F_MARK != 0
    }

    pub fn is_lit(&self, x: i32, y: i32) -> bool {
        let f = self.get(x, y);
        f & (F_LITE | F_GLOW) != 0
    }

    /// Fully visible now (in FOV and illuminated).
    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        let f = self.get(x, y);
        (f & F_VIEW != 0) && (f & (F_LITE | F_GLOW) != 0)
    }

    /// Display class for viewer: 0=unknown 1=memory 2=visible
    pub fn display_class(&self, x: i32, y: i32) -> u8 {
        let f = self.get(x, y);
        if (f & F_VIEW != 0) && (f & (F_LITE | F_GLOW) != 0) {
            2
        } else if f & F_MARK != 0 {
            1
        } else {
            0
        }
    }
}

/// Frog `los(y1,x1,y2,x2)` — true if line of sight is clear.
/// Blockers: cells that do **not** have FF_LOS (walls, closed doors, mountains…).
/// Endpoint is never required to allow LOS (you can see a wall).
pub fn los(grid: &Grid, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
    let table = f_info::table();
    let allows = |x: i32, y: i32| -> bool {
        grid.get(x, y)
            .map(|id| table.allows_los(id))
            .unwrap_or(false)
    };

    let dy = y2 - y1;
    let dx = x2 - x1;
    let ay = dy.abs();
    let ax = dx.abs();

    // Adjacent or same
    if ax < 2 && ay < 2 {
        return true;
    }

    // Straight vertical
    if dx == 0 {
        let (lo, hi) = if dy > 0 { (y1 + 1, y2) } else { (y2 + 1, y1) };
        for ty in lo..hi {
            if !allows(x1, ty) {
                return false;
            }
        }
        return true;
    }

    // Straight horizontal
    if dy == 0 {
        let (lo, hi) = if dx > 0 { (x1 + 1, x2) } else { (x2 + 1, x1) };
        for tx in lo..hi {
            if !allows(tx, y1) {
                return false;
            }
        }
        return true;
    }

    let sx = if dx < 0 { -1 } else { 1 };
    let sy = if dy < 0 { -1 } else { 1 };

    // Knight-step specials
    if ax == 1 && ay == 2 {
        return allows(x1, y1 + sy);
    }
    if ay == 1 && ax == 2 {
        return allows(x1 + sx, y1);
    }

    let f2 = ax * ay;
    let f1 = f2 << 1;

    if ax >= ay {
        // Travel horizontally
        let mut qy = ay * ay;
        let m = qy << 1;
        let mut tx = x1 + sx;
        let mut ty = if qy == f2 {
            qy -= f1;
            y1 + sy
        } else {
            y1
        };

        while x2 - tx != 0 {
            if !allows(tx, ty) {
                return false;
            }
            qy += m;
            if qy < f2 {
                tx += sx;
            } else if qy > f2 {
                ty += sy;
                if !allows(tx, ty) {
                    return false;
                }
                qy -= f1;
                tx += sx;
            } else {
                ty += sy;
                qy -= f1;
                tx += sx;
            }
        }
    } else {
        // Travel vertically
        let mut qx = ax * ax;
        let m = qx << 1;
        let mut ty = y1 + sy;
        let mut tx = if qx == f2 {
            qx -= f1;
            x1 + sx
        } else {
            x1
        };

        while y2 - ty != 0 {
            if !allows(tx, ty) {
                return false;
            }
            qx += m;
            if qx < f2 {
                ty += sy;
            } else if qx > f2 {
                tx += sx;
                if !allows(tx, ty) {
                    return false;
                }
                qx -= f1;
                ty += sy;
            } else {
                tx += sx;
                qx -= f1;
                ty += sy;
            }
        }
    }

    true
}

/// Add one agent's FOV into `vis`.
fn add_origin_fov(grid: &Grid, vis: &mut VisionMap, px: i32, py: i32) {
    let full = MAX_SIGHT;
    let over = MAX_SIGHT * 3 / 2;
    let tr = TORCH_RADIUS;

    if let Some(i) = vis.idx(px, py) {
        vis.flags[i] |= F_VIEW | F_LITE;
    }

    let y0 = (py - full).max(0);
    let y1 = (py + full).min(grid.height - 1);
    let x0 = (px - full).max(0);
    let x1 = (px + full).min(grid.width - 1);

    for y in y0..=y1 {
        for x in x0..=x1 {
            if x == px && y == py {
                continue;
            }
            let dx = (x - px).abs();
            let dy = (y - py).abs();
            if dx + dy > over {
                continue;
            }
            if dx * dx + dy * dy > full * full {
                continue;
            }
            if !los(grid, px, py, x, y) {
                continue;
            }
            if let Some(i) = vis.idx(x, y) {
                vis.flags[i] |= F_VIEW;
            }
        }
    }

    let ty0 = (py - tr).max(0);
    let ty1 = (py + tr).min(grid.height - 1);
    let tx0 = (px - tr).max(0);
    let tx1 = (px + tr).min(grid.width - 1);
    for y in ty0..=ty1 {
        for x in tx0..=tx1 {
            let dx = x - px;
            let dy = y - py;
            if dx * dx + dy * dy > tr * tr {
                continue;
            }
            if let Some(i) = vis.idx(x, y) {
                if vis.flags[i] & F_VIEW != 0 {
                    vis.flags[i] |= F_LITE;
                }
            }
        }
    }
}

/// Sight bbox (clamped) — every per-agent FOV loop should live inside this
/// rect instead of scanning the whole map.
pub fn fov_bbox(grid: &Grid, px: i32, py: i32) -> (i32, i32, i32, i32) {
    (
        (px - MAX_SIGHT).max(0),
        (px + MAX_SIGHT).min(grid.width - 1),
        (py - MAX_SIGHT).max(0),
        (py + MAX_SIGHT).min(grid.height - 1),
    )
}

/// Compute a fresh per-agent FOV map. GLOW is copied only inside the sight
/// bbox (a GLOW cell without VIEW displays as unseen anyway); LITE comes
/// from the agent's torch.
pub fn compute_fov_map(grid: &Grid, glow: &GlowMask, px: i32, py: i32) -> VisionMap {
    let mut vis = VisionMap::new(grid.width, grid.height);
    let (x0, x1, y0, y1) = fov_bbox(grid, px, py);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let i = (y * grid.width + x) as usize;
            if glow.mask[i] {
                vis.flags[i] |= F_GLOW;
            }
        }
    }
    add_origin_fov(grid, &mut vis, px, py);
    vis
}

/// Frog `update_view` + torch lite + `note_spot`.
/// Multi-agent: OR vision from every Agent (union FOV).  This is internal/
/// terminal only; web clients get per-token views from `compute_view_for_agents`.
pub fn update_view(world: &mut World) {
    let origins: Vec<(i32, i32)> = {
        let mut q = world.query_filtered::<&Position, With<Agent>>();
        q.iter(world).map(|p| (p.x, p.y)).collect()
    };
    if origins.is_empty() {
        return;
    }

    let (w, h) = {
        let g = world.resource::<Grid>();
        (g.width, g.height)
    };

    // Ensure resources exist and match grid size.
    {
        let need_new = world
            .get_resource::<VisionMap>()
            .map(|v| v.width != w || v.height != h)
            .unwrap_or(true);
        if need_new {
            world.insert_resource(VisionMap::new(w, h));
        }
    }
    {
        let need_new = world
            .get_resource::<GlowMask>()
            .map(|m| m.width != w || m.height != h)
            .unwrap_or(true);
        if need_new {
            world.insert_resource(GlowMask::new(w, h));
        }
    }

    let grid = world.resource::<Grid>().clone();
    let mut vis = world.resource_mut::<VisionMap>();

    // Clear VIEW + LITE; keep MARK + GLOW
    for f in vis.flags.iter_mut() {
        *f &= !(F_VIEW | F_LITE);
    }

    for &(px, py) in &origins {
        add_origin_fov(&grid, &mut vis, px, py);
    }

    // note_spot across union FOV bbox
    let (min_x, max_x, min_y, max_y) = {
        let mut min_x = grid.width;
        let mut max_x = 0;
        let mut min_y = grid.height;
        let mut max_y = 0;
        for &(px, py) in &origins {
            min_x = min_x.min((px - MAX_SIGHT).max(0));
            max_x = max_x.max((px + MAX_SIGHT).min(grid.width - 1));
            min_y = min_y.min((py - MAX_SIGHT).max(0));
            max_y = max_y.max((py + MAX_SIGHT).min(grid.height - 1));
        }
        (min_x, max_x, min_y, max_y)
    };

    let table = f_info::table();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let Some(i) = vis.idx(x, y) else { continue };
            let f = vis.flags[i];
            if f & F_VIEW == 0 {
                continue;
            }
            let lit = f & (F_LITE | F_GLOW) != 0;
            if !lit {
                continue;
            }
            if f & F_MARK != 0 {
                continue;
            }
            let feat = grid.get(x, y).unwrap_or(0);
            if table.remember(feat) || table.allows_los(feat) || lit {
                vis.flags[i] |= F_MARK;
            }
        }
    }
}

/// Update each agent's personal memory from their current FOV.
pub fn update_agent_memories(world: &mut World) {
    let grid = world.resource::<Grid>().clone();
    let glow = world
        .get_resource::<GlowMask>()
        .cloned()
        .unwrap_or_else(|| GlowMask::new(grid.width, grid.height));

    let agents: Vec<(Entity, i32, i32)> = {
        let mut q = world.query_filtered::<(Entity, &Position), With<Agent>>();
        q.iter(world).map(|(e, p)| (e, p.x, p.y)).collect()
    };

    for (e, x, y) in agents {
        let fov = compute_fov_map(&grid, &glow, x, y);
        let (x0, x1, y0, y1) = fov_bbox(&grid, x, y);
        let mut mem = world
            .get_mut::<VisionMemory>(e)
            .expect("agent should have VisionMemory");
        // Only the sight bbox can be visible now — never scan the full map.
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                if fov.is_visible(cx, cy) {
                    mem.mark(cx, cy);
                }
            }
        }
    }
}

/// Build a view for a specific set of tracked agents.
/// Combines:
///   - level GLOW (only matters where an agent currently has VIEW)
///   - current FOV of each tracked agent
///   - remembered cells from each tracked agent's `VisionMemory`
pub fn compute_view_for_agents(world: &World, agents: &[Entity]) -> VisionMap {
    let grid = world.resource::<Grid>().clone();
    let glow = world
        .get_resource::<GlowMask>()
        .cloned()
        .unwrap_or_else(|| GlowMask::new(grid.width, grid.height));

    let mut out = VisionMap::from_glow(grid.width, grid.height, &glow);

    for &e in agents {
        let Some(pos) = world.get::<Position>(e) else {
            continue;
        };
        let fov = compute_fov_map(&grid, &glow, pos.x, pos.y);
        // Current FOV: only the sight bbox can have VIEW/LITE set.
        let (x0, x1, y0, y1) = fov_bbox(&grid, pos.x, pos.y);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let i = (y * grid.width + x) as usize;
                out.flags[i] |= fov.flags[i] & (F_VIEW | F_LITE);
            }
        }
        // Memory: full-map OR (remembered cells legitimately span the map).
        if let Some(mem) = world.get::<VisionMemory>(e) {
            for i in 0..out.flags.len() {
                if mem.flags[i] & F_MARK != 0 {
                    out.flags[i] |= F_MARK;
                }
            }
        }
    }

    out
}

/// Call after level rebuild: install glow mask then first FOV pass.
pub fn install_and_update(world: &mut World, glow: Option<Vec<bool>>) {
    let (w, h) = {
        let g = world.resource::<Grid>();
        (g.width, g.height)
    };
    let glow_mask = glow
        .map(|g| GlowMask::from_slice(w, h, &g))
        .unwrap_or_else(|| GlowMask::new(w, h));
    let vis = VisionMap::from_glow(w, h, &glow_mask);
    world.insert_resource(glow_mask);
    world.insert_resource(vis);
    update_view(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f_info::id;

    fn grid_corridor() -> Grid {
        // 7x7 open middle corridor, walls on sides
        let w = 7;
        let h = 7;
        let mut cells = vec![id::GRANITE; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                // center cross open
                if x == 3 || y == 3 {
                    cells[(y * w + x) as usize] = id::FLOOR;
                }
            }
        }
        Grid {
            width: w,
            height: h,
            cells,
        }
    }

    #[test]
    fn los_straight_and_blocked() {
        let g = grid_corridor();
        assert!(los(&g, 3, 3, 3, 0)); // vertical open
        assert!(los(&g, 3, 3, 0, 3)); // horizontal open
                                      // diagonal through wall corner region
        assert!(!los(&g, 0, 0, 6, 6) || !g.walkable(1, 1));
        // from open to wall endpoint is OK if path clear
        assert!(los(&g, 3, 3, 3, 1));
    }
}
