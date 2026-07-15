//! Tile grid — frog `cave[][]` feat cells.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::feat::Feat;

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    /// row-major feat id
    pub cells: Vec<Feat>,
}

impl Grid {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Feat> {
        if !self.in_bounds(x, y) {
            return None;
        }
        Some(self.cells[(y * self.width + x) as usize])
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.get(x, y).map(|f| f.walk()).unwrap_or(false)
    }

    pub fn buildable(&self, x: i32, y: i32) -> bool {
        self.get(x, y).map(|f| f.build()).unwrap_or(false)
    }

    pub fn set(&mut self, x: i32, y: i32, f: Feat) {
        if self.in_bounds(x, y) {
            self.cells[(y * self.width + x) as usize] = f;
        }
    }
}
