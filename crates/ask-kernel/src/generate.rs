//! Level generation ported in structure from FrogComposband/Angband.
//!
//! Source of algorithm (not code copy):
//! - `generate.c` :: `cave_gen` — fill walls → rooms → scramble centers → tunnels
//! - `rooms.c` :: `generate_rooms` / `build_type1` — block grid + rectangular rooms
//! - `grid.c` :: `build_tunnel` — wandering corridor with direction changes
//!
//! Content (trees/iron/agent) is ASK-specific; dungeon vaults/traps/monsters are NOT ported.

use crate::config::Config;
use crate::grid::{Grid, Terrain};

/// Frog `BLOCK_HGT` / `BLOCK_WID` (generate.h)
const BLOCK_HGT: i32 = 11;
const BLOCK_WID: i32 = 11;

/// Frog tunnel params (generate.h ranges; we pick mid-band defaults)
const DUN_TUN_RND: i32 = 10; // chance of random direction
const DUN_TUN_CHG: i32 = 40; // chance of changing direction

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub cx: i32,
    pub cy: i32,
}

/// xorshift64* — deterministic like frog RNG seed chain
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

    /// Frog `randint0(n)` → 0..n-1
    pub fn randint0(&mut self, n: i32) -> i32 {
        if n <= 1 {
            return 0;
        }
        (self.next_u64() % n as u64) as i32
    }

    /// Frog `rand_range(a,b)` inclusive-ish via [a, b)
    pub fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.randint0(hi - lo)
    }

    /// percent chance 0..99
    pub fn percent(&mut self, p: i32) -> bool {
        self.randint0(100) < p
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

struct Cave {
    w: i32,
    h: i32,
    /// 0 = solid wall (extra granite), 1 = floor, 2 = outer wall (room edge), 3 = room floor
    cells: Vec<u8>,
}

impl Cave {
    fn new(w: i32, h: i32) -> Self {
        // Frog: start with walls (place_extra_bold everywhere)
        Self {
            w,
            h,
            cells: vec![0; (w * h) as usize],
        }
    }

    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1
    }

    fn get(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.cells[self.idx(x, y)]
    }

    fn set(&mut self, x: i32, y: i32, v: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let i = self.idx(x, y);
            self.cells[i] = v;
        }
    }

    fn is_floorish(&self, x: i32, y: i32) -> bool {
        matches!(self.get(x, y), 1 | 3)
    }
}

/// Frog `correct_dir` — primary axis toward target
fn correct_dir(y1: i32, x1: i32, y2: i32, x2: i32) -> (i32, i32) {
    let mut dy = (y2 - y1).signum();
    let mut dx = (x2 - x1).signum();
    if dy != 0 && dx != 0 {
        // frog picks one axis when diagonal needed
        if (y2 - y1).abs() > (x2 - x1).abs() {
            dx = 0;
        } else {
            dy = 0;
        }
    }
    (dy, dx)
}

/// Frog `rand_dir` — random 4-way
fn rand_dir(rng: &mut Rng) -> (i32, i32) {
    match rng.randint0(4) {
        0 => (-1, 0),
        1 => (1, 0),
        2 => (0, -1),
        _ => (0, 1),
    }
}

/// Port of `grid.c` :: `build_tunnel` (simplified wall types).
/// Digs from (y1,x1) to (y2,x2) with bends; writes floor (1) into solid rock.
fn build_tunnel(cave: &mut Cave, mut y1: i32, mut x1: i32, y2: i32, x2: i32, rng: &mut Rng) -> bool {
    let mut main_loop = 0;
    let (mut row_dir, mut col_dir) = correct_dir(y1, x1, y2, x2);

    while y1 != y2 || x1 != x2 {
        main_loop += 1;
        if main_loop > 4000 {
            return false;
        }

        // Allow bends (dun_tun_chg)
        if rng.percent(DUN_TUN_CHG) {
            let (rd, cd) = correct_dir(y1, x1, y2, x2);
            row_dir = rd;
            col_dir = cd;
            if rng.percent(DUN_TUN_RND) {
                let (rd, cd) = rand_dir(rng);
                row_dir = rd;
                col_dir = cd;
            }
        }

        let mut ty = y1 + row_dir;
        let mut tx = x1 + col_dir;

        // Stay in bounds
        let mut guard = 0;
        while !cave.in_bounds(tx, ty) && guard < 16 {
            guard += 1;
            let (rd, cd) = correct_dir(y1, x1, y2, x2);
            row_dir = rd;
            col_dir = cd;
            if rng.percent(DUN_TUN_RND) {
                let (rd, cd) = rand_dir(rng);
                row_dir = rd;
                col_dir = cd;
            }
            ty = y1 + row_dir;
            tx = x1 + col_dir;
        }
        if !cave.in_bounds(tx, ty) {
            // force toward target
            let (rd, cd) = correct_dir(y1, x1, y2, x2);
            ty = y1 + rd;
            tx = x1 + cd;
            if !cave.in_bounds(tx, ty) {
                return false;
            }
        }

        let cell = cave.get(tx, ty);
        // Pierce outer walls (2) → become floor and continue
        // Tunnel solid (0) → floor
        // Travel through room floor (3) / corridor (1)
        if cell == 2 {
            // outer wall: convert to floor (doorway)
            cave.set(tx, ty, 1);
            // solidify adjacent outer walls (frog solidify neighbors)
            for oy in -1..=1 {
                for ox in -1..=1 {
                    if ox == 0 && oy == 0 {
                        continue;
                    }
                    let nx = tx + ox;
                    let ny = ty + oy;
                    if cave.get(nx, ny) == 2 {
                        cave.set(nx, ny, 0); // solid — no re-pierce mess
                    }
                }
            }
        } else if cell == 0 {
            cave.set(tx, ty, 1);
        }
        // 1 or 3: just travel

        y1 = ty;
        x1 = tx;
    }
    true
}

