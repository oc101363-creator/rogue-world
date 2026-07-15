//! Level generation — frog `cave_gen` structure + f_info terrain kinds.
//!
//! Pipeline (generate.c / rooms.c / grid.c):
//! fill granite → block rooms with outer walls → scramble centers →
//! wandering tunnels → permanent border → veins/water/lava/doors/stairs → objects.

use crate::config::Config;
use crate::feat::Feat;
use crate::grid::Grid;

const BLOCK_HGT: i32 = 11;
const BLOCK_WID: i32 = 11;
const DUN_TUN_RND: i32 = 10;
const DUN_TUN_CHG: i32 = 40;

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub cx: i32,
    pub cy: i32,
}

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

    fn next_u64(&mut self) -> u64 {
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

    pub fn rand_range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.randint0(hi - lo)
    }

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

/// Generation-time cell tags (before baking to Feat).
#[derive(Clone, Copy, PartialEq)]
enum Cell {
    Solid,  // diggable granite fill
    Outer,  // room outer wall
    Room,   // room floor
    Tunnel, // corridor floor
}

struct Cave {
    w: i32,
    h: i32,
    cells: Vec<Cell>,
}

impl Cave {
    fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            cells: vec![Cell::Solid; (w * h) as usize],
        }
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x > 0 && y > 0 && x < self.w - 1 && y < self.h - 1
    }

    fn get(&self, x: i32, y: i32) -> Cell {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Cell::Solid;
        }
        self.cells[(y * self.w + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, v: Cell) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.cells[(y * self.w + x) as usize] = v;
        }
    }
}

fn correct_dir(y1: i32, x1: i32, y2: i32, x2: i32) -> (i32, i32) {
    let mut dy = (y2 - y1).signum();
    let mut dx = (x2 - x1).signum();
    if dy != 0 && dx != 0 {
        if (y2 - y1).abs() > (x2 - x1).abs() {
            dx = 0;
        } else {
            dy = 0;
        }
    }
    (dy, dx)
}

fn rand_dir(rng: &mut Rng) -> (i32, i32) {
    match rng.randint0(4) {
        0 => (-1, 0),
        1 => (1, 0),
        2 => (0, -1),
        _ => (0, 1),
    }
}

/// `grid.c` build_tunnel (structure).
fn build_tunnel(cave: &mut Cave, mut y1: i32, mut x1: i32, y2: i32, x2: i32, rng: &mut Rng) -> bool {
    let mut n = 0;
    let (mut row_dir, mut col_dir) = correct_dir(y1, x1, y2, x2);
    while y1 != y2 || x1 != x2 {
        n += 1;
        if n > 4000 {
            return false;
        }
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
        let mut g = 0;
        while !cave.in_bounds(tx, ty) && g < 16 {
            g += 1;
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
            let (rd, cd) = correct_dir(y1, x1, y2, x2);
            ty = y1 + rd;
            tx = x1 + cd;
            if !cave.in_bounds(tx, ty) {
                return false;
            }
        }
        match cave.get(tx, ty) {
            Cell::Outer => {
                cave.set(tx, ty, Cell::Tunnel);
                for oy in -1..=1 {
                    for ox in -1..=1 {
                        if ox == 0 && oy == 0 {
                            continue;
                        }
                        if cave.get(tx + ox, ty + oy) == Cell::Outer {
                            cave.set(tx + ox, ty + oy, Cell::Solid);
                        }
                    }
                }
            }
            Cell::Solid => cave.set(tx, ty, Cell::Tunnel),
            Cell::Room | Cell::Tunnel => {}
        }
        y1 = ty;
        x1 = tx;
    }
    true
}

