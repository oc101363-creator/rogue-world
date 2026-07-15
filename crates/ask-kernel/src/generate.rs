//! Procedural map generation — Frog/Angband *ideas* (rooms + corridors), original code.
//!
//! Frog `generate.c`: fill solid rock → dig rooms → pierce tunnels → place features.
//! We keep the same structure without dungeon content (no vaults/traps/classes).

use crate::config::Config;
use crate::grid::{Grid, Terrain};

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Room {
    pub fn center(self) -> (i32, i32) {
        ((self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2)
    }

    pub fn intersects(self, other: Room, pad: i32) -> bool {
        !(self.x2 + pad < other.x1
            || self.x1 > other.x2 + pad
            || self.y2 + pad < other.y1
            || self.y1 > other.y2 + pad)
    }
}

/// Simple deterministic PRNG (xorshift64*) — seedable like frog RNG.
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

    pub fn gen_range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as i32
    }

    pub fn chance(&mut self, percent: u32) -> bool {
        (self.next_u64() % 100) < percent as u64
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

/// Generate a level:
/// 1. Fill walls (frog solid granite)
/// 2. Place non-overlapping rooms
/// 3. Connect with L-shaped corridors
/// 4. Scatter resources on floors; agent in first room center
pub fn generate_level(cfg: &Config) -> GeneratedLevel {
    let w = cfg.width.max(20);
    let h = cfg.height.max(16);
    let mut rng = Rng::new(cfg.seed);
    let mut cells = vec![Terrain::Wall; (w * h) as usize];

    let set = |cells: &mut [Terrain], x: i32, y: i32, t: Terrain| {
        if x >= 0 && y >= 0 && x < w && y < h {
            cells[(y * w + x) as usize] = t;
        }
    };

    // --- rooms ---
    let target_rooms = cfg.room_count.max(3);
    let mut rooms: Vec<Room> = Vec::new();
    let mut attempts = 0;
    while rooms.len() < target_rooms as usize && attempts < target_rooms as usize * 40 {
        attempts += 1;
        let rw = rng.gen_range(cfg.room_min_size, cfg.room_max_size + 1);
        let rh = rng.gen_range(cfg.room_min_size, cfg.room_max_size + 1);
        let x1 = rng.gen_range(1, (w - rw - 1).max(2));
        let y1 = rng.gen_range(1, (h - rh - 1).max(2));
        let room = Room {
            x1,
            y1,
            x2: x1 + rw - 1,
            y2: y1 + rh - 1,
        };
        if room.x2 >= w - 1 || room.y2 >= h - 1 {
            continue;
        }
        if rooms.iter().any(|r| r.intersects(room, 1)) {
            continue;
        }
        // dig room interior
        for y in room.y1..=room.y2 {
            for x in room.x1..=room.x2 {
                set(&mut cells, x, y, Terrain::Floor);
            }
        }
        rooms.push(room);
    }

    // fallback: at least one room
    if rooms.is_empty() {
        let room = Room {
            x1: 2,
            y1: 2,
            x2: w / 3,
            y2: h / 3,
        };
        for y in room.y1..=room.y2 {
            for x in room.x1..=room.x2 {
                set(&mut cells, x, y, Terrain::Floor);
            }
        }
        rooms.push(room);
    }

    // --- corridors (connect room i to i+1, frog-style tunnel between centers) ---
    for i in 1..rooms.len() {
        let (x1, y1) = rooms[i - 1].center();
        let (x2, y2) = rooms[i].center();
        dig_corridor(&mut cells, w, h, x1, y1, x2, y2, &mut rng);
    }
    // extra loops for connectivity feel
    if rooms.len() > 2 {
        let (x1, y1) = rooms[0].center();
        let (x2, y2) = rooms[rooms.len() - 1].center();
        dig_corridor(&mut cells, w, h, x1, y1, x2, y2, &mut rng);
    }

    let grid = Grid {
        width: w,
        height: h,
        cells,
    };

    // floor cells for placement
    let mut floors: Vec<(i32, i32)> = Vec::new();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if grid.walkable(x, y) {
                floors.push((x, y));
            }
        }
    }
    // shuffle floors
    for i in (1..floors.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        floors.swap(i, j);
    }

    let agent = rooms[0].center();

    let mut trees = Vec::new();
    let mut irons = Vec::new();
    let mut used = std::collections::HashSet::new();
    used.insert(agent);

    let mut fi = 0;
    while trees.len() < cfg.tree_count as usize && fi < floors.len() {
        let p = floors[fi];
        fi += 1;
        if used.contains(&p) {
            continue;
        }
        // prefer not blocking room centers too hard — still ok
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

fn dig_corridor(
    cells: &mut [Terrain],
    w: i32,
    h: i32,
    mut x1: i32,
    mut y1: i32,
    x2: i32,
    y2: i32,
    rng: &mut Rng,
) {
    let set = |cells: &mut [Terrain], x: i32, y: i32| {
        if x > 0 && y > 0 && x < w - 1 && y < h - 1 {
            cells[(y * w + x) as usize] = Terrain::Floor;
        }
    };

    // random L order (frog tunnels bend)
    if rng.chance(50) {
        while x1 != x2 {
            set(cells, x1, y1);
            x1 += (x2 - x1).signum();
        }
        while y1 != y2 {
            set(cells, x1, y1);
            y1 += (y2 - y1).signum();
        }
    } else {
        while y1 != y2 {
            set(cells, x1, y1);
            y1 += (y2 - y1).signum();
        }
        while x1 != x2 {
            set(cells, x1, y1);
            x1 += (x2 - x1).signum();
        }
    }
    set(cells, x2, y2);
}
