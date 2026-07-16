//! Frog `rooms.c` room builders used by generate_rooms.

use super::{Cave, Cell, Rng};

/// Frog BLOCK_HGT / BLOCK_WID
pub const BLOCK_HGT: i32 = 11;
pub const BLOCK_WID: i32 = 11;

#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub cx: i32,
    pub cy: i32,
    pub kind: RoomKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Normal,  // type1
    Overlap, // type2
    Cavern,  // type9-ish organic
    Vault,   // lesser vault spirit: thick rim + rubble interior
    Trap,    // type14 trap room
    Crypt,   // type12 crypt-ish pillars + rubble
}

pub struct DunRooms {
    pub rooms: Vec<Room>,
    /// Room-cell map kept for future feature passes (maze vaults use rooms only).
    #[allow(dead_code)]
    pub room_map: Vec<Vec<bool>>,
}

/// Frog generate_rooms: allocate room-type budget, build until full.
pub fn generate_rooms(cave: &mut Cave, rng: &mut Rng) -> DunRooms {
    let row_rooms = cave.h / BLOCK_HGT;
    let col_rooms = cave.w / BLOCK_WID;
    let mut room_map = vec![vec![false; col_rooms as usize]; row_rooms as usize];

    // Open map: pack most blocks with rooms so rock is ~20% not ~70%.
    // Frog default is denser rock; ASK open-world target is more floor.
    let blocks = row_rooms * col_rooms;
    // Aim to attempt rooms on ~85% of blocks (minus edge friction).
    let dun_rooms = (blocks * 85 / 100).max(12).min(blocks - 2);

    // Weights inspired by frog room_build_order / room_info_normal
    // Normal 35, Overlap 20, Cavern 20, Vault 8, Trap 10, Crypt 7
    let mut want_normal = 0;
    let mut want_overlap = 0;
    let mut want_cavern = 0;
    let mut want_vault = 0;
    let mut want_trap = 0;
    let mut want_crypt = 0;
    for _ in 0..dun_rooms {
        let r = rng.randint0(100);
        if r < 35 {
            want_normal += 1;
        } else if r < 55 {
            want_overlap += 1;
        } else if r < 75 {
            want_cavern += 1;
        } else if r < 83 {
            want_vault += 1;
        } else if r < 93 {
            want_trap += 1;
        } else {
            want_crypt += 1;
        }
    }

    let mut rooms = Vec::new();
    // frog room_build_order: rarer / larger first
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_vault,
        RoomKind::Vault,
    );
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_crypt,
        RoomKind::Crypt,
    );
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_trap,
        RoomKind::Trap,
    );
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_cavern,
        RoomKind::Cavern,
    );
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_overlap,
        RoomKind::Overlap,
    );
    build_many(
        cave,
        &mut room_map,
        &mut rooms,
        rng,
        want_normal,
        RoomKind::Normal,
    );

    if rooms.is_empty() {
        if let Some(r) = build_type1(cave, &mut room_map, rng) {
            rooms.push(r);
        }
    }

    DunRooms { rooms, room_map }
}

fn build_many(
    cave: &mut Cave,
    room_map: &mut [Vec<bool>],
    rooms: &mut Vec<Room>,
    rng: &mut Rng,
    count: i32,
    kind: RoomKind,
) {
    let mut left = count;
    let mut fails = 0;
    while left > 0 && fails < count * 25 {
        let ok = match kind {
            RoomKind::Normal => build_type1(cave, room_map, rng),
            RoomKind::Overlap => build_type2(cave, room_map, rng),
            RoomKind::Cavern => build_type_cavern(cave, room_map, rng),
            RoomKind::Vault => build_type_vault(cave, room_map, rng),
            RoomKind::Trap => build_type_trap(cave, room_map, rng),
            RoomKind::Crypt => build_type_crypt(cave, room_map, rng),
        };
        if let Some(r) = ok {
            rooms.push(r);
            left -= 1;
            fails = 0;
        } else {
            fails += 1;
        }
    }
}

