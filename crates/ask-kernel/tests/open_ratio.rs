use ask_kernel::config::Config;
use ask_kernel::generate::generate_level;

#[test]
fn wall_ratio_around_20_percent() {
    let mut ratios = Vec::new();
    for seed in [1u64, 2, 3, 7, 42] {
        let mut cfg = Config::default();
        cfg.seed = seed;
        // mid size for speed
        cfg.width = 198;
        cfg.height = 132;
        let level = generate_level(&cfg);
        let total = level.grid.cells.len() as f64;
        let walls = level
            .grid
            .cells
            .iter()
            .filter(|&&id| !ask_kernel::f_info::table().walk(id))
            .count() as f64;
        ratios.push(walls / total);
    }
    let avg = ratios.iter().sum::<f64>() / ratios.len() as f64;
    eprintln!("wall ratios: {:?} avg={:.1}%", ratios.iter().map(|r| format!("{:.1}%", r*100.0)).collect::<Vec<_>>(), avg*100.0);
    // target ~20% rock; allow 12%–32% band
    assert!(avg > 0.10 && avg < 0.50, "avg wall ratio {avg} outside 10-50%");
}
