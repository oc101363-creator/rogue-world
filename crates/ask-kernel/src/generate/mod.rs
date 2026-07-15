//! FrogComposband-style level generation.
//!
//! Mirrors `generate.c` :: `cave_gen` order:
//!   fill solid → generate_rooms (typed) → scramble centers →
//!   build_tunnel (wall piercings + door list) → apply floors →
//!   lakes / rivers / trees → stairs → place objects.
//!
//! Feat ids are always real `f_info` N: numbers.

mod features;
mod rooms;
mod tunnel;

use crate::config::Config;
use crate::f_info::id;
use crate::grid::Grid;

pub use rooms::Room;

use features::{alloc_traps, destroy_level, maybe_maze_level, stamp_maze_vault};
use rooms::{generate_rooms, DunRooms};
use tunnel::{build_tunnel, correct_dir, DunTunnel};
use crate::vaults::{self, VaultRng};

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1),
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn randint0(&mut self, n: i32) -> i32 {
        if n <= 1 {
            return 0;
        }
        (self.next_u64() % n as u64) as i32
    }

    pub fn randint1(&mut self, n: i32) -> i32 {
        1 + self.randint0(n)
    }

    pub fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.randint0(hi - lo)
    }

    pub fn percent(&mut self, p: i32) -> bool {
        self.randint0(100) < p
    }

    pub fn one_in(&mut self, n: i32) -> bool {
        n > 0 && self.randint0(n) == 0
    }
}

impl VaultRng for Rng {
    fn vault_index(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        self.randint0(n as i32) as usize
    }
}

/// Generation-time cave cell (before f_info bake).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// place_extra / solid diggable rock
    Solid,
    /// place_outer_grid — room rim, tunnel may pierce
    Outer,
    /// place_inner_grid — pillars inside rooms
    Inner,
    /// room floor (CAVE_ROOM)
    Room,
    /// corridor floor
    Tunnel,
}

pub struct Cave {
    pub w: i32,
    pub h: i32,
    pub cells: Vec<Cell>,
}

impl Cave {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            cells: vec![Cell::Solid; (w * h) as usize],
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1
    }

    pub fn get(&self, x: i32, y: i32) -> Cell {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Cell::Solid;
        }
        self.cells[(y * self.w + x) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, v: Cell) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = v;
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedLevel {
    pub grid: Grid,
    pub rooms: Vec<Room>,
    pub agent: (i32, i32),
    pub trees: Vec<(i32, i32)>,
    pub irons: Vec<(i32, i32)>,
}