fn reserve_blocks(room_map: &mut [Vec<bool>], by: i32, bx: i32, bh: i32, bw: i32) -> bool {
    let row_rooms = room_map.len() as i32;
    let col_rooms = room_map[0].len() as i32;
    for y in by..by + bh {
        for x in bx..bx + bw {
            if y >= row_rooms || x >= col_rooms || room_map[y as usize][x as usize] {
                return false;
            }
        }
    }
    for y in by..by + bh {
        for x in bx..bx + bw {
            room_map[y as usize][x as usize] = true;
        }
    }
    true
}

fn pick_block(room_map: &[Vec<bool>], rng: &mut Rng) -> (i32, i32) {
    let row_rooms = room_map.len() as i32;
    let col_rooms = room_map[0].len() as i32;
    (rng.randint0(row_rooms), rng.randint0(col_rooms))
}

/// Frog build_type1 — rectangular room + pillar / four-pillar / ragged variants.
fn build_type1(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let (by, bx) = pick_block(room_map, rng);
    let mut bh = 1;
    let mut bw = 1;
    if rng.percent(15) {
        bh = 2;
    }
    if rng.percent(15) {
        bw = 2;
    }
    if !reserve_blocks(room_map, by, bx, bh, bw) {
        return None;
    }

    // Larger rooms to raise floor ratio (still fit block reserve)
    let y1off = 2 + rng.randint1(5);
    let x1off = 3 + rng.randint1(9);
    let y2off = 2 + rng.randint1(4);
    let x2off = 3 + rng.randint1(9);
    let mut ysize = y1off + y2off + 1;
    let mut xsize = x1off + x2off + 1;
    ysize = ysize.min(bh * BLOCK_HGT - 2).max(5);
    xsize = xsize.min(bw * BLOCK_WID - 2).max(6);

    let block_y0 = by * BLOCK_HGT;
    let block_x0 = bx * BLOCK_WID;
    let block_y1 = (by + bh) * BLOCK_HGT - 1;
    let block_x1 = (bx + bw) * BLOCK_WID - 1;

    let yval = (block_y0 + block_y1) / 2;
    let xval = (block_x0 + block_x1) / 2;
    let y1 = yval - ysize / 2;
    let x1 = xval - xsize / 2;
    let y2 = yval + (ysize - 1) / 2;
    let x2 = xval + (xsize - 1) / 2;

    if y1 <= 1 || x1 <= 1 || y2 >= cave.h - 2 || x2 >= cave.w - 2 {
        return None;
    }

    // Floor interior; outer ring is irregular (not a perfect rectangle wireframe)
    for y in y1..=y2 {
        for x in x1..=x2 {
            cave.set(x, y, Cell::Room);
        }
    }
    // Ragged perimeter: 1–2 cell thick rock with random gaps/juts
    for y in (y1 - 1)..=(y2 + 1) {
        for x in (x1 - 1)..=(x2 + 1) {
            let on_edge = y == y1 - 1 || y == y2 + 1 || x == x1 - 1 || x == x2 + 1;
            let near_edge = y <= y1 || y >= y2 || x <= x1 || x >= x2;
            if !on_edge && !near_edge {
                continue;
            }
            if !cave.in_bounds(x, y) {
                continue;
            }
            if on_edge {
                // sometimes skip → natural opening; sometimes double-thick rock
                if rng.percent(18) {
                    cave.set(x, y, Cell::Room);
                } else if rng.percent(40) {
                    cave.set(x, y, Cell::Solid);
                } else {
                    cave.set(x, y, Cell::Outer);
                }
            } else if near_edge && rng.percent(22) {
                // juts of rock into room / soft corners
                cave.set(x, y, Cell::Solid);
            }
        }
    }

    // occasional pillar room (1/20)
    if rng.one_in(20) {
        let mut y = y1;
        while y <= y2 {
            let mut x = x1;
            while x <= x2 {
                cave.set(x, y, Cell::Inner);
                x += 2;
            }
            y += 2;
        }
    } else if rng.one_in(20) && y1 + 4 < y2 && x1 + 4 < x2 {
        // four pillars
        cave.set(x1 + 1, y1 + 1, Cell::Inner);
        cave.set(x2 - 1, y1 + 1, Cell::Inner);
        cave.set(x1 + 1, y2 - 1, Cell::Inner);
        cave.set(x2 - 1, y2 - 1, Cell::Inner);
    } else if rng.one_in(50) {
        // ragged edge
        let mut y = y1 + 2;
        while y <= y2 - 2 {
            cave.set(x1, y, Cell::Inner);
            cave.set(x2, y, Cell::Inner);
            y += 2;
        }
        let mut x = x1 + 2;
        while x <= x2 - 2 {
            cave.set(x, y1, Cell::Inner);
            cave.set(x, y2, Cell::Inner);
            x += 2;
        }
    }

    Some(Room {
        x1: x1 - 1,
        y1: y1 - 1,
        x2: x2 + 1,
        y2: y2 + 1,
        cx: xval,
        cy: yval,
        kind: RoomKind::Normal,
    })
}