/// Frog `build_type1` simplified — rectangular room on block grid.
/// `by`,`bx` are block coordinates.
fn try_build_room(
    cave: &mut Cave,
    room_map: &mut [Vec<bool>],
    by: i32,
    bx: i32,
    rng: &mut Rng,
) -> Option<Room> {
    let row_rooms = room_map.len() as i32;
    let col_rooms = room_map[0].len() as i32;

    // Room size in blocks (usually 1x1, sometimes 2x1 / 1x2 like larger type1)
    let mut blocks_high = 1;
    let mut blocks_wide = 1;
    if rng.percent(20) && by + 1 < row_rooms {
        blocks_high = 2;
    }
    if rng.percent(20) && bx + 1 < col_rooms {
        blocks_wide = 2;
    }

    // Check room_map free
    for y in by..by + blocks_high {
        for x in bx..bx + blocks_wide {
            if y >= row_rooms || x >= col_rooms || room_map[y as usize][x as usize] {
                return None;
            }
        }
    }

    // Mark blocks used
    for y in by..by + blocks_high {
        for x in bx..bx + blocks_wide {
            room_map[y as usize][x as usize] = true;
        }
    }

    // Pixel bounds of reserved blocks
    let y0 = by * BLOCK_HGT;
    let x0 = bx * BLOCK_WID;
    let y1b = (by + blocks_high) * BLOCK_HGT - 1;
    let x1b = (bx + blocks_wide) * BLOCK_WID - 1;

    // Inner room with margin (frog leaves outer granite)
    let height = rng.rand_range(4, (y1b - y0).min(9).max(5));
    let width = rng.rand_range(4, (x1b - x0).min(9).max(5));
    let y1 = y0 + rng.rand_range(1, (y1b - y0 - height).max(2));
    let x1 = x0 + rng.rand_range(1, (x1b - x0 - width).max(2));
    let y2 = y1 + height - 1;
    let x2 = x1 + width - 1;

    if y2 >= cave.h - 1 || x2 >= cave.w - 1 || y1 <= 0 || x1 <= 0 {
        return None;
    }

    // Draw room: outer walls = 2, interior = 3 (CAVE_ROOM floor)
    for y in y1..=y2 {
        for x in x1..=x2 {
            if y == y1 || y == y2 || x == x1 || x == x2 {
                cave.set(x, y, 2); // outer
            } else {
                cave.set(x, y, 3); // room floor
            }
        }
    }

    let cx = (x1 + x2) / 2;
    let cy = (y1 + y2) / 2;
    Some(Room {
        x1,
        y1,
        x2,
        y2,
        cx,
        cy,
    })
}

/// Frog `generate_rooms` — place as many rooms as area allows on block grid.
fn generate_rooms(cave: &mut Cave, rng: &mut Rng) -> Vec<Room> {
    let row_rooms = cave.h / BLOCK_HGT;
    let col_rooms = cave.w / BLOCK_WID;
    let mut room_map = vec![vec![false; col_rooms as usize]; row_rooms as usize];

    // Frog: dun_rooms = rand_range(10,25) * area_size / 100
    let area_size = 100 * (cave.h * cave.w) / (66 * 198); // relative to frog MAX
    let mut dun_rooms = rng.rand_range(10, 26) * area_size.max(1) / 100;
    if dun_rooms < 8 {
        dun_rooms = 8;
    }
    // scale up for huge maps
    let block_count = row_rooms * col_rooms;
    dun_rooms = dun_rooms.max(block_count / 4).min(block_count - 4);

    let mut rooms = Vec::new();
    let mut fails = 0;
    while (rooms.len() as i32) < dun_rooms && fails < dun_rooms * 20 {
        let by = rng.randint0(row_rooms);
        let bx = rng.randint0(col_rooms);
        if let Some(room) = try_build_room(cave, &mut room_map, by, bx, rng) {
            rooms.push(room);
            fails = 0;
        } else {
            fails += 1;
        }
    }

    // guarantee at least one room
    if rooms.is_empty() {
        if let Some(room) = try_build_room(cave, &mut room_map, 1, 1, rng) {
            rooms.push(room);
        }
    }

    rooms
}

