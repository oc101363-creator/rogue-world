//! Game balance — every gameplay number lives here, exactly once.
//!
//! If a number affects play (hp, damage, heal, ranges, costs), it belongs in
//! this module, not scattered across systems. Data files (r_info) may later
//! override per-race values; until then these are the single source.

use crate::f_info::id;

// --- body ---
pub const AGENT_HP: i32 = 20;
pub const MONSTER_HP: i32 = 8;

// --- melee ---
pub const MONSTER_DAMAGE: i32 = 2;
pub const PLAYER_MELEE_DAMAGE: i32 = 3;

// --- recovery ---
pub const REST_HEAL: i32 = 1;
/// Rest heal multiplier when on/adjacent to a hut (shelter).
pub const HUT_REST_MULT: i32 = 2;

// --- monster AI ---
pub const MONSTER_CHASE_RANGE: i32 = 8;

// --- terrain hazards ---
pub const LAVA_DEEP_DAMAGE: i32 = 6;
pub const LAVA_SHALLOW_DAMAGE: i32 = 3;
pub const DEEP_WATER_DAMAGE: i32 = 1;

/// Trap damage by feat (frog place_trap spirit, simplified).
pub fn trap_damage(feat: u16) -> i32 {
    match feat {
        id::TRAP_TRAPDOOR => 4,
        id::TRAP_PIT | id::TRAP_SPIKED_PIT => 3,
        id::TRAP_POISON_PIT | id::TRAP_POISON => 2,
        id::TRAP_FIRE | id::TRAP_ACID => 4,
        id::TRAP_TY_CURSE => 5,
        _ => 2,
    }
}

// --- planting ---
/// Wood cost to plant a tree (a TREE block also works, it is worth 2 wood).
pub const PLANT_COST_WOOD: u32 = 2;
/// Yield of a planted tree. amount ≤ cost: planting moves wood, never prints it.
pub const PLANTED_TREE_AMOUNT: u32 = 2;
