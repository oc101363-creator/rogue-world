#[derive(Clone, Debug)]
pub struct Config {
    pub width: i32,
    pub height: i32,
    pub seed: u64,
    pub tree_amount: u32,
    pub iron_amount: u32,
    pub hut_wood_cost: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 16,
            height: 12,
            seed: 1,
            tree_amount: 5,
            iron_amount: 5,
            hut_wood_cost: 3,
        }
    }
}
