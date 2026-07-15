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
    assert!(
        level.rooms.len() >= 5,
        "rooms={} (frog block placement)",
        level.rooms.len()
    );
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
    let mut sim = Sim::new(KernelWorld::new(&Config::default()));
    sim.run_steps(200, false);
    let agent = sim.kernel.agent_entity().unwrap();
    let wood = sim
        .kernel
        .world
        .get::<Inventory>(agent)
        .map(|i| i.wood)
        .unwrap_or(0);
    let huts = {
        let mut q = sim.kernel.world.query::<&ask_kernel::components::Building>();
        q.iter(&sim.kernel.world).count()
    };
    assert!(
        wood >= 1 || huts >= 1,
        "expected wood gathered or hut built, wood={wood} huts={huts}"
    );
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