/// Frog build_type2 — two overlapping rectangles.
fn build_type2(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let (by, bx) = pick_block(room_map, rng);
    if !reserve_blocks(room_map, by, bx, 2, 2) {
        // need 2x2 blocks; try 1x1 fallback area
        if !reserve_blocks(room_map, by, bx, 1, 1) {
            return None;
        }
    }

    let yval = by * BLOCK_HGT + BLOCK_HGT;
    let xval = bx * BLOCK_WID + BLOCK_WID;
    if yval >= cave.h - 6 || xval >= cave.w - 6 {
        return None;
    }

    let y1a = yval - rng.rand_range(4, 8);
    let y2a = yval + rng.rand_range(3, 7);
    let x1a = xval - rng.rand_range(6, 12);
    let x2a = xval + rng.rand_range(5, 11);

    let y1b = yval - rng.rand_range(3, 7);
    let y2b = yval + rng.rand_range(4, 8);
    let x1b = xval - rng.rand_range(5, 11);
    let x2b = xval + rng.rand_range(6, 12);

    let y1 = y1a.min(y1b).max(2);
    let x1 = x1a.min(x1b).max(2);
    let y2 = y2a.max(y2b).min(cave.h - 3);
    let x2 = x2a.max(x2b).min(cave.w - 3);
    if y2 - y1 < 5 || x2 - x1 < 6 {
        return None;
    }

    // draw rect A
    fill_rect_room(
        cave,
        x1a.max(2),
        y1a.max(2),
        x2a.min(cave.w - 3),
        y2a.min(cave.h - 3),
    );
    // draw rect B
    fill_rect_room(
        cave,
        x1b.max(2),
        y1b.max(2),
        x2b.min(cave.w - 3),
        y2b.min(cave.h - 3),
    );

    Some(Room {
        x1,
        y1,
        x2,
        y2,
        cx: xval.clamp(x1 + 1, x2 - 1),
        cy: yval.clamp(y1 + 1, y2 - 1),
        kind: RoomKind::Overlap,
    })
}

fn fill_rect_room(cave: &mut Cave, x1: i32, y1: i32, x2: i32, y2: i32) {
    if x2 <= x1 + 2 || y2 <= y1 + 2 {
        return;
    }
    for y in y1..=y2 {
        for x in x1..=x2 {
            if y == y1 || y == y2 || x == x1 || x == x2 {
                if cave.get(x, y) != Cell::Room {
                    cave.set(x, y, Cell::Outer);
                }
            } else {
                cave.set(x, y, Cell::Room);
            }
        }
    }
}

