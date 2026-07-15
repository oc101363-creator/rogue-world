#[derive(Clone, Debug)]
pub struct Config {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub tree_amount: u32,
    pub iron_amount: u32,
    pub hut_wood_cost: u32,
    pub tree_count: u32,
    pub iron_count: u32,
    /// Kept for API compat; room count is now derived from map area like frog.
    pub room_count: u32,
    pub room_min_size: i32,
    pub room_max_size: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // ~10× area of previous 96×64 (6144 → ~61440).
            // Multiples of frog BLOCK 11: 33*11=363, 18*11=198 → 363×198 = 71874 cells.
            width: 363,
            height: 198,
            seed: 1,
            tree_amount: 4,
            iron_amount: 4,
            hut_wood_cost: 3,
            tree_count: 220,
            iron_count: 120,
            room_count: 0, // derived
            room_min_size: 4,
            room_max_size: 10,
        }
    }
}
