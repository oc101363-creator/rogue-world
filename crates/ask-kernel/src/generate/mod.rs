//! FrogComposband-style level generation.
//!
//! Mirrors `generate.c` :: `cave_gen` order:
//!   fill solid → generate_rooms (typed) → scramble centers →
//!   build_tunnel (wall piercings + door list) → apply floors →
//!   lakes / rivers / trees → stairs → place objects.
//!
//! Feat ids are always real `f_info` N: numbers.

mod alloc;
mod features;
mod rooms;
mod tunnel;

use crate::config::Config;
use crate::f_info::id;
use crate::grid::Grid;

pub use rooms::Room;

use crate::vaults::{self, TemplateRng};
use alloc::{alloc_monsters, alloc_objects, fill_trap_room};
use features::{alloc_traps, destroy_level, maybe_maze_level, stamp_maze_vault};
use rooms::{generate_rooms, DunRooms, RoomKind};
use tunnel::{build_tunnel, correct_dir, DunTunnel};

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
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

impl TemplateRng for Rng {
    fn pick(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        self.randint0(n as i32) as usize
    }

    fn next_u64(&mut self) -> u64 {
        Rng::next_u64(self)
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
    pub monsters: Vec<vaults::SpawnMon>,
    pub items: Vec<vaults::SpawnObj>,
    /// Frog CAVE_GLOW — room interiors are permanently lit.
    pub glow: Vec<bool>,
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

    // Maze levels are very rocky; keep rare for open-world feel
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
        // More cross-links → less isolated rock between rooms
        let extra = (rooms.len() / 2).max(2);
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

    // Mild open-up only (keep big rock masses)
    open_rock_pockets(&mut cave, &mut rng);
    organicize_rock(&mut cave, &mut rng);

    let destroyed = !maze_level && rng.percent(6);

    let mut feats = bake_base(&cave, &mut rng);
    // Contiguous biomes first so later features sit on readable terrain
    place_biomes(&mut feats, w, h, &mut rng);
    // Big mountain / highland masses (connected ranges, not salt-and-pepper)
    place_mountain_ranges(&mut feats, w, h, &mut rng);

    place_doors(&mut feats, w, h, &dun_tun, &mut rng);
    place_stairs(&mut feats, w, &rooms);
    place_lakes(&mut feats, w, h, &rooms, &mut rng);
    place_rivers(&mut feats, w, h, &mut rng);
    place_tree_patches(&mut feats, w, h, &mut rng);

    let mut monsters = Vec::new();
    let mut items = Vec::new();
    place_templates(
        &mut feats,
        w,
        h,
        &rooms,
        &mut rng,
        &mut monsters,
        &mut items,
    );

    // trap rooms (build_type14): denser traps inside trap-kind rooms
    for r in &rooms {
        if r.kind == RoomKind::Trap {
            fill_trap_room(&mut feats, w, r.x1, r.y1, r.x2, r.y2, &mut rng);
        }
    }

    if destroyed {
        destroy_level(&mut feats, w, h, &mut rng);
    }
    let trap_count = ((w * h) / 1800).clamp(8, 80);
    alloc_traps(&mut feats, w, h, trap_count, &mut rng);

    for x in 0..w {
        feats[x as usize] = id::PERMANENT;
        feats[((h - 1) * w + x) as usize] = id::PERMANENT;
    }
    for y in 0..h {
        feats[(y * w) as usize] = id::PERMANENT;
        feats[(y * w + w - 1) as usize] = id::PERMANENT;
    }

    // Frog room light: floor cells that were Room get CAVE_GLOW
    let mut glow = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            if matches!(cave.get(x, y), Cell::Room) {
                glow[(y * w + x) as usize] = true;
            }
        }
    }
    // also light adjacent walls of rooms a bit (so walls of lit rooms are visible)
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = (y * w + x) as usize;
            if glow[i] {
                continue;
            }
            let mut adj = false;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if glow[((y + dy) * w + (x + dx)) as usize] {
                    adj = true;
                    break;
                }
            }
            if adj {
                // only non-LOS walls / doors get wall-glow helper; floors already glow
                let id = feats[i];
                if !crate::f_info::table().allows_los(id) {
                    glow[i] = true;
                }
            }
        }
    }

    let grid = Grid {
        width: w,
        height: h,
        cells: feats,
    };

    let (agent, trees, irons) = place_objects(&grid, &rooms, cfg, &mut rng);

    // frog _cave_gen_monsters / _cave_gen_objects (extra fill)
    let depth = 0u32; // starting depth; Sim can re-gen with higher later
    alloc_monsters(&grid, depth, &mut rng, &mut monsters);
    alloc_objects(&grid, depth, &mut rng, &mut items);

    GeneratedLevel {
        grid,
        rooms,
        agent,
        trees,
        irons,
        monsters,
        items,
        glow,
    }
}

