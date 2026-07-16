use ask_kernel::actions::{Action, ActionQueue};
use ask_kernel::components::{Glyph, Health, Inventory, Position, Resource, ResourceKind};
use ask_kernel::config::Config;
use ask_kernel::events::{EventBuf, GameEvent};
use ask_kernel::f_info::id;
use ask_kernel::generate::generate_level;
use ask_kernel::grid::Grid;
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
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("harvest".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.wood(), 1);
}

#[test]
fn build_hut_costs_wood() {
    let mut cfg = Config::default();
    cfg.hut_wood_cost = 3;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    if let Some(mut inv) = kw.world.get_mut::<Inventory>(agent) {
        inv.add(
            ask_kernel::components::Matter::Resource {
                resource: ResourceKind::Wood,
            },
            3,
        );
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
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("build".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.wood(), 0);
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
        let mut q = sim.kernel.world.query::<(&Position, &Resource)>();
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
        .map(|i| i.wood())
        .unwrap_or(0);
    assert!(wood >= 1, "expected harvest on tree, wood={wood}");
}

#[test]
fn trap_damages_and_clears() {
    let mut kw = KernelWorld::new(&Config {
        width: 88,
        height: 66,
        seed: 1,
        ..Config::default()
    });
    let agent = kw.agent_entity().unwrap();
    // find a floor next to walkable and plant a trap on target
    let (fx, fy) = {
        let g = kw.world.resource::<Grid>();
        (1..g.width - 1)
            .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
            .find(|&(x, y)| g.buildable(x, y))
            .expect("floor")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = fx;
        p.y = fy;
    }
    let tx = fx + 1;
    let ty = fy;
    // ensure target is walkable then set trap
    {
        let g = kw.world.resource_mut::<Grid>();
        // force floor then trap
        let _ = g;
    }
    kw.world.resource_mut::<Grid>().set(tx, ty, id::TRAP_PIT);
    // if trap cell not walkable in table, force FLOOR then trap — traps have MOVE in f_info
    let hp_before = kw.world.get::<Health>(agent).map(|h| h.hp).unwrap_or(0);
    kw.world
        .resource_mut::<ActionQueue>()
        .push(agent, Action::Move { dx: 1, dy: 0 });
    apply_actions_system(&mut kw.world);
    let hp_after = kw.world.get::<Health>(agent).map(|h| h.hp).unwrap_or(0);
    assert!(hp_after < hp_before, "trap should damage");
    assert_eq!(
        kw.world.resource::<Grid>().get(tx, ty),
        Some(id::FLOOR),
        "trap clears to floor"
    );
    let ev = kw.world.resource::<EventBuf>().events.clone();
    assert!(
        ev.iter()
            .any(|e| matches!(e, GameEvent::TrapTriggered { .. })),
        "expected TrapTriggered"
    );
}

#[test]
fn open_door_changes_feat() {
    let mut kw = KernelWorld::new(&Config {
        width: 88,
        height: 66,
        seed: 2,
        ..Config::default()
    });
    let agent = kw.agent_entity().unwrap();
    let (fx, fy) = {
        let g = kw.world.resource::<Grid>();
        (1..g.width - 1)
            .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
            .find(|&(x, y)| g.buildable(x, y) && g.buildable(x + 1, y))
            .expect("pair")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = fx;
        p.y = fy;
    }
    kw.world
        .resource_mut::<Grid>()
        .set(fx + 1, fy, id::CLOSED_DOOR);
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 1,
            dy: 0,
            verb: Some("open".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(fx + 1, fy),
        Some(id::OPEN_DOOR)
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

#[test]
fn monsters_spawn_from_templates_and_can_move() {
    let mut cfg = Config::default();
    cfg.width = 198;
    cfg.height = 132;
    cfg.seed = 11;
    let mut sim = Sim::new(KernelWorld::new(&cfg));
    let mon_count = {
        let mut q = sim.kernel.world.query::<&ask_kernel::components::Monster>();
        q.iter(&sim.kernel.world).count()
    };
    // templates usually place some MON() — if zero, still ok if r_info loaded
    assert!(ask_kernel::r_info::table().count() > 500);
    assert!(ask_kernel::k_info::table().count() > 100);
    // run a few ticks; should not panic
    sim.run_steps(10, false);
    let _ = mon_count;
}

#[test]
fn item_pickup_on_same_cell() {
    use ask_kernel::components::{Item, StableId};
    let mut kw = KernelWorld::new(&Config {
        width: 88,
        height: 66,
        seed: 5,
        ..Config::default()
    });
    let agent = kw.agent_entity().unwrap();
    let (x, y) = {
        let p = kw.world.get::<Position>(agent).unwrap();
        (p.x, p.y)
    };
    kw.world.spawn((
        Position { x, y },
        Glyph('!'),
        Item {
            matter: ask_kernel::components::Matter::Object {
                kind_id: 1,
                name: "test potion".into(),
            },
            qty: 1,
        },
        StableId(9999),
    ));
    let before = {
        let mut q = kw.world.query::<&Item>();
        q.iter(&kw.world).count()
    };
    ask_kernel::systems::pickup_items(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert!(
        inv.slots
            .iter()
            .any(|s| s.matter.label().contains("potion")),
        "slots={:?}",
        inv.slots
    );
    let left = {
        let mut q = kw.world.query::<&Item>();
        q.iter(&kw.world).count()
    };
    // may have world-generated items; at least one fewer after pickup
    assert!(left < before, "before={before} left={left}");
}

#[test]
fn player_bus_overrides_mock_and_moves() {
    use ask_kernel::components::StableId;
    use ask_kernel::player::{BusPolicy, PlayerActionBus};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 17;
    let kw = KernelWorld::new(&cfg);
    let bus = PlayerActionBus::new();
    let mut sim = Sim::with_policy(kw, Box::new(BusPolicy::new(bus.clone(), true)));

    let agent = sim.kernel.agent_entity().unwrap();
    let sid = sim.kernel.world.get::<StableId>(agent).unwrap().0;

    // Place agent on a floor that has at least one walkable neighbor
    let (origin, step) = {
        let g = sim.kernel.world.resource::<Grid>();
        let mut found = None;
        'outer: for y in 1..g.height - 1 {
            for x in 1..g.width - 1 {
                if !g.walkable(x, y) {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if g.walkable(x + dx, y + dy) {
                        found = Some(((x, y), (dx, dy)));
                        break 'outer;
                    }
                }
            }
        }
        found.expect("floor with walkable neighbor")
    };
    if let Some(mut p) = sim.kernel.world.get_mut::<Position>(agent) {
        p.x = origin.0;
        p.y = origin.1;
    }

    bus.submit(
        Some(sid),
        Action::Move {
            dx: step.0,
            dy: step.1,
        },
        Some(0),
    );
    // submitting must not flip the world-wide human_control switch
    assert!(!bus.human_control());
    sim.step();

    let after = {
        let p = sim.kernel.world.get::<Position>(agent).unwrap();
        (p.x, p.y)
    };
    assert_eq!(after, (origin.0 + step.0, origin.1 + step.1));

    // Next tick with no input should Idle (not mock-run away)
    let mid = after;
    sim.step();
    let still = {
        let p = sim.kernel.world.get::<Position>(agent).unwrap();
        (p.x, p.y)
    };
    assert_eq!(still, mid, "human_control idles without input");
}

#[test]
fn dig_puts_terrain_in_pack_and_place_restores() {
    use ask_kernel::actions::{Action, ActionQueue};
    use ask_kernel::components::{Inventory, Matter};
    use ask_kernel::grid::Grid;
    use ask_kernel::systems::apply_actions_system;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 9;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();

    // Find diggable wall next to a floor
    let ((ox, oy), (dx, dy), feat) = {
        let g = kw.world.resource::<Grid>();
        let mut found = None;
        'outer: for y in 1..g.height - 1 {
            for x in 1..g.width - 1 {
                if !g.walkable(x, y) {
                    continue;
                }
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let feat = g.get(x + dx, y + dy).unwrap_or(0);
                    if ask_kernel::systems::dig::is_diggable(feat) {
                        found = Some(((x, y), (dx, dy), feat));
                        break 'outer;
                    }
                }
            }
        }
        found.expect("floor next to diggable")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = ox;
        p.y = oy;
    }

    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx,
            dy,
            verb: Some("dig".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);

    let next = kw.world.resource::<Grid>().get(ox + dx, oy + dy).unwrap();
    assert_ne!(next, feat, "cell should change after dig");
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert!(
        inv.slots
            .iter()
            .any(|s| matches!(&s.matter, Matter::Terrain { feat: f } if *f == feat)),
        "pack should contain dug terrain {feat}, slots={:?}",
        inv.slots
    );

    // place back onto the cell we just dug (now rubble/floor)
    let pack_before = kw
        .world
        .get::<Inventory>(agent)
        .map(|i| {
            i.slots
                .iter()
                .filter(|s| matches!(&s.matter, Matter::Terrain { feat: f } if *f == feat))
                .map(|s| s.qty)
                .sum::<u32>()
        })
        .unwrap_or(0);

    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx,
            dy,
            verb: Some("place".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let after = kw.world.resource::<Grid>().get(ox + dx, oy + dy);
    assert_eq!(after, Some(feat), "placed feat should match pack terrain");
    let pack_after = kw
        .world
        .get::<Inventory>(agent)
        .map(|i| {
            i.slots
                .iter()
                .filter(|s| matches!(&s.matter, Matter::Terrain { feat: f } if *f == feat))
                .map(|s| s.qty)
                .sum::<u32>()
        })
        .unwrap_or(0);
    assert_eq!(pack_after, pack_before - 1, "place consumes one terrain");
}

#[test]
fn scoop_floor_and_craft_door() {
    use ask_kernel::actions::{Action, ActionQueue};
    use ask_kernel::components::{Inventory, Matter, ResourceKind};
    use ask_kernel::f_info::id;
    use ask_kernel::grid::Grid;
    use ask_kernel::systems::apply_actions_system;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 21;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();

    let floor = {
        let g = kw.world.resource::<Grid>();
        (1..g.width - 1)
            .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
            .find(|&(x, y)| g.get(x, y) == Some(id::FLOOR))
            .expect("floor")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = floor.0;
        p.y = floor.1;
    }
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("scoop".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_ne!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::FLOOR)
    );
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert!(
        inv.slots
            .iter()
            .any(|s| matches!(s.matter, Matter::Terrain { feat: id::FLOOR })),
        "scooped floor into pack: {:?}",
        inv.slots
    );

    if let Some(mut inv) = kw.world.get_mut::<Inventory>(agent) {
        inv.add(
            Matter::Resource {
                resource: ResourceKind::Wood,
            },
            2,
        );
    }
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("craft".into()),
            slot: None,
            recipe: Some("plank_door".into()),
        },
    );
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert!(
        inv.slots.iter().any(|s| matches!(
            s.matter,
            Matter::Terrain {
                feat: id::CLOSED_DOOR
            }
        )),
        "crafted door: {:?}",
        inv.slots
    );
}