/// Full `cave_gen`-shaped pipeline for ASK.
pub fn generate_level(cfg: &Config) -> GeneratedLevel {
    // Snap size to block multiples (frog levels are multiples of BLOCK_*)
    let mut w = cfg.width.max(BLOCK_WID * 4);
    let mut h = cfg.height.max(BLOCK_HGT * 4);
    w = (w / BLOCK_WID) * BLOCK_WID;
    h = (h / BLOCK_HGT) * BLOCK_HGT;
    if w < BLOCK_WID * 4 {
        w = BLOCK_WID * 4;
    }
    if h < BLOCK_HGT * 4 {
        h = BLOCK_HGT * 4;
    }

    let mut rng = Rng::new(cfg.seed);
    let mut cave = Cave::new(w, h);

    // 1) already filled with solid walls

    // 2) rooms
    let mut rooms = generate_rooms(&mut cave, &mut rng);

    // 3) scramble room order (frog scramble cent[])
    for i in 0..rooms.len() {
        let pick = rng.randint0((i + 1) as i32) as usize;
        rooms.swap(i, pick);
    }

    // 4) tunnels between consecutive centers (frog loop)
    if rooms.len() >= 2 {
        let mut y = rooms[rooms.len() - 1].cy;
        let mut x = rooms[rooms.len() - 1].cx;
        for room in &rooms {
            let _ = build_tunnel(&mut cave, y, x, room.cy, room.cx, &mut rng);
            y = room.cy;
            x = room.cx;
        }
        // frog sometimes extra connections
        if rooms.len() > 3 && rng.percent(50) {
            let a = rng.randint0(rooms.len() as i32) as usize;
            let b = rng.randint0(rooms.len() as i32) as usize;
            if a != b {
                let _ = build_tunnel(
                    &mut cave,
                    rooms[a].cy,
                    rooms[a].cx,
                    rooms[b].cy,
                    rooms[b].cx,
                    &mut rng,
                );
            }
        }
    }

    // 5) convert outer walls left as walls (2→0), room/corridor floors → Floor
    let mut cells = vec![Terrain::Wall; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let v = cave.get(x, y);
            let t = match v {
                1 | 3 => Terrain::Floor,
                _ => Terrain::Wall,
            };
            cells[(y * w + x) as usize] = t;
        }
    }

    // permanent boundary (frog special boundary walls)
    for x in 0..w {
        cells[x as usize] = Terrain::Wall;
        cells[((h - 1) * w + x) as usize] = Terrain::Wall;
    }
    for y in 0..h {
        cells[(y * w) as usize] = Terrain::Wall;
        cells[(y * w + w - 1) as usize] = Terrain::Wall;
    }

    let grid = Grid {
        width: w,
        height: h,
        cells,
    };

    // 6) alloc objects — frog alloc_object; we place trees/iron on floors
    let mut floors: Vec<(i32, i32)> = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if grid.walkable(x, y) {
                floors.push((x, y));
            }
        }
    }
    // shuffle
    for i in (1..floors.len()).rev() {
        let j = rng.randint0((i + 1) as i32) as usize;
        floors.swap(i, j);
    }

    let agent = if !rooms.is_empty() {
        (rooms[0].cx, rooms[0].cy)
    } else if !floors.is_empty() {
        floors[0]
    } else {
        (w / 2, h / 2)
    };

    // ensure agent cell is floor
    // (room center should be)

    let mut used = std::collections::HashSet::new();
    used.insert(agent);

    let mut trees = Vec::new();
    let mut irons = Vec::new();
    let mut fi = 0usize;
    while trees.len() < cfg.tree_count as usize && fi < floors.len() {
        let p = floors[fi];
        fi += 1;
        if used.contains(&p) {
            continue;
        }
        used.insert(p);
        trees.push(p);
    }
    while irons.len() < cfg.iron_count as usize && fi < floors.len() {
        let p = floors[fi];
        fi += 1;
        if used.contains(&p) {
            continue;
        }
        used.insert(p);
        irons.push(p);
    }

    GeneratedLevel {
        grid,
        rooms,
        agent,
        trees,
        irons,
    }
}