/// Simplified organic cavern (frog type9 spirit: irregular blob).
fn build_type_cavern(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let (by, bx) = pick_block(room_map, rng);
    if !reserve_blocks(room_map, by, bx, 2, 2) {
        if !reserve_blocks(room_map, by, bx, 1, 1) {
            return None;
        }
    }
    let cy = by * BLOCK_HGT + BLOCK_HGT / 2 + rng.randint0(3);
    let cx = bx * BLOCK_WID + BLOCK_WID / 2 + rng.randint0(3);
    if !cave.in_bounds(cx, cy) {
        return None;
    }
    let rad = rng.rand_range(4, 9);
    let mut minx = cx;
    let mut maxx = cx;
    let mut miny = cy;
    let mut maxy = cy;

    for yy in (cy - rad)..=(cy + rad) {
        for xx in (cx - rad)..=(cx + rad) {
            if !cave.in_bounds(xx, yy) {
                continue;
            }
            // ellipse with noise
            let nx = (xx - cx) as f32 / rad as f32;
            let ny = (yy - cy) as f32 / (rad as f32 * 0.8);
            let d = nx * nx + ny * ny;
            if d > 1.0 + (rng.randint0(30) as f32 - 15.0) * 0.01 {
                continue;
            }
            cave.set(xx, yy, Cell::Room);
            minx = minx.min(xx);
            maxx = maxx.max(xx);
            miny = miny.min(yy);
            maxy = maxy.max(yy);
        }
    }
    // rough outer: any room adjacent to solid becomes outer
    for yy in miny..=maxy {
        for xx in minx..=maxx {
            if cave.get(xx, yy) != Cell::Room {
                continue;
            }
            let mut edge = false;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if cave.get(xx + dx, yy + dy) == Cell::Solid {
                    edge = true;
                    break;
                }
            }
            if edge && rng.percent(40) {
                cave.set(xx, yy, Cell::Outer);
            }
        }
    }

    Some(Room {
        x1: minx,
        y1: miny,
        x2: maxx,
        y2: maxy,
        cx,
        cy,
        kind: RoomKind::Cavern,
    })
}

/// Lesser-vault spirit: thick outer, floor core, rubble scatter (not full v_info templates).
fn build_type_vault(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let (by, bx) = pick_block(room_map, rng);
    if !reserve_blocks(room_map, by, bx, 2, 2) {
        return None;
    }
    let y1 = by * BLOCK_HGT + 1;
    let x1 = bx * BLOCK_WID + 1;
    let y2 = y1 + BLOCK_HGT + rng.rand_range(2, 6);
    let x2 = x1 + BLOCK_WID + rng.rand_range(2, 8);
    let y2 = y2.min(cave.h - 3);
    let x2 = x2.min(cave.w - 3);
    if y2 - y1 < 6 || x2 - x1 < 6 {
        return None;
    }
    for y in y1..=y2 {
        for x in x1..=x2 {
            let edge = y == y1 || y == y2 || x == x1 || x == x2;
            let inner_edge = y == y1 + 1 || y == y2 - 1 || x == x1 + 1 || x == x2 - 1;
            if edge {
                cave.set(x, y, Cell::Outer);
            } else if inner_edge && rng.percent(70) {
                cave.set(x, y, Cell::Inner);
            } else if rng.percent(12) {
                cave.set(x, y, Cell::Inner); // rubble-like pillars
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
        kind: RoomKind::Vault,
    })
}

/// Frog build_type14 spirit — normal room marked Trap (feats filled later).
fn build_type_trap(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let mut r = build_type1(cave, room_map, rng)?;
    r.kind = RoomKind::Trap;
    Some(r)
}

/// Frog crypt spirit — pillars + inner walls like a burial chamber.
fn build_type_crypt(cave: &mut Cave, room_map: &mut [Vec<bool>], rng: &mut Rng) -> Option<Room> {
    let mut r = build_type1(cave, room_map, rng)?;
    r.kind = RoomKind::Crypt;
    // dense pillar grid
    let mut y = r.y1 + 2;
    while y <= r.y2 - 2 {
        let mut x = r.x1 + 2;
        while x <= r.x2 - 2 {
            if cave.get(x, y) == Cell::Room && rng.percent(70) {
                cave.set(x, y, Cell::Inner);
            }
            x += 2;
        }
        y += 2;
    }
    Some(r)
}