fn try_build_room(
    cave: &mut Cave,
    room_map: &mut [Vec<bool>],
    by: i32,
    bx: i32,
    rng: &mut Rng,
) -> Option<Room> {
    let row_rooms = room_map.len() as i32;
    let col_rooms = room_map[0].len() as i32;
    let mut bh = 1;
    let mut bw = 1;
    if rng.percent(20) && by + 1 < row_rooms {
        bh = 2;
    }
    if rng.percent(20) && bx + 1 < col_rooms {
        bw = 2;
    }
    for y in by..by + bh {
        for x in bx..bx + bw {
            if y >= row_rooms || x >= col_rooms || room_map[y as usize][x as usize] {
                return None;
            }
        }
    }
    for y in by..by + bh {
        for x in bx..bx + bw {
            room_map[y as usize][x as usize] = true;
        }
    }
    let y0 = by * BLOCK_HGT;
    let x0 = bx * BLOCK_WID;
    let y1b = (by + bh) * BLOCK_HGT - 1;
    let x1b = (bx + bw) * BLOCK_WID - 1;
    let height = rng.rand_range(4, (y1b - y0).min(9).max(5));
    let width = rng.rand_range(4, (x1b - x0).min(9).max(5));
    let y1 = y0 + rng.rand_range(1, (y1b - y0 - height).max(2));
    let x1 = x0 + rng.rand_range(1, (x1b - x0 - width).max(2));
    let y2 = y1 + height - 1;
    let x2 = x1 + width - 1;
    if y2 >= cave.h - 1 || x2 >= cave.w - 1 || y1 <= 0 || x1 <= 0 {
        return None;
    }
    for y in y1..=y2 {
        for x in x1..=x2 {
            if y == y1 || y == y2 || x == x1 || x == x2 {
                cave.set(x, y, Cell::Outer);
            } else {
                cave.set(x, y, Cell::Room);
            }
        }
    }
    Some(Room {
        x1,
        y1,
        x2,
        y2,
        cx: (x1 + x2) / 2,
        cy: (y1 + y2) / 2,
    })
}

fn generate_rooms(cave: &mut Cave, rng: &mut Rng) -> Vec<Room> {
    let row_rooms = cave.h / BLOCK_HGT;
    let col_rooms = cave.w / BLOCK_WID;
    let mut room_map = vec![vec![false; col_rooms as usize]; row_rooms as usize];
    let area_size = 100 * (cave.h * cave.w) / (66 * 198);
    let mut dun_rooms = rng.rand_range(10, 26) * area_size.max(1) / 100;
    if dun_rooms < 8 {
        dun_rooms = 8;
    }
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
    if rooms.is_empty() {
        if let Some(room) = try_build_room(cave, &mut room_map, 1, 1, rng) {
            rooms.push(room);
        }
    }
    rooms
}

