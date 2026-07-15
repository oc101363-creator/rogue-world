//! Tile grid — Frog `cave_type` / feat idea without content.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Frog-style terrain flags (minimal subset of f_info MOVE/PLACE).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainFlags {
    pub walk: bool,
    pub build: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Wall,
    Floor,
}

impl Terrain {
    pub fn flags(self) -> TerrainFlags {
        match self {
            Terrain::Wall => TerrainFlags {
                walk: false,
                build: false,
            },
            Terrain::Floor => TerrainFlags {
                walk: true,
                build: true,
            },
        }
    }

    pub fn glyph(self) -> char {
        match self {
            Terrain::Wall => '#',
            Terrain::Floor => '.',
        }
    }
}

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Grid {
    pub width: i32,
    pub height: i32,
    /// row-major: index = y * width + x
    pub cells: Vec<Terrain>,
}

impl Grid {
    pub fn new_bordered(width: i32, height: i32) -> Self {
        let mut cells = vec![Terrain::Floor; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                    cells[(y * width + x) as usize] = Terrain::Wall;
                }
            }
        }
        Self {
            width,
            height,
            cells,
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Terrain> {
        if !self.in_bounds(x, y) {
            return None;
        }
        Some(self.cells[(y * self.width + x) as usize])
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            .map(|t| t.flags().walk)
            .unwrap_or(false)
    }

    pub fn buildable(&self, x: i32, y: i32) -> bool {
        self.get(x, y)
            .map(|t| t.flags().build)
            .unwrap_or(false)
    }
}
