//! Tile grid — frog `cave[y][x].feat` as u16 ids from f_info.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::f_info::{self, FeatId};

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    /// row-major frog feat ids
    pub cells: Vec<FeatId>,
}

impl Grid {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn get(&self, x: i32, y: i32) -> Option<FeatId> {
        if !self.in_bounds(x, y) {
            return None;
        }
        Some(self.cells[(y * self.width + x) as usize])
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            .map(|id| f_info::table().walk(id))
            .unwrap_or(false)
    }

    pub fn buildable(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            .map(|id| f_info::table().buildable(id))
            .unwrap_or(false)
    }

    pub fn set(&mut self, x: i32, y: i32, id: FeatId) {
        if self.in_bounds(x, y) {
            self.cells[(y * self.width + x) as usize] = id;
        }
    }

    pub fn glyph(&self, x: i32, y: i32) -> char {
        self.get(x, y)
            .map(|id| f_info::table().glyph(id))
            .unwrap_or(' ')
    }
}
