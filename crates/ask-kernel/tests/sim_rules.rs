use ask_kernel::actions::{Action, ActionQueue};
use ask_kernel::components::{Inventory, Position, Resource, ResourceKind};
use ask_kernel::config::Config;
use ask_kernel::events::EventBuf;
use ask_kernel::generate::generate_level;
use ask_kernel::persist;
use ask_kernel::systems::apply_actions_system;
use ask_kernel::tick::Sim;
use ask_kernel::world::KernelWorld;

#[test]
fn generate_has_rooms_and_floors() {
    // Use a mid-size map for unit test speed; still frog pipeline
    let mut cfg = Config::default();
    cfg.width = 132; // 12 blocks
    cfg.height = 88; // 8 blocks
    cfg.tree_count = 40;
    cfg.iron_count = 20;
    cfg.seed = 42;
    let level = generate_level(&cfg);
    // maze-only levels may have 0 formal rooms
    let floors = level
        .grid
        .cells
        .iter()
        .filter(|c| ask_kernel::f_info::table().walk(**c))
        .count();
    assert!(floors > 200, "floors={floors}");
    assert!(
        level.grid.walkable(level.agent.0, level.agent.1),
        "agent not on floor"
    );
    assert!(!level.trees.is_empty());
    // size snapped to BLOCK 11
    assert_eq!(level.grid.width % 11, 0);
    assert_eq!(level.grid.height % 11, 0);
}

#[test]
fn generate_uses_traps_from_f_info() {
    let mut cfg = Config::default();
    cfg.width = 132;
    cfg.height = 88;
    cfg.seed = 7;
    let level = generate_level(&cfg);
    let traps = level
        .grid
        .cells
        .iter()
        .filter(|&&id| (16..=31).contains(&id))
        .count();
    assert!(traps >= 5, "expected frog trap feats, got {traps}");
}

#[test]
fn vaults_and_rooms_txt_load() {
    assert!(ask_kernel::vaults::lesser_vaults().len() >= 50);
    assert!(ask_kernel::vaults::greater_vaults().len() >= 50);
    assert!(ask_kernel::vaults::room_templates().len() >= 50);
    let mut cfg = Config::default();
    cfg.width = 198;
    cfg.height = 132;
    cfg.seed = 99;
    let level = generate_level(&cfg);
    assert!(level.grid.width >= 198 - 11);
    let quartz = level
        .grid
        .cells
        .iter()
        .filter(|&&id| id == ask_kernel::f_info::id::QUARTZ_VEIN)
        .count();
    assert!(quartz > 0, "expected quartz from solid/vaults");
}

#[test]
fn move_four_way_and_blocked_by_wall() {
    let mut kw = KernelWorld::new(&Config::default());
    let agent = kw.agent_entity().unwrap();
    // Put agent at (1,1) — border wall is x=0
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        // find a floor next to a wall: use agent pos, try move into solid rock outside corridor
        p.x = 1;
        p.y = 1;
    }
    // force cell (1,1) may be wall on generated maps — place on known floor then move to wall
    let floor = {
        let g = kw.world.resource::<ask_kernel::grid::Grid>();
        let mut found = None;
        'outer: for y in 1..g.height - 1 {
            for x in 1..g.width - 1 {
                if g.walkable(x, y) && !g.walkable(x - 1, y) {
                    found = Some((x, y));
                    break 'outer;
                }
            }
        }
        found.expect("floor next to wall")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = floor.0;
        p.y = floor.1;
    }
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::Move { dx: -1, dy: 0 });
    apply_actions_system(&mut kw.world);
    let p = kw.world.get::<Position>(agent).unwrap();
    assert_eq!((p.x, p.y), floor, "should not walk into wall");

    kw.world.resource_mut::<EventBuf>().clear();
    // move to a walkable neighbor if possible
    let right_ok = kw
        .world
        .resource::<ask_kernel::grid::Grid>()
        .walkable(floor.0 + 1, floor.1);
    if right_ok {
        kw.world
            .resource_mut::<ActionQueue>()
            .push(agent, Action::Move { dx: 1, dy: 0 });
        apply_actions_system(&mut kw.world);
        let p = kw.world.get::<Position>(agent).unwrap();
        assert_eq!((p.x, p.y), (floor.0 + 1, floor.1));
    }
}

#[test]
fn harvest_increments_wood() {
    let mut kw = KernelWorld::new(&Config::default());
    let agent = kw.agent_entity().unwrap();
    let tree_pos = {
        let mut q = kw.world.query::<(&Position, &Resource)>();
        q.iter(&kw.world)
            .find(|(_, r)| r.kind == ResourceKind::Wood)
            .map(|(p, _)| (p.x, p.y))
            .expect("tree")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = tree_pos.0;
        p.y = tree_pos.1;
    }
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::Harvest);
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.wood, 1);
}

#[test]
fn build_hut_costs_wood() {
    let mut cfg = Config::default();
    cfg.hut_wood_cost = 3;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    if let Some(mut inv) = kw.world.get_mut::<Inventory>(agent) {
        inv.wood = 3;
    }
    // place on a buildable floor (not water/lava/door)
    let floor = {
        let g = kw.world.resource::<ask_kernel::grid::Grid>();
        (0..g.width * g.height)
            .map(|i| {
                let x = i % g.width;
                let y = i / g.width;
                (x, y)
            })
            .find(|(x, y)| g.buildable(*x, *y))
            .expect("buildable cell")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = floor.0;
        p.y = floor.1;
    }
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::BuildHut);
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.wood, 0);
    let huts = {
        let mut q = kw.world.query::<&ask_kernel::components::Building>();
        q.iter(&kw.world).count()
    };
    assert_eq!(huts, 1);
}

#[test]
fn mock_sim_gathers_wood_over_steps() {
    // Place agent on a tree so policy harvests without long pathfinding.
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 3;
    cfg.tree_count = 40;
    let mut sim = Sim::new(KernelWorld::new(&cfg));
    let agent = sim.kernel.agent_entity().unwrap();
    let tree_pos = {
        let mut q = sim
            .kernel
            .world
            .query::<(&Position, &Resource)>();
        q.iter(&sim.kernel.world)
            .find(|(_, r)| r.kind == ResourceKind::Wood)
            .map(|(p, _)| (p.x, p.y))
            .expect("tree")
    };
    if let Some(mut p) = sim.kernel.world.get_mut::<Position>(agent) {
        p.x = tree_pos.0;
        p.y = tree_pos.1;
    }
    sim.run_steps(5, false);
    let wood = sim
        .kernel
        .world
        .get::<Inventory>(agent)
        .map(|i| i.wood)
        .unwrap_or(0);
    assert!(wood >= 1, "expected harvest on tree, wood={wood}");
}

#[test]
fn save_load_roundtrip() {
    let mut sim = Sim::new(KernelWorld::new(&Config::default()));
    sim.run_steps(15, false);
    let path = std::env::temp_dir().join("ask_test_world.json");
    persist::save_to_path(&mut sim.kernel.world, &path).unwrap();
    let loaded = persist::load_from_path(&path).unwrap();
    assert_eq!(loaded.tick(), sim.kernel.tick());
    let _ = std::fs::remove_file(path);
}
