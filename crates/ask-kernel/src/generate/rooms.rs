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
    Normal,     // type1
    Overlap,    // type2
    Cavern,     // type9-ish organic
}

pub struct DunRooms {
    pub rooms: Vec<Room>,
    pub room_map: Vec<Vec<bool>>,
}

/// Frog generate_rooms: allocate room-type budget, build until full.
pub fn generate_rooms(cave: &mut Cave, rng: &mut Rng) -> DunRooms {
    let row_rooms = cave.h / BLOCK_HGT;
    let col_rooms = cave.w / BLOCK_WID;
    let mut room_map = vec![vec![false; col_rooms as usize]; row_rooms as usize];

    // area scaling like frog
    let area_size = 100 * (cave.h * cave.w) / (66 * 198);
    let mut dun_rooms = rng.rand_range(10, 26) * area_size.max(1) / 100;
    if dun_rooms < 8 {
        dun_rooms = 8;
    }
    let blocks = row_rooms * col_rooms;
    dun_rooms = dun_rooms.max(blocks / 4).min(blocks - 4);

    // probability weights (frog room_info_normal simplified)
    // Normal 60, Overlap 25, Cavern 15
    let mut want_normal = 0;
    let mut want_overlap = 0;
    let mut want_cavern = 0;
    for _ in 0..dun_rooms {
        let r = rng.randint0(100);
        if r < 60 {
            want_normal += 1;
        } else if r < 85 {
            want_overlap += 1;
        } else {
            want_cavern += 1;
        }
    }

    let mut rooms = Vec::new();
    // build larger/rarer first (frog room_build_order)
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

fn reserve_blocks(
    room_map: &mut [Vec<bool>],
    by: i32,
    bx: i32,
    bh: i32,
    bw: i32,
) -> bool {
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

    // frog size pick
    let y1off = 1 + rng.randint1(4);
    let x1off = 2 + rng.randint1(8);
    let y2off = 1 + rng.randint1(3);
    let x2off = 2 + rng.randint1(8);
    let mut ysize = y1off + y2off + 1;
    let mut xsize = x1off + x2off + 1;
    ysize = ysize.min(bh * BLOCK_HGT - 4).max(4);
    xsize = xsize.min(bw * BLOCK_WID - 4).max(5);

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

    // full floor under room (+1 margin lit as room floor then outer walls)
    for y in (y1 - 1)..=(y2 + 1) {
        for x in (x1 - 1)..=(x2 + 1) {
            cave.set(x, y, Cell::Room);
        }
    }
    // outer walls
    for y in (y1 - 1)..=(y2 + 1) {
        cave.set(x1 - 1, y, Cell::Outer);
        cave.set(x2 + 1, y, Cell::Outer);
    }
    for x in (x1 - 1)..=(x2 + 1) {
        cave.set(x, y1 - 1, Cell::Outer);
        cave.set(x, y2 + 1, Cell::Outer);
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
    fill_rect_room(cave, x1a.max(2), y1a.max(2), x2a.min(cave.w - 3), y2a.min(cave.h - 3));
    // draw rect B
    fill_rect_room(cave, x1b.max(2), y1b.max(2), x2b.min(cave.w - 3), y2b.min(cave.h - 3));

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
