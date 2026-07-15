//! Frog `grid.c` :: `build_tunnel` — wandering dig with wall/door lists.

use super::{Cave, Cell, Rng};

#[derive(Default, Debug)]
pub struct DunTunnel {
    /// Outer-wall piercings → become doors later
    pub walls: Vec<(i32, i32)>,
    /// Corridor junctions → optional doors
    pub doors: Vec<(i32, i32)>,
}

pub fn correct_dir(y1: i32, x1: i32, y2: i32, x2: i32) -> (i32, i32) {
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

/// Port of frog build_tunnel (structure + door/wall collection).
pub fn build_tunnel(
    cave: &mut Cave,
    dun: &mut DunTunnel,
    rng: &mut Rng,
    mut y1: i32,
    mut x1: i32,
    y2: i32,
    x2: i32,
) -> bool {
    let start_y = y1;
    let start_x = x1;
    let mut main_loop = 0;
    let mut door_flag = false;
    let (mut row_dir, mut col_dir) = correct_dir(y1, x1, y2, x2);

    // frog tun params mid-range
    let tun_chg = 40;
    let tun_rnd = 10;
    let tun_con = 25; // chance to NOT early-terminate at junction

    while y1 != y2 || x1 != x2 {
        main_loop += 1;
        if main_loop > 4000 {
            return false;
        }

        if rng.percent(tun_chg) {
            let (rd, cd) = correct_dir(y1, x1, y2, x2);
            row_dir = rd;
            col_dir = cd;
            if rng.percent(tun_rnd) {
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
            if rng.percent(tun_rnd) {
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
                // pierce: save wall, solidify neighbors
                y1 = ty;
                x1 = tx;
                dun.walls.push((x1, y1));
                for j in (y1 - 1)..=(y1 + 1) {
                    for i in (x1 - 1)..=(x1 + 1) {
                        if cave.get(i, j) == Cell::Outer {
                            // solid — mark as Solid so no double pierce mess
                            // (keep as Outer only on the pierced cell which becomes tunnel)
                        }
                    }
                }
                cave.set(x1, y1, Cell::Tunnel);
                // neighbors outer → treat as solid rock (can't re-pierce)
                for j in (y1 - 1)..=(y1 + 1) {
                    for i in (x1 - 1)..=(x1 + 1) {
                        if i == x1 && j == y1 {
                            continue;
                        }
                        if cave.get(i, j) == Cell::Outer {
                            cave.set(i, j, Cell::Solid);
                        }
                    }
                }
                door_flag = false;
            }
            Cell::Room | Cell::Inner => {
                y1 = ty;
                x1 = tx;
            }
            Cell::Solid => {
                y1 = ty;
                x1 = tx;
                // 3-wide corridor (center + sides) for more open floors
                cave.set(x1, y1, Cell::Tunnel);
                if row_dir != 0 {
                    // vertical dig → widen on x
                    if cave.in_bounds(x1 - 1, y1) && cave.get(x1 - 1, y1) == Cell::Solid {
                        cave.set(x1 - 1, y1, Cell::Tunnel);
                    }
                    if cave.in_bounds(x1 + 1, y1) && cave.get(x1 + 1, y1) == Cell::Solid {
                        cave.set(x1 + 1, y1, Cell::Tunnel);
                    }
                } else {
                    // horizontal dig → widen on y
                    if cave.in_bounds(x1, y1 - 1) && cave.get(x1, y1 - 1) == Cell::Solid {
                        cave.set(x1, y1 - 1, Cell::Tunnel);
                    }
                    if cave.in_bounds(x1, y1 + 1) && cave.get(x1, y1 + 1) == Cell::Solid {
                        cave.set(x1, y1 + 1, Cell::Tunnel);
                    }
                }
                door_flag = false;
            }
            Cell::Tunnel => {
                y1 = ty;
                x1 = tx;
                // junction
                if !door_flag {
                    dun.doors.push((x1, y1));
                    door_flag = true;
                }
                // early terminate sometimes (frog tun_con)
                if rng.randint0(100) >= tun_con {
                    let dy = (y1 - start_y).abs();
                    let dx = (x1 - start_x).abs();
                    if dy > 10 || dx > 10 {
                        break;
                    }
                }
            }
        }
    }
    true
}