/// Bake Cell → Feat and sprinkle frog-style variety.
fn bake_feats(cave: &Cave, rooms: &[Room], rng: &mut Rng) -> Vec<Feat> {
    let w = cave.w;
    let h = cave.h;
    let mut feats = vec![Feat::Granite; (w * h) as usize];

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            feats[i] = match cave.get(x, y) {
                Cell::Room => {
                    // room floors: mostly FLOOR, some dirt/grass
                    let r = rng.randint0(100);
                    if r < 8 {
                        Feat::Dirt
                    } else if r < 14 {
                        Feat::Grass
                    } else {
                        Feat::Floor
                    }
                }
                Cell::Tunnel => Feat::Floor,
                Cell::Outer => Feat::GraniteOuter,
                Cell::Solid => {
                    // solid rock variety: granite / magma / quartz / rubble (frog veins)
                    let r = rng.randint0(100);
                    if r < 4 {
                        Feat::MagmaVein
                    } else if r < 7 {
                        Feat::QuartzVein
                    } else if r < 8 {
                        Feat::Rubble
                    } else if r < 9 {
                        Feat::MagmaTreasure
                    } else if r < 10 {
                        Feat::QuartzTreasure
                    } else {
                        Feat::Granite
                    }
                }
            };
        }
    }

    // permanent border
    for x in 0..w {
        feats[x as usize] = Feat::Permanent;
        feats[((h - 1) * w + x) as usize] = Feat::Permanent;
    }
    for y in 0..h {
        feats[(y * w) as usize] = Feat::Permanent;
        feats[(y * w + w - 1) as usize] = Feat::Permanent;
    }

    // doors on outer piercings: tunnel adjacent to room
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if feats[(y * w + x) as usize] != Feat::Floor {
                continue;
            }
            // doorway if next to granite outer and room-ish
            let mut adj_outer = 0;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if feats[((y + dy) * w + (x + dx)) as usize] == Feat::GraniteOuter {
                    adj_outer += 1;
                }
            }
            if adj_outer >= 1 && rng.percent(12) {
                feats[(y * w + x) as usize] = if rng.percent(70) {
                    Feat::OpenDoor
                } else {
                    Feat::ClosedDoor
                };
            }
        }
    }

    // stairs in a couple of rooms (frog alloc_stairs simplified)
    if rooms.len() >= 2 {
        let r0 = rooms[0];
        let r1 = rooms[rooms.len() - 1];
        feats[(r0.cy * w + r0.cx) as usize] = Feat::UpStair;
        // offset down stair if same cell
        let mut dx = r1.cx;
        let mut dy = r1.cy;
        if dx == r0.cx && dy == r0.cy {
            dx = (r1.cx + 1).min(r1.x2 - 1);
        }
        feats[(dy * w + dx) as usize] = Feat::DownStair;
    }

    // shallow water pools in some rooms
    let n_pools = (rooms.len() / 5).max(1);
    for _ in 0..n_pools {
        if rooms.is_empty() {
            break;
        }
        let r = rooms[rng.randint0(rooms.len() as i32) as usize];
        let cy = rng.rand_range(r.y1 + 1, r.y2);
        let cx = rng.rand_range(r.x1 + 1, r.x2);
        let rad = rng.rand_range(1, 3);
        for yy in (cy - rad)..=(cy + rad) {
            for xx in (cx - rad)..=(cx + rad) {
                if xx <= r.x1 || xx >= r.x2 || yy <= r.y1 || yy >= r.y2 {
                    continue;
                }
                if (xx - cx).abs() + (yy - cy).abs() <= rad {
                    let deep = (xx - cx).abs() + (yy - cy).abs() == 0 && rad > 1;
                    feats[(yy * w + xx) as usize] = if deep {
                        Feat::DeepWater
                    } else {
                        Feat::ShallowWater
                    };
                }
            }
        }
    }

    // rare lava patch (frog lake-ish)
    if rooms.len() > 4 && rng.percent(40) {
        let r = rooms[rng.randint0(rooms.len() as i32) as usize];
        let cy = (r.y1 + r.y2) / 2;
        let cx = (r.x1 + r.x2) / 2;
        for yy in (cy - 1)..=(cy + 1) {
            for xx in (cx - 1)..=(cx + 1) {
                if xx > r.x1 && xx < r.x2 && yy > r.y1 && yy < r.y2 {
                    feats[(yy * w + xx) as usize] = if xx == cx && yy == cy {
                        Feat::DeepLava
                    } else {
                        Feat::ShallowLava
                    };
                }
            }
        }
    }

    feats
}

pub fn generate_level(cfg: &Config) -> GeneratedLevel {
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
    let mut rooms = generate_rooms(&mut cave, &mut rng);

    for i in 0..rooms.len() {
        let pick = rng.randint0((i + 1) as i32) as usize;
        rooms.swap(i, pick);
    }

    if rooms.len() >= 2 {
        let mut y = rooms[rooms.len() - 1].cy;
        let mut x = rooms[rooms.len() - 1].cx;
        for room in &rooms {
            let _ = build_tunnel(&mut cave, y, x, room.cy, room.cx, &mut rng);
            y = room.cy;
            x = room.cx;
        }
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

    let cells = bake_feats(&cave, &rooms, &mut rng);
    let grid = Grid {
        width: w,
        height: h,
        cells,
    };

    let mut floors: Vec<(i32, i32)> = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if grid.walkable(x, y) {
                // don't spawn on stairs/doors/lava center ideally
                let f = grid.get(x, y).unwrap();
                if matches!(
                    f,
                    Feat::Floor | Feat::Dirt | Feat::Grass | Feat::ShallowWater
                ) {
                    floors.push((x, y));
                }
            }
        }
    }
    for i in (1..floors.len()).rev() {
        let j = rng.randint0((i + 1) as i32) as usize;
        floors.swap(i, j);
    }

    let agent = if !rooms.is_empty() {
        // prefer not on stair
        let (cx, cy) = (rooms[0].cx, rooms[0].cy);
        if grid.get(cx, cy) == Some(Feat::UpStair) {
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

    GeneratedLevel {
        grid,
        rooms,
        agent,
        trees,
        irons,
    }
}
