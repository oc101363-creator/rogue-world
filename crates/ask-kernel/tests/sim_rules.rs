use ask_kernel::actions::{Action, ActionQueue};
use ask_kernel::components::{Agent, Inventory, Position, Resource, ResourceKind};
use ask_kernel::config::Config;
use ask_kernel::events::EventBuf;
use ask_kernel::persist;
use ask_kernel::systems::apply_actions_system;
use ask_kernel::tick::Sim;
use ask_kernel::world::KernelWorld;
use bevy_ecs::prelude::*;

#[test]
fn move_four_way_and_blocked_by_wall() {
    let mut kw = KernelWorld::new(&Config::default());
    let agent = kw.agent_entity().unwrap();
    // Place agent next to left wall at (1, 5)
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = 1;
        p.y = 5;
    }
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::Move { dx: -1, dy: 0 });
    apply_actions_system(&mut kw.world);
    let p = kw.world.get::<Position>(agent).unwrap();
    assert_eq!((p.x, p.y), (1, 5), "should not walk into wall");

    kw.world.resource_mut::<EventBuf>().clear();
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::Move { dx: 1, dy: 0 });
    apply_actions_system(&mut kw.world);
    let p = kw.world.get::<Position>(agent).unwrap();
    assert_eq!((p.x, p.y), (2, 5));
}

#[test]
fn harvest_increments_wood() {
    let mut kw = KernelWorld::new(&Config::default());
    let agent = kw.agent_entity().unwrap();
    // Put agent on a tree
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
    // Ensure open floor
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = 3;
        p.y = 5;
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
    sim.run_steps(80, false);
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
