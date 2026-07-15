use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph(pub char);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Agent;

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub wood: u32,
    pub iron: u32,
    /// Picked-up object kind ids / names (frog pack spirit, simplified).
    pub items: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Wood,
    Iron,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Resource {
    pub kind: ResourceKind,
    pub amount: u32,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Building;

/// Stable id for save/load (Frog entity index idea).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableId(pub u64);

/// Simple vitality for terrain hazards (lava / traps).
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Health {
    pub hp: i32,
    pub max_hp: i32,
}

impl Default for Health {
    fn default() -> Self {
        Self { hp: 20, max_hp: 20 }
    }
}

impl Health {
    pub fn damage(&mut self, n: i32) {
        self.hp = (self.hp - n).max(0);
    }
}

/// Frog monster instance (from r_info / template MON()).
#[derive(Component, Clone, Debug)]
pub struct Monster {
    pub race_id: u16,
    pub name: String,
    pub color: char,
}

/// Frog object instance (from k_info / template OBJ()).
#[derive(Component, Clone, Debug)]
pub struct Item {
    pub kind_id: u16,
    pub name: String,
    pub color: char,
}
