#[derive(Clone, Debug)]
pub struct Config {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub tree_amount: u32,
    pub iron_amount: u32,
    pub hut_wood_cost: u32,
    /// How many tree entities to scatter
    pub tree_count: u32,
    pub iron_count: u32,
    pub room_count: u32,
    pub room_min_size: i32,
    pub room_max_size: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Larger map (frog levels are much bigger; this is a practical MVP size)
            width: 48,
            height: 32,
            seed: 1,
            tree_amount: 4,
            iron_amount: 4,
            hut_wood_cost: 3,
            tree_count: 18,
            iron_count: 10,
            room_count: 10,
            room_min_size: 4,
            room_max_size: 9,
        }
    }
}