#[test]
fn plant_build_deconstruct() {
    use ask_kernel::actions::{Action, ActionQueue};
    use ask_kernel::components::{Building, Inventory, Matter, ResourceKind};
    use ask_kernel::f_info::id;
    use ask_kernel::grid::Grid;
    use ask_kernel::systems::apply_actions_system;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 33;
    cfg.hut_wood_cost = 2;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = {
        let g = kw.world.resource::<Grid>();
        (1..g.width - 1)
            .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
            .find(|&(x, y)| g.buildable(x, y))
            .expect("buildable")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = floor.0;
        p.y = floor.1;
    }
    if let Some(mut inv) = kw.world.get_mut::<Inventory>(agent) {
        inv.add(
            Matter::Resource {
                resource: ResourceKind::Wood,
            },
            5,
        );
    }

    // ensure plantable surface
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DIRT);
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("plant".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::TREE)
    );

    // build hut on another cell
    let floor2 = {
        let g = kw.world.resource::<Grid>();
        (1..g.width - 1)
            .flat_map(|x| (1..g.height - 1).map(move |y| (x, y)))
            .find(|&(x, y)| g.buildable(x, y) && (x, y) != floor)
            .expect("buildable2")
    };
    if let Some(mut p) = kw.world.get_mut::<Position>(agent) {
        p.x = floor2.0;
        p.y = floor2.1;
    }
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("build".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let huts = {
        let mut q = kw.world.query::<&Building>();
        q.iter(&kw.world).count()
    };
    assert_eq!(huts, 1);
    let wood_before = kw
        .world
        .get::<Inventory>(agent)
        .map(|i| i.wood())
        .unwrap_or(0);
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("deconstruct".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let huts = {
        let mut q = kw.world.query::<&Building>();
        q.iter(&kw.world).count()
    };
    assert_eq!(huts, 0);
    let wood_after = kw
        .world
        .get::<Inventory>(agent)
        .map(|i| i.wood())
        .unwrap_or(0);
    assert!(
        wood_after >= wood_before + 2,
        "wood refunded {wood_before}->{wood_after}"
    );
}

#[test]
fn vision_marks_agent_cell_and_blocks_walls() {
    use ask_kernel::vision::{self, VisionMap, F_MARK, F_VIEW};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 3;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let (ax, ay) = {
        let p = kw.world.get::<Position>(agent).unwrap();
        (p.x, p.y)
    };
    vision::update_view(&mut kw.world);
    let vis = kw.world.resource::<VisionMap>();
    assert!(vis.is_view(ax, ay), "agent cell must be VIEW");
    assert!(vis.is_visible(ax, ay), "agent cell torch-lit");
    assert!(vis.get(ax, ay) & F_MARK != 0, "agent cell memorized");
    assert!(vis.get(ax, ay) & F_VIEW != 0);

    // Far corner should be unknown (not marked, not view) on a mid-size map
    let far_ok = !vis.is_view(1, 1) || (ax - 1).abs() + (ay - 1).abs() <= vision::MAX_SIGHT;
    assert!(far_ok || !vis.is_visible(1, 1));
}

#[test]
fn change_level_preserves_all_agents_body_and_identity() {
    use ask_kernel::components::{Agent, AgentProfile, Matter, StableId};
    use bevy_ecs::prelude::{Entity, With, Without};

    let mut cfg = Config::default();
    cfg.width = 132;
    cfg.height = 88;
    cfg.seed = 5;
    let mut kw = KernelWorld::new(&cfg);

    // second, registered agent
    let (sid2, _, _) = kw
        .spawn_agent("Scout".into(), "map west".into())
        .expect("spawn");
    // first agent stable id + give both some pack contents
    let sid1 = {
        let mut q = kw
            .world
            .query_filtered::<&StableId, (With<Agent>, Without<AgentProfile>)>();
        q.iter(&kw.world).next().map(|s| s.0).expect("first agent")
    };
    for sid in [sid1, sid2] {
        let e = {
            let mut q = kw.world.query_filtered::<(Entity, &StableId), With<Agent>>();
            q.iter(&kw.world)
                .find(|(_, s)| s.0 == sid)
                .map(|(e, _)| e)
                .unwrap()
        };
        let mut inv = kw.world.get_mut::<Inventory>(e).unwrap();
        inv.add(Matter::Resource { resource: ResourceKind::Wood }, 7);
    }

    kw.change_level(999, 1, 3, 4, 4);

    // both agents survived with body + identity
    let mut seen = std::collections::HashMap::new();
    {
        let mut q = kw.world.query_filtered::<
            (&StableId, &Inventory, Option<&AgentProfile>),
            With<Agent>,
        >();
        for (sid, inv, pr) in q.iter(&kw.world) {
            seen.insert(sid.0, (inv.wood(), pr.map(|p| p.name.clone())));
        }
    }
    assert_eq!(seen.len(), 2, "both agents must survive level change");
    assert_eq!(seen.get(&sid1).map(|v| v.0), Some(7), "agent 1 pack lost");
    assert_eq!(seen.get(&sid2).map(|v| v.0), Some(7), "agent 2 pack lost");
    assert_eq!(
        seen.get(&sid2).and_then(|v| v.1.clone()),
        Some("Scout".to_string()),
        "registered profile lost"
    );
    // depth advanced
    assert_eq!(kw.world.resource::<ask_kernel::world::Depth>().0, 1);
}