pub fn generate_level(cfg: &Config) -> GeneratedLevel {
    let _ = crate::f_info::table();

    const BLOCK: i32 = 11;
    let mut w = cfg.width.max(BLOCK * 4);
    let mut h = cfg.height.max(BLOCK * 4);
    w = (w / BLOCK) * BLOCK;
    h = (h / BLOCK) * BLOCK;

    let mut rng = Rng::new(cfg.seed);
    let mut cave = Cave::new(w, h);

    // Rare full maze level (frog DF1_MAZE / build_maze_vault driver)
    let maze_level = maybe_maze_level(&mut cave, &mut rng);

    // --- rooms (frog generate_rooms) ---
    let DunRooms {
        mut rooms,
        room_map: _,
    } = if maze_level {
        DunRooms {
            rooms: Vec::new(),
            room_map: vec![],
        }
    } else {
        generate_rooms(&mut cave, &mut rng)
    };

    // scramble centers
    for i in 0..rooms.len() {
        let pick = rng.randint0((i + 1) as i32) as usize;
        rooms.swap(i, pick);
    }

    // occasional maze vault stamped into a large room
    if !maze_level {
        stamp_maze_vault(&mut cave, &rooms, &mut rng);
    }

    // --- tunnels (frog build_tunnel + wall/door lists) ---
    let mut dun_tun = DunTunnel::default();
    if rooms.len() >= 2 {
        let mut y = rooms[rooms.len() - 1].cy;
        let mut x = rooms[rooms.len() - 1].cx;
        for room in &rooms {
            let _ = build_tunnel(&mut cave, &mut dun_tun, &mut rng, y, x, room.cy, room.cx);
            y = room.cy;
            x = room.cx;
        }
        let extra = (rooms.len() / 4).max(1);
        for _ in 0..extra {
            let a = rng.randint0(rooms.len() as i32) as usize;
            let b = rng.randint0(rooms.len() as i32) as usize;
            if a != b {
                let _ = build_tunnel(
                    &mut cave,
                    &mut dun_tun,
                    &mut rng,
                    rooms[a].cy,
                    rooms[a].cx,
                    rooms[b].cy,
                    rooms[b].cx,
                );
            }
        }
    }

    // frog: occasional destroyed level after rooms
    let destroyed = !maze_level && rng.percent(12);

    let mut feats = bake_base(&cave, &mut rng);
    place_doors(&mut feats, w, h, &dun_tun, &mut rng);
    place_stairs(&mut feats, w, &rooms);
    place_lakes(&mut feats, w, h, &rooms, &mut rng);
    place_rivers(&mut feats, w, h, &mut rng);
    place_tree_patches(&mut feats, w, h, &mut rng);

    // frog vault templates from vaults.txt (lesser/greater)
    place_vault_templates(&mut feats, w, h, &rooms, &mut rng);

    if destroyed {
        destroy_level(&mut feats, w, h, &mut rng);
    }
    // frog _cave_gen_traps — scale with area
    let trap_count = ((w * h) / 1800).clamp(8, 80);
    alloc_traps(&mut feats, w, h, trap_count, &mut rng);

    // permanent border
    for x in 0..w {
        feats[x as usize] = id::PERMANENT;
        feats[((h - 1) * w + x) as usize] = id::PERMANENT;
    }
    for y in 0..h {
        feats[(y * w) as usize] = id::PERMANENT;
        feats[(y * w + w - 1) as usize] = id::PERMANENT;
    }

    let grid = Grid {
        width: w,
        height: h,
        cells: feats,
    };

    let (agent, trees, irons) = place_objects(&grid, &rooms, cfg, &mut rng);

    GeneratedLevel {
        grid,
        rooms,
        agent,
        trees,
        irons,
    }
}

fn bake_base(cave: &Cave, rng: &mut Rng) -> Vec<u16> {
    let w = cave.w;
    let h = cave.h;
    let mut feats = vec![id::GRANITE; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            feats[i] = match cave.get(x, y) {
                Cell::Room => match rng.randint0(100) {
                    0..=6 => id::DIRT,
                    7..=12 => id::GRASS,
                    13..=15 => id::BRAKE,
                    _ => id::FLOOR,
                },
                Cell::Tunnel => id::FLOOR,
                Cell::Outer => id::GRANITE_OUTER,
                Cell::Inner => id::GRANITE_INNER,
                Cell::Solid => match rng.randint0(100) {
                    0..=3 => id::MAGMA_VEIN,
                    4..=6 => id::QUARTZ_VEIN,
                    7 => id::RUBBLE,
                    8 => id::MAGMA_TREASURE,
                    9 => id::QUARTZ_TREASURE,
                    10..=11 => id::GRANITE_SOLID,
                    _ => id::GRANITE,
                },
            };
        }
    }
    feats
}

fn place_doors(feats: &mut [u16], w: i32, h: i32, dun: &DunTunnel, rng: &mut Rng) {
    // wall piercings → open/closed/secret doors (frog converts wall list after tunnel)
    for &(x, y) in &dun.walls {
        if x <= 0 || y <= 0 || x >= w - 1 || y >= h - 1 {
            continue;
        }
        let i = (y * w + x) as usize;
        feats[i] = match rng.randint0(10) {
            0..=5 => id::OPEN_DOOR,
            6..=8 => id::CLOSED_DOOR,
            _ => id::SECRET_DOOR,
        };
    }
    // corridor junctions
    for &(x, y) in &dun.doors {
        if x <= 0 || y <= 0 || x >= w - 1 || y >= h - 1 {
            continue;
        }
        let i = (y * w + x) as usize;
        if feats[i] == id::FLOOR && rng.percent(60) {
            feats[i] = id::OPEN_DOOR;
        }
    }
    // also opportunistic doors: floor next to outer granite
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = (y * w + x) as usize;
            if feats[i] != id::FLOOR {
                continue;
            }
            let mut adj = 0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if feats[((y + dy) * w + (x + dx)) as usize] == id::GRANITE_OUTER {
                    adj += 1;
                }
            }
            if adj >= 1 && rng.percent(8) {
                feats[i] = if rng.percent(70) {
                    id::OPEN_DOOR
                } else {
                    id::CLOSED_DOOR
                };
            }
        }
    }
}