/// Turn solid cells that touch open space into floor (boost open ratio toward ~80%).
fn open_rock_pockets(cave: &mut Cave, rng: &mut Rng) {
    // Fewer/milder passes — keep large rock bodies for mountains later
    for _ in 0..2 {
        let mut to_open = Vec::new();
        for y in 1..cave.h - 1 {
            for x in 1..cave.w - 1 {
                if cave.get(x, y) != Cell::Solid {
                    continue;
                }
                let mut open_n = 0;
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    match cave.get(x + dx, y + dy) {
                        Cell::Room | Cell::Tunnel => open_n += 1,
                        _ => {}
                    }
                }
                if open_n >= 2 && rng.percent(40 + open_n * 10) {
                    to_open.push((x, y));
                } else if open_n == 0 && rng.percent(3) {
                    to_open.push((x, y));
                }
            }
        }
        for (x, y) in to_open {
            cave.set(x, y, Cell::Tunnel);
        }
    }
}

/// Paint large contiguous biome fields (grass / dirt plains) on open floor.
/// Uses seed blobs + soft growth so regions read as patches, not salt-and-pepper.
fn place_biomes(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let area = w * h;
    // A handful of large fields scaled to map size
    let n_fields = (area / 12_000).clamp(6, 18);
    for _ in 0..n_fields {
        let cx = rng.rand_range(w / 10, w - w / 10);
        let cy = rng.rand_range(h / 10, h - h / 10);
        let rad_x = rng.rand_range(14, 36);
        let rad_y = rng.rand_range(10, 28);
        let biome = if rng.percent(55) {
            id::GRASS
        } else {
            id::DIRT
        };
        for yy in (cy - rad_y)..=(cy + rad_y) {
            for xx in (cx - rad_x)..=(cx + rad_x) {
                if xx <= 1 || yy <= 1 || xx >= w - 2 || yy >= h - 2 {
                    continue;
                }
                let nx = (xx - cx) as f32 / rad_x as f32;
                let ny = (yy - cy) as f32 / rad_y as f32;
                let d = nx * nx + ny * ny;
                let noise = (rng.randint0(50) as f32 - 25.0) * 0.012;
                if d > 1.0 + noise {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                // only recolor plain floors — leave rock/water alone
                if feats[i] != id::FLOOR {
                    continue;
                }
                // denser core, softer rim
                let p = if d < 0.55 {
                    92
                } else if d < 0.85 {
                    70
                } else {
                    40
                };
                if rng.percent(p) {
                    feats[i] = biome;
                }
            }
        }
    }
}

/// Large irregular mountain / highland masses (f_info MOUNTAIN).
/// Fewer, bigger, worm-linked ranges so they read as continuous highlands.
fn place_mountain_ranges(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let n = rng.rand_range(2, 5); // fewer main ranges
    let mut centers: Vec<(i32, i32, i32, i32)> = Vec::new(); // cx,cy,rad_x,rad_y
    for i in 0..n {
        let (cx, cy, rad_x, rad_y) = if i == 0 || centers.is_empty() {
            (
                rng.rand_range(w / 8, w - w / 8),
                rng.rand_range(h / 8, h - h / 8),
                rng.rand_range(22, 48),
                rng.rand_range(16, 36),
            )
        } else {
            // chain from previous center so ranges connect into a ridge
            let (px, py, prx, pry) = centers[centers.len() - 1];
            let link = rng.rand_range(12, 28);
            let ang = rng.randint0(8); // 8-way
            let (dx, dy) = match ang {
                0 => (1, 0),
                1 => (1, 1),
                2 => (0, 1),
                3 => (-1, 1),
                4 => (-1, 0),
                5 => (-1, -1),
                6 => (0, -1),
                _ => (1, -1),
            };
            let nrx = rng.rand_range(18, 40);
            let nry = rng.rand_range(14, 32);
            (
                (px + dx * (prx + link) / 2).clamp(w / 8, w - w / 8),
                (py + dy * (pry + link) / 2).clamp(h / 8, h - h / 8),
                nrx,
                nry,
            )
        };
        centers.push((cx, cy, rad_x, rad_y));
        paint_mountain_blob(feats, w, h, cx, cy, rad_x, rad_y, rng);

        // solid foothill connectors toward previous range
        if i > 0 {
            let (px, py, _, _) = centers[i as usize - 1];
            paint_mountain_ridge(feats, w, h, px, py, cx, cy, rng);
        }
    }
}

fn paint_mountain_blob(
    feats: &mut [u16],
    w: i32,
    h: i32,
    cx: i32,
    cy: i32,
    rad_x: i32,
    rad_y: i32,
    rng: &mut Rng,
) {
    for yy in (cy - rad_y)..=(cy + rad_y) {
        for xx in (cx - rad_x)..=(cx + rad_x) {
            if xx <= 1 || yy <= 1 || xx >= w - 2 || yy >= h - 2 {
                continue;
            }
            let nx = (xx - cx) as f32 / rad_x as f32;
            let ny = (yy - cy) as f32 / rad_y as f32;
            let d = nx * nx + ny * ny;
            let noise = (rng.randint0(30) as f32 - 15.0) * 0.008;
            if d > 1.0 + noise {
                continue;
            }
            let i = (yy * w + xx) as usize;
            if feats[i] == id::PERMANENT {
                continue;
            }
            // dense core, soft rim — high fill so it reads solid
            if d < 0.5 {
                feats[i] = id::MOUNTAIN;
            } else if d < 0.8 {
                feats[i] = if rng.percent(85) {
                    id::MOUNTAIN
                } else {
                    id::GRANITE
                };
            } else if rng.percent(75) {
                feats[i] = if rng.percent(35) {
                    id::RUBBLE
                } else {
                    id::GRANITE
                };
            }
        }
    }
}

/// Thick worm corridor of mountain between two peaks.
fn paint_mountain_ridge(
    feats: &mut [u16],
    w: i32,
    h: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    rng: &mut Rng,
) {
    let mut x = x0;
    let mut y = y0;
    let steps = ((x1 - x0).abs() + (y1 - y0).abs()).max(8) * 2;
    for _ in 0..steps {
        let half = rng.rand_range(3, 7);
        for yy in (y - half)..=(y + half) {
            for xx in (x - half)..=(x + half) {
                if xx <= 1 || yy <= 1 || xx >= w - 2 || yy >= h - 2 {
                    continue;
                }
                if (xx - x) * (xx - x) + (yy - y) * (yy - y) > half * half {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                if feats[i] == id::PERMANENT {
                    continue;
                }
                if rng.percent(90) {
                    feats[i] = if rng.percent(75) {
                        id::MOUNTAIN
                    } else {
                        id::GRANITE
                    };
                }
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        let (dy, dx) = correct_dir(y, x, y1, x1);
        // mostly advance toward target, light meander
        if rng.percent(18) {
            let (rdy, rdx) = match rng.randint0(4) {
                0 => (-1, 0),
                1 => (1, 0),
                2 => (0, -1),
                _ => (0, 1),
            };
            y = (y + rdy).clamp(2, h - 3);
            x = (x + rdx).clamp(2, w - 3);
        } else {
            y = (y + dy).clamp(2, h - 3);
            x = (x + dx).clamp(2, w - 3);
        }
    }
}

fn is_rock(c: Cell) -> bool {
    matches!(c, Cell::Solid | Cell::Outer | Cell::Inner)
}

fn is_open(c: Cell) -> bool {
    matches!(c, Cell::Room | Cell::Tunnel)
}

/// Break straight 1-cell wall rings into irregular rock piles / blobs.
fn organicize_rock(cave: &mut Cave, rng: &mut Rng) {
    // 1) Nibble rectangular Outer walls: random cells become floor or thicken into Solid
    let mut nibble = Vec::new();
    let mut thicken = Vec::new();
    for y in 2..cave.h - 2 {
        for x in 2..cave.w - 2 {
            if cave.get(x, y) != Cell::Outer {
                continue;
            }
            let r = rng.randint0(100);
            if r < 35 {
                nibble.push((x, y)); // open gap / ragged edge
            } else if r < 55 {
                thicken.push((x, y)); // keep as rock mass
            }
        }
    }
    for (x, y) in nibble {
        cave.set(x, y, Cell::Room);
    }
    for (x, y) in thicken {
        cave.set(x, y, Cell::Solid);
        // pile: randomly add adjacent solid blobs
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1)] {
            if rng.percent(40) {
                let nx = x + dx;
                let ny = y + dy;
                if cave.in_bounds(nx, ny) && is_open(cave.get(nx, ny)) && rng.percent(50) {
                    // only thicken into solid from outer, don't fill rooms completely
                    if cave.get(x, y) == Cell::Solid {
                        // grow rock one step into open sometimes
                        if rng.percent(30) {
                            cave.set(nx, ny, Cell::Solid);
                        }
                    }
                } else if cave.in_bounds(nx, ny) && cave.get(nx, ny) == Cell::Solid {
                    // ok
                }
            }
        }
    }

    // 2) Cellular automata on rock vs open — smooth into clumps, kill 1-wide lines
    for _ in 0..4 {
        let mut next: Vec<(i32, i32, Cell)> = Vec::new();
        for y in 2..cave.h - 2 {
            for x in 2..cave.w - 2 {
                let cur = cave.get(x, y);
                if matches!(cur, Cell::Room) {
                    // don't destroy room interiors en masse
                    continue;
                }
                let mut rock_n = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if is_rock(cave.get(x + dx, y + dy)) {
                            rock_n += 1;
                        }
                    }
                }
                if is_rock(cur) {
                    // isolated / line rock → open
                    if rock_n <= 2 {
                        next.push((x, y, Cell::Tunnel));
                    } else if rock_n >= 5 && cur == Cell::Outer {
                        next.push((x, y, Cell::Solid)); // merge into pile
                    }
                } else if cur == Cell::Tunnel {
                    // fill thin corridors' jagged sides into soft rock clumps sometimes
                    if rock_n >= 6 && rng.percent(25) {
                        next.push((x, y, Cell::Solid));
                    }
                }
            }
        }
        for (x, y, c) in next {
            cave.set(x, y, c);
        }
    }

    // 3) A few larger rock outcrops only — avoid salt-and-pepper micro piles
    for _ in 0..(cave.w * cave.h / 2500).max(4).min(12) {
        let cx = rng.rand_range(4, cave.w - 4);
        let cy = rng.rand_range(4, cave.h - 4);
        if !is_open(cave.get(cx, cy)) {
            continue;
        }
        let rad = rng.rand_range(3, 7);
        for yy in (cy - rad)..=(cy + rad) {
            for xx in (cx - rad)..=(cx + rad) {
                if !cave.in_bounds(xx, yy) {
                    continue;
                }
                let d = (xx - cx) * (xx - cx) + (yy - cy) * (yy - cy);
                if d > rad * rad {
                    continue;
                }
                if matches!(cave.get(xx, yy), Cell::Room) {
                    continue; // leave room interiors alone
                }
                if is_open(cave.get(xx, yy)) && rng.percent(if d < rad * rad / 2 { 85 } else { 55 })
                {
                    cave.set(xx, yy, Cell::Solid);
                }
            }
        }
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
                // Outer treated as piled rock (same family as solid), not a distinct wireframe
                Cell::Outer | Cell::Inner | Cell::Solid => match rng.randint0(100) {
                    0..=4 => id::MAGMA_VEIN,
                    5..=8 => id::QUARTZ_VEIN,
                    9..=12 => id::RUBBLE,
                    13 => id::MAGMA_TREASURE,
                    14 => id::QUARTZ_TREASURE,
                    15..=18 => id::GRANITE_SOLID,
                    19..=22 => id::GRANITE_INNER,
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

/// Large lakes in open areas (not tiny room puddles).
fn place_lakes(feats: &mut [u16], w: i32, h: i32, rooms: &[Room], rng: &mut Rng) {
    let area = w * h;
    let n = (area / 14_000).clamp(3, 8);
    for li in 0..n {
        let kind = if rng.percent(70) {
            0 // water
        } else if rng.percent(50) {
            1 // lava
        } else {
            2 // rubble hollow
        };
        // prefer open map, sometimes near a room
        let (cx, cy) = if !rooms.is_empty() && rng.percent(40) {
            let r = &rooms[rng.randint0(rooms.len() as i32) as usize];
            (r.cx, r.cy)
        } else {
            (
                rng.rand_range(w / 8, w - w / 8),
                rng.rand_range(h / 8, h - h / 8),
            )
        };
        let rad = rng.rand_range(6, 16);
        // slightly elongate some lakes
        let rad_x = rad + rng.rand_range(0, 6);
        let rad_y = rad + rng.rand_range(0, 4);
        for yy in (cy - rad_y)..=(cy + rad_y) {
            for xx in (cx - rad_x)..=(cx + rad_x) {
                if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                    continue;
                }
                let nx = (xx - cx) as f32 / rad_x as f32;
                let ny = (yy - cy) as f32 / rad_y as f32;
                let d = nx * nx + ny * ny;
                let noise = (rng.randint0(40) as f32 - 20.0) * 0.01;
                if d > 1.0 + noise {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                if feats[i] == id::PERMANENT || feats[i] == id::MOUNTAIN {
                    continue;
                }
                feats[i] = match kind {
                    0 => {
                        if d < 0.35 {
                            id::DEEP_WATER
                        } else {
                            id::SHALLOW_WATER
                        }
                    }
                    1 => {
                        if d < 0.35 {
                            id::DEEP_LAVA
                        } else {
                            id::SHALLOW_LAVA
                        }
                    }
                    _ => {
                        if d < 0.3 {
                            id::DARK_PIT
                        } else {
                            id::RUBBLE
                        }
                    }
                };
            }
        }
        // second blob for the biggest lakes so they feel irregular
        if li % 2 == 0 && rad >= 10 {
            let ox = cx + rng.rand_range(-rad / 2, rad / 2 + 1);
            let oy = cy + rng.rand_range(-rad / 2, rad / 2 + 1);
            let r2 = rad * 2 / 3;
            for yy in (oy - r2)..=(oy + r2) {
                for xx in (ox - r2)..=(ox + r2) {
                    if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                        continue;
                    }
                    if (xx - ox) * (xx - ox) + (yy - oy) * (yy - oy) > r2 * r2 {
                        continue;
                    }
                    let i = (yy * w + xx) as usize;
                    if feats[i] == id::PERMANENT {
                        continue;
                    }
                    if kind == 0 {
                        feats[i] = id::SHALLOW_WATER;
                    } else if kind == 1 {
                        feats[i] = id::SHALLOW_LAVA;
                    }
                }
            }
        }
    }
}

/// Continuous wide rivers (1–3 guaranteed on large maps).
fn place_rivers(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let n_rivers = if w * h > 40_000 {
        rng.rand_range(1, 4)
    } else {
        1
    };
    for ri in 0..n_rivers {
        let water = ri == 0 || rng.percent(75);
        let deep = if water {
            id::DEEP_WATER
        } else {
            id::DEEP_LAVA
        };
        let shallow = if water {
            id::SHALLOW_WATER
        } else {
            id::SHALLOW_LAVA
        };

        // Prefer long N-S or E-W spans
        let vertical = rng.percent(55);
        let (mut x, mut y, target_x, target_y) = if vertical {
            let x0 = rng.rand_range(w / 6, w - w / 6);
            (
                x0,
                if rng.percent(50) { 1 } else { h - 2 },
                x0 + rng.rand_range(-w / 5, w / 5 + 1),
                if rng.percent(50) { h - 2 } else { 1 },
            )
        } else {
            let y0 = rng.rand_range(h / 6, h - h / 6);
            (
                if rng.percent(50) { 1 } else { w - 2 },
                y0,
                if rng.percent(50) { w - 2 } else { 1 },
                y0 + rng.rand_range(-h / 5, h / 5 + 1),
            )
        };
        let target_x = target_x.clamp(2, w - 3);
        let target_y = target_y.clamp(2, h - 3);
        let max_steps = (w + h) * 3;
        for _ in 0..max_steps {
            // wide channel: core deep + shallow banks
            let half = rng.rand_range(2, 5);
            for oy in -half..=half {
                for ox in -half..=half {
                    let nx = x + ox;
                    let ny = y + oy;
                    if nx <= 0 || ny <= 0 || nx >= w - 1 || ny >= h - 1 {
                        continue;
                    }
                    let dd = ox * ox + oy * oy;
                    if dd > half * half {
                        continue;
                    }
                    let i = (ny * w + nx) as usize;
                    if feats[i] == id::PERMANENT {
                        continue;
                    }
                    // don't erase mountain cores completely — allow river gorge
                    feats[i] = if dd <= 1 { deep } else { shallow };
                }
            }
            if (x - target_x).abs() <= 1 && (y - target_y).abs() <= 1 {
                break;
            }
            let (dy, dx) = correct_dir(y, x, target_y, target_x);
            // gentle meander, mostly forward
            if rng.percent(22) {
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
}

/// Dense forest patches — few large woods, high fill.
fn place_tree_patches(feats: &mut [u16], w: i32, h: i32, rng: &mut Rng) {
    let area = w * h;
    let patches = (area / 18_000).clamp(3, 8);
    for _ in 0..patches {
        let cx = rng.rand_range(w / 10, w - w / 10);
        let cy = rng.rand_range(h / 10, h - h / 10);
        let rad_x = rng.rand_range(10, 24);
        let rad_y = rng.rand_range(8, 18);
        for yy in (cy - rad_y)..=(cy + rad_y) {
            for xx in (cx - rad_x)..=(cx + rad_x) {
                if xx <= 1 || yy <= 1 || xx >= w - 2 || yy >= h - 2 {
                    continue;
                }
                let nx = (xx - cx) as f32 / rad_x as f32;
                let ny = (yy - cy) as f32 / rad_y as f32;
                let d = nx * nx + ny * ny;
                let noise = (rng.randint0(40) as f32 - 20.0) * 0.01;
                if d > 1.0 + noise {
                    continue;
                }
                let i = (yy * w + xx) as usize;
                // plant forests on soft open terrain only
                if !matches!(
                    feats[i],
                    id::FLOOR | id::DIRT | id::GRASS | id::BRAKE | id::FLOWER
                ) {
                    continue;
                }
                let p = if d < 0.5 {
                    92
                } else if d < 0.8 {
                    78
                } else {
                    50
                };
                if rng.percent(p) {
                    feats[i] = if rng.percent(82) {
                        id::TREE
                    } else if rng.percent(60) {
                        id::BRAKE
                    } else {
                        id::GRASS
                    };
                }
            }
        }
        // satellite groves glued to the main wood
        for _ in 0..rng.rand_range(1, 4) {
            let gx = cx + rng.rand_range(-rad_x, rad_x + 1);
            let gy = cy + rng.rand_range(-rad_y, rad_y + 1);
            let gr = rng.rand_range(4, 9);
            for yy in (gy - gr)..=(gy + gr) {
                for xx in (gx - gr)..=(gx + gr) {
                    if xx <= 1 || yy <= 1 || xx >= w - 2 || yy >= h - 2 {
                        continue;
                    }
                    if (xx - gx) * (xx - gx) + (yy - gy) * (yy - gy) > gr * gr {
                        continue;
                    }
                    let i = (yy * w + xx) as usize;
                    if matches!(feats[i], id::FLOOR | id::DIRT | id::GRASS | id::BRAKE)
                        && rng.percent(80)
                    {
                        feats[i] = id::TREE;
                    }
                }
            }
        }
    }
}

/// Stamp frog vaults.txt + rooms.txt templates (generate_rooms TEMPLATE path).
fn place_templates(
    feats: &mut [u16],
    w: i32,
    h: i32,
    rooms: &[Room],
    rng: &mut Rng,
    mons: &mut Vec<vaults::SpawnMon>,
    objs: &mut Vec<vaults::SpawnObj>,
) {
    let tries_greater = if w * h > 40_000 { 2 } else { 1 };
    let tries_lesser = if w * h > 20_000 { 3 } else { 2 };
    let tries_room = if w * h > 20_000 { 4 } else { 2 };

    for _ in 0..tries_greater {
        if rng.percent(50) {
            if let Some(v) = vaults::pick_greater(rng) {
                try_stamp(feats, w, h, rooms, rng, v, mons, objs);
            }
        }
    }
    for _ in 0..tries_lesser {
        if rng.percent(75) {
            if let Some(v) = vaults::pick_lesser(rng) {
                try_stamp(feats, w, h, rooms, rng, v, mons, objs);
            }
        }
    }
    for _ in 0..tries_room {
        if rng.percent(80) {
            if let Some(v) = vaults::pick_room(rng) {
                try_stamp(feats, w, h, rooms, rng, v, mons, objs);
            }
        }
    }
}

fn try_stamp(
    feats: &mut [u16],
    w: i32,
    h: i32,
    rooms: &[Room],
    rng: &mut Rng,
    v: &vaults::MapTemplate,
    mons: &mut Vec<vaults::SpawnMon>,
    objs: &mut Vec<vaults::SpawnObj>,
) {
    let vw = v.width();
    let vh = v.height();
    if vw + 4 >= w || vh + 4 >= h {
        return;
    }
    for _ in 0..40 {
        let (ox, oy) = if !rooms.is_empty() && rng.percent(70) {
            let r = &rooms[rng.randint0(rooms.len() as i32) as usize];
            (
                (r.cx - vw / 2).clamp(2, w - vw - 2),
                (r.cy - vh / 2).clamp(2, h - vh - 2),
            )
        } else {
            (rng.rand_range(2, w - vw - 2), rng.rand_range(2, h - vh - 2))
        };
        if vaults::stamp_template(feats, w, h, ox, oy, v, rng, mons, objs) {
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

    // Soft walkable open for agent spawn
    let mut floors = Vec::new();
    // Forest floor candidates (terrain already wooded)
    let mut forest = Vec::new();
    // Near-mountain candidates for iron veins
    let mut foothills = Vec::new();

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let f = grid.get(x, y).unwrap_or(0);
            if matches!(
                f,
                id::FLOOR | id::DIRT | id::GRASS | id::BRAKE | id::SHALLOW_WATER
            ) {
                floors.push((x, y));
            }
            if matches!(f, id::TREE | id::BRAKE | id::GRASS) {
                forest.push((x, y));
            }
            // iron prefers mountain-adjacent granite/rubble/floor
            if matches!(
                f,
                id::GRANITE
                    | id::GRANITE_INNER
                    | id::GRANITE_SOLID
                    | id::RUBBLE
                    | id::MOUNTAIN
                    | id::FLOOR
                    | id::DIRT
            ) {
                let mut near_mtn = matches!(f, id::MOUNTAIN);
                if !near_mtn {
                    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-2, 0), (2, 0), (0, -2), (0, 2)]
                    {
                        if grid.get(x + dx, y + dy) == Some(id::MOUNTAIN) {
                            near_mtn = true;
                            break;
                        }
                    }
                }
                if near_mtn {
                    foothills.push((x, y));
                }
            }
        }
    }

    let shuffle = |rng: &mut Rng, v: &mut Vec<(i32, i32)>| {
        for i in (1..v.len()).rev() {
            let j = rng.randint0((i + 1) as i32) as usize;
            v.swap(i, j);
        }
    };
    shuffle(rng, &mut floors);
    shuffle(rng, &mut forest);
    shuffle(rng, &mut foothills);

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

    // Trees: clump on forest terrain first, grow small clusters around seeds
    let mut trees = Vec::new();
    let want_trees = cfg.tree_count as usize;
    let mut seeds = Vec::new();
    // pick seed points from forest (or floors if no forest)
    let seed_src = if forest.len() > 8 { &forest } else { &floors };
    let n_seeds = (want_trees / 18).clamp(4, 16).min(seed_src.len().max(1));
    for i in 0..n_seeds {
        if i < seed_src.len() {
            seeds.push(seed_src[i]);
        }
    }
    for &(sx, sy) in &seeds {
        // blob around each seed
        let rad = rng.rand_range(3, 8);
        for yy in (sy - rad)..=(sy + rad) {
            for xx in (sx - rad)..=(sx + rad) {
                if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                    continue;
                }
                if (xx - sx) * (xx - sx) + (yy - sy) * (yy - sy) > rad * rad {
                    continue;
                }
                if !grid.walkable(xx, yy) {
                    continue;
                }
                let f = grid.get(xx, yy).unwrap_or(0);
                if !matches!(
                    f,
                    id::FLOOR | id::DIRT | id::GRASS | id::BRAKE | id::TREE | id::FLOWER
                ) {
                    continue;
                }
                let p = (xx, yy);
                if trees.len() >= want_trees {
                    break;
                }
                if rng.percent(80) && used.insert(p) {
                    trees.push(p);
                }
            }
            if trees.len() >= want_trees {
                break;
            }
        }
        if trees.len() >= want_trees {
            break;
        }
    }
    // fill remainder from forest tiles if short
    for &p in &forest {
        if trees.len() >= want_trees {
            break;
        }
        if used.insert(p) {
            trees.push(p);
        }
    }

    // Iron: clump in foothills / mountain edges
    let mut irons = Vec::new();
    let want_iron = cfg.iron_count as usize;
    let iron_src = if foothills.len() > 8 {
        &foothills
    } else {
        &floors
    };
    let n_seeds = (want_iron / 14).clamp(3, 12).min(iron_src.len().max(1));
    for i in 0..n_seeds {
        if i >= iron_src.len() {
            break;
        }
        let (sx, sy) = iron_src[i];
        let rad = rng.rand_range(2, 5);
        for yy in (sy - rad)..=(sy + rad) {
            for xx in (sx - rad)..=(sx + rad) {
                if xx <= 0 || yy <= 0 || xx >= w - 1 || yy >= h - 1 {
                    continue;
                }
                if (xx - sx).abs() + (yy - sy).abs() > rad {
                    continue;
                }
                let p = (xx, yy);
                if irons.len() >= want_iron {
                    break;
                }
                if rng.percent(75) && used.insert(p) {
                    irons.push(p);
                }
            }
            if irons.len() >= want_iron {
                break;
            }
        }
        if irons.len() >= want_iron {
            break;
        }
    }
    for &p in iron_src {
        if irons.len() >= want_iron {
            break;
        }
        if used.insert(p) {
            irons.push(p);
        }
    }

    (agent, trees, irons)
}