fn place_stairs(feats: &mut [u16], w: i32, rooms: &[Room]) {
    if rooms.is_empty() {
        return;
    }
    let r0 = &rooms[0];
    feats[(r0.cy * w + r0.cx) as usize] = id::UP_STAIR;
    if rooms.len() >= 2 {
        let r1 = &rooms[rooms.len() - 1];
        let mut x = r1.cx;
        let y = r1.cy;
        if x == r0.cx && y == r0.cy {
            x = (r1.cx + 1).min(r1.x2 - 1);
        }
        feats[(y * w + x) as usize] = id::DOWN_STAIR;
    }
}

/// Frog lakes (simplified circular pools of water/lava/cave rubble).
fn place_lakes(feats: &mut [u16], w: i32, h: i32, rooms: &[Room], rng: &mut Rng) {
    let n = (rooms.len() / 4).max(1).min(12);
    for _ in 0..n {
        if rooms.is_empty() {
            break;
        }
        let kind = rng.randint0(3); // 0 water 1 lava 2 rubble cave
        let r = &rooms[rng.randint0(rooms.len() as i32) as usize];
        let cy = rng.rand_range(r.y1 + 1, r.y2.max(r.y1 + 2));
        let cx = rng.rand_range(r.x1 + 1, r.x2.max(r.x1 + 2));
        let rad = rng.rand_range(2, 5);
        for yy in (cy - rad)..=(cy + rad) {
            for xx in (cx - rad)..=(cx + rad) {
                if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                    continue;
                }
                let d = (xx - cx) * (xx - cx) + (yy - cy) * (yy - cy);
                if d > rad * rad {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                // don't overwrite permanent
                if feats[i] == id::PERMANENT {
                    continue;
                }
                feats[i] = match kind {
                    0 => {
                        if d <= 1 {
                            id::DEEP_WATER
                        } else {
                            id::SHALLOW_WATER
                        }
                    }
                    1 => {
                        if d <= 1 {
                            id::DEEP_LAVA
                        } else {
                            id::SHALLOW_LAVA
                        }
                    }
                    _ => {
                        if d <= 1 {
                            id::DARK_PIT
                        } else {
                            id::RUBBLE
                        }
                    }
                };
            }
        }
    }
}

/// Frog `add_river` — drunk-walk water/lava stream.
fn place_rivers(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    if !rng.percent(55) {
        return;
    }
    let deep = if rng.percent(50) {
        id::DEEP_WATER
    } else {
        id::DEEP_LAVA
    };
    let shallow = if deep == id::DEEP_WATER {
        id::SHALLOW_WATER
    } else {
        id::SHALLOW_LAVA
    };

    let mut x = rng.rand_range(2, w - 2);
    let mut y = if rng.percent(50) { 1 } else { h - 2 };
    let target_y = if y == 1 { h - 2 } else { 1 };
    let mut steps = 0;
    while steps < w + h * 2 {
        steps += 1;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let nx = x + ox;
                let ny = y + oy;
                if nx <= 0 || ny <= 0 || nx >= w - 1 || ny >= h - 1 {
                    continue;
                }
                let i = (ny * w + nx) as usize;
                if feats[i] == id::PERMANENT {
                    continue;
                }
                feats[i] = if ox == 0 && oy == 0 { deep } else { shallow };
            }
        }
        if y == target_y {
            break;
        }
        // prefer toward target, with wiggle
        let (dy, dx) = correct_dir(y, x, target_y, rng.rand_range(2, w - 2));
        if rng.percent(30) {
            let (rdy, rdx) = match rng.randint0(4) {
                0 => (-1, 0),
                1 => (1, 0),
                2 => (0, -1),
                _ => (0, 1),
            };
            y = (y + rdy).clamp(1, h - 2);
            x = (x + rdx).clamp(1, w - 2);
        } else {
            y = (y + dy).clamp(1, h - 2);
            x = (x + dx).clamp(1, w - 2);
        }
    }
}

/// Frog `place_trees` clusters.
fn place_tree_patches(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let patches = rng.rand_range(3, 10);
    for _ in 0..patches {
        let cx = rng.rand_range(3, w - 3);
        let cy = rng.rand_range(3, h - 3);
        let rad = rng.rand_range(2, 6);
        for yy in (cy - rad)..=(cy + rad) {
            for xx in (cx - rad)..=(cx + rad) {
                if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                    continue;
                }
                if (xx - cx).abs() + (yy - cy).abs() > rad {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                // only convert floors / grass / dirt
                if matches!(feats[i], id::FLOOR | id::DIRT | id::GRASS | id::BRAKE)
                    && rng.percent(70)
                {
                    feats[i] = if rng.percent(80) {
                        id::TREE
                    } else {
                        id::BRAKE
                    };
                }
            }
        }
    }
}

/// Stamp 1–3 vaults from frog vaults.txt into free-ish regions.
fn place_vault_templates(
    feats: &mut [u16],
    w: i32,
    h: i32,
    rooms: &[Room],
    rng: &mut Rng,
) {
    let _ = vaults::table(); // ensure loaded

    // Always try at least one lesser vault when map is large enough
    let tries_lesser = if w * h > 20_000 { 3 } else { 2 };
    let tries_greater = if w * h > 40_000 { 2 } else { 1 };

    for _ in 0..tries_greater {
        if !rng.percent(45) {
            continue;
        }
        if let Some(v) = vaults::pick_vault(rng, true) {
            try_stamp(feats, w, h, rooms, rng, v);
        }
    }
    for _ in 0..tries_lesser {
        if !rng.percent(70) {
            continue;
        }
        if let Some(v) = vaults::pick_vault(rng, false) {
            try_stamp(feats, w, h, rooms, rng, v);
        }
    }
}

fn try_stamp(
    feats: &mut [u16],
    w: i32,
    h: i32,
    rooms: &[Room],
    rng: &mut Rng,
    v: &vaults::VaultTemplate,
) {
    let vw = v.width();
    let vh = v.height();
    if vw + 4 >= w || vh + 4 >= h {
        return;
    }
    // Prefer near a random room center, else random
    for _ in 0..30 {
        let (ox, oy) = if !rooms.is_empty() && rng.percent(70) {
            let r = &rooms[rng.randint0(rooms.len() as i32) as usize];
            (
                (r.cx - vw / 2).clamp(2, w - vw - 2),
                (r.cy - vh / 2).clamp(2, h - vh - 2),
            )
        } else {
            (
                rng.rand_range(2, w - vw - 2),
                rng.rand_range(2, h - vh - 2),
            )
        };
        if vaults::stamp_vault(feats, w, h, ox, oy, v) {
            return;
        }
    }
}

fn place_objects(
    grid: &Grid,
    rooms: &[Room],
    cfg: &Config,
    rng: &mut Rng,
) -> ((i32, i32), Vec<(i32, i32)>, Vec<(i32, i32)>) {
    let w = grid.width;
    let h = grid.height;
    let mut floors = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if !grid.walkable(x, y) {
                continue;
            }
            let f = grid.get(x, y).unwrap_or(0);
            if matches!(
                f,
                id::FLOOR | id::DIRT | id::GRASS | id::BRAKE | id::SHALLOW_WATER
            ) {
                floors.push((x, y));
            }
        }
    }
    for i in (1..floors.len()).rev() {
        let j = rng.randint0((i + 1) as i32) as usize;
        floors.swap(i, j);
    }

    let agent = if !rooms.is_empty() {
        let (cx, cy) = (rooms[0].cx, rooms[0].cy);
        if grid.get(cx, cy) == Some(id::UP_STAIR) {
            (cx + 1, cy)
        } else {
            (cx, cy)
        }
    } else if !floors.is_empty() {
        floors[0]
    } else {
        (w / 2, h / 2)
    };

    let mut used = std::collections::HashSet::new();
    used.insert(agent);
    let mut trees = Vec::new();
    let mut irons = Vec::new();
    let mut fi = 0usize;
    while trees.len() < cfg.tree_count as usize && fi < floors.len() {
        let p = floors[fi];
        fi += 1;
        if used.insert(p) {
            trees.push(p);
        }
    }
    while irons.len() < cfg.iron_count as usize && fi < floors.len() {
        let p = floors[fi];
        fi += 1;
        if used.insert(p) {
            irons.push(p);
        }
    }
    (agent, trees, irons)
}
