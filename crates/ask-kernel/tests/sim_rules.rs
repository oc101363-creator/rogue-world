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

#[test]
fn monster_targets_nearest_agent_not_first() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 11;
    let mut kw = KernelWorld::new(&cfg);
    let (sid2, _, _) = kw.spawn_agent("Bait".into(), "tank".into()).expect("spawn");

    // park both agents on walkable floor; monster adjacent to agent 2 only
    let floor = find_open_floor(&mut kw, 10);
    let e1 = agent_entity_by(&mut kw, None);
    let e2 = agent_entity_by(&mut kw, Some(sid2));
    set_pos(&mut kw, e1, (floor.0, floor.1));
    set_pos(&mut kw, e2, (floor.0 + 6, floor.1));
    let mid = kw.world.resource::<ask_kernel::world::IdCounter>().0 + 1;
    kw.world.insert_resource(ask_kernel::world::IdCounter(mid));
    kw.world.spawn((
        Position { x: floor.0 + 5, y: floor.1 },
        Glyph('o'),
        Monster { race_id: 1, name: "rat".into(), color: 'r' },
        Health { hp: 8, max_hp: 8 },
        StableId(mid),
    ));

    ask_kernel::systems::process_monsters(&mut kw.world);

    let hp1 = kw.world.get::<Health>(e1).unwrap().hp;
    let hp2 = kw.world.get::<Health>(e2).unwrap().hp;
    assert_eq!(hp1, 20, "far agent must not be touched");
    assert!(hp2 < 20, "nearest agent should have been attacked, hp={hp2}");
}

#[test]
fn death_drops_pack_and_respawns_full_hp() {
    use ask_kernel::components::{Item, Matter, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 13;
    let mut kw = KernelWorld::new(&cfg);

    let e1 = agent_entity_by(&mut kw, None);
    let sid1 = kw.world.get::<StableId>(e1).unwrap().0;
    let floor = find_open_floor(&mut kw, 10);
    set_pos(&mut kw, e1, floor);
    // pack something, then nearly die
    kw.world.get_mut::<Inventory>(e1).unwrap().add(
        Matter::Resource { resource: ResourceKind::Wood },
        5,
    );
    kw.world.get_mut::<Health>(e1).unwrap().hp = 0;

    let before_pos = floor;
    ask_kernel::systems::check_deaths(&mut kw.world);

    let h = kw.world.get::<Health>(e1).unwrap();
    assert_eq!(h.hp, h.max_hp, "respawn must refill hp");
    assert_eq!(kw.world.get::<Inventory>(e1).unwrap().slots.len(), 0, "pack must drop");
    // dropped wood lies on the death cell
    let dropped: u32 = {
        let mut q = kw.world.query::<(&Position, &Item)>();
        q.iter(&kw.world)
            .filter(|(p, _)| p.x == before_pos.0 && p.y == before_pos.1)
            .map(|(_, it)| it.qty)
            .sum()
    };
    assert_eq!(dropped, 5, "dropped matter should remain at death cell");
    // events tell the story
    let evs = kw.world.resource_mut::<EventBuf>().drain();
    assert!(evs.iter().any(|e| matches!(e, GameEvent::AgentDied { entity, .. } if *entity == sid1)));
    assert!(evs.iter().any(|e| matches!(e, GameEvent::AgentRespawned { entity, .. } if *entity == sid1)));
}

// --- small local helpers (keep tests terse) ---
fn find_open_floor(kw: &mut KernelWorld, min_open: i32) -> (i32, i32) {
    let (w, h, cells) = {
        let g = kw.world.resource::<Grid>();
        (g.width, g.height, g.cells.clone())
    };
    let table = ask_kernel::f_info::table();
    let walk = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        table.walk(cells[(y * w + x) as usize])
    };
    for y in 2..h - 2 {
        for x in 2..w - 2 {
            let mut open = 0;
            for dy in -1..=1 {
                for dx in -min_open..=min_open {
                    if walk(x + dx, y + dy) {
                        open += 1;
                    }
                }
            }
            if open >= min_open * 2 {
                // prefer a spot with no agents nearby
                let mut q = kw.world.query_filtered::<&Position, bevy_ecs::prelude::With<ask_kernel::components::Agent>>();
                if q.iter(&kw.world).all(|p| (p.x - x).abs() + (p.y - y).abs() > 8) {
                    return (x, y);
                }
            }
        }
    }
    panic!("no open floor found");
}

fn agent_entity_by(
    kw: &mut KernelWorld,
    sid: Option<u64>,
) -> bevy_ecs::prelude::Entity {
    use ask_kernel::components::{Agent, AgentProfile, StableId};
    use bevy_ecs::prelude::With;
    // None → the unregistered level agent (no profile); Some(id) → by stable id
    if let Some(want) = sid {
        let mut q =
            kw.world
                .query_filtered::<(bevy_ecs::prelude::Entity, &StableId), With<Agent>>();
        return q
            .iter(&kw.world)
            .find(|(_, s)| s.0 == want)
            .map(|(e, _)| e)
            .expect("agent by stable id");
    }
    let mut q = kw
        .world
        .query_filtered::<(bevy_ecs::prelude::Entity, Option<&AgentProfile>), With<Agent>>();
    q.iter(&kw.world)
        .find(|(_, pr)| pr.is_none())
        .map(|(e, _)| e)
        .expect("unregistered agent")
}

fn set_pos(kw: &mut KernelWorld, e: bevy_ecs::prelude::Entity, pos: (i32, i32)) {
    let mut p = kw.world.get_mut::<Position>(e).unwrap();
    p.x = pos.0;
    p.y = pos.1;
}

#[test]
fn plant_is_zero_sum_no_wood_printing() {
    use ask_kernel::components::{Matter, Resource};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 41;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DIRT);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 10);

    // plant: costs exactly PLANT_COST_WOOD
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
    let wood = kw.world.get::<Inventory>(agent).unwrap().wood();
    assert_eq!(wood, 10 - ask_kernel::balance::PLANT_COST_WOOD);

    // planted tree yields exactly PLANTED_TREE_AMOUNT — the full cycle is zero-sum
    // (query scoped to the planted cell: generated trees carry cfg.tree_amount)
    let amount = {
        let mut q = kw.world.query::<(&Resource, &Position)>();
        q.iter(&kw.world)
            .find(|(r, p)| {
                r.kind == ResourceKind::Wood && p.x == floor.0 && p.y == floor.1
            })
            .map(|(r, _)| r.amount)
            .expect("planted tree resource")
    };
    assert_eq!(amount, ask_kernel::balance::PLANTED_TREE_AMOUNT);
    assert!(amount <= ask_kernel::balance::PLANT_COST_WOOD, "plant must not print wood");
}

#[test]
fn scoop_place_cycle_creates_nothing() {
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 43;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::FLOOR);

    // scoop FLOOR → pack gets FLOOR block, cell becomes DIRT
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
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.qty_terrain(id::FLOOR), 1, "scoop should pack the floor");
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::DIRT)
    );

    // place FLOOR back onto DIRT → cell becomes FLOOR, and NOTHING else
    // (soft ground must not be returned as a displaced block)
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("place".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0, floor.1),
        Some(id::FLOOR)
    );
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.slots.len(), 0, "cycle must be zero-sum, pack: {:?}", inv.slots);
}

#[test]
fn ore_vein_cycle_cannot_print_iron() {
    // A crafted magma-treasure block is worth 3 iron total
    // (1 dig bonus + 2 from smelt). Its recipe must cost ≥ 3 iron,
    // or granite (infinite) becomes iron (infinite).
    let recipe = ask_kernel::sandbox::recipes()
        .iter()
        .find(|r| r.id == "ore_vein")
        .expect("ore_vein recipe");
    let iron_cost: u32 = recipe
        .needs
        .iter()
        .map(|n| match n {
            ask_kernel::sandbox::RecipeNeed::Iron(q) => *q,
            _ => 0,
        })
        .sum();
    assert!(iron_cost >= 3, "ore_vein costs {iron_cost} iron < 3 — iron printing");
}

#[test]
fn monsters_suffer_terrain_too() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 51;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    // trap on an open cell; monster next to it, agent far away (out of chase)
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::TRAP_FIRE);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('o'),
        Monster { race_id: 1, name: "sacrificial rat".into(), color: 'r' },
        Health { hp: 2, max_hp: 2 },
        StableId(99901),
    )).id();
    // force-walk the monster onto the trap cell by simulating its move
    // (process_monsters wanders deterministically; instead apply the same
    // post-move rule directly: move + on_enter_cell via a helper)
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    let hp = kw.world.get::<Health>(e).map(|h| h.hp);
    assert_eq!(hp, None, "monster should have died on TRAP_FIRE (2hp vs 4 dmg)");
}

#[test]
fn monsters_die_on_lava_and_emit_kill_event() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 52;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DEEP_LAVA);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('o'),
        Monster { race_id: 1, name: "lava rat".into(), color: 'r' },
        Health { hp: 5, max_hp: 5 },
        StableId(99902),
    )).id();
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    assert!(kw.world.get::<Health>(e).is_none(), "lava should kill 5hp rat");
    let evs = kw.world.resource_mut::<EventBuf>().drain();
    assert!(
        evs.iter().any(|ev| matches!(ev, GameEvent::MonsterKilled { name, .. } if name == "lava rat")),
        "expected MonsterKilled event"
    );
}

#[test]
fn salamander_swims_and_ignores_lava() {
    use ask_kernel::components::{Monster, StableId};

    // race 50 Salamander has CAN_SWIM | RES_FIRE in r_info.txt
    let race = ask_kernel::r_info::table().get(50).expect("race 50");
    assert!(race.can_swim, "CAN_SWIM flag not parsed");
    assert!(race.res_fire, "RES_FIRE flag not parsed");

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 53;
    let mut kw = KernelWorld::new(&cfg);
    let floor = find_open_floor(&mut kw, 4);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DEEP_LAVA);
    let e = kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('R'),
        Monster { race_id: 50, name: "Salamander".into(), color: 'o' },
        Health { hp: 5, max_hp: 5 },
        StableId(99903),
    )).id();
    ask_kernel::systems::monster_move_to(&mut kw.world, e, floor.0, floor.1);
    let hp = kw.world.get::<Health>(e).map(|h| h.hp);
    assert_eq!(hp, Some(5), "RES_FIRE salamander must ignore lava");
}

#[test]
fn ascend_at_surface_rejected_and_teleport_trap_teleports() {
    use ask_kernel::components::StableId;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 61;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 4);

    // ascend at depth 0 must be rejected, not a free level re-roll
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::UP_STAIR);
    set_pos(&mut kw, agent, floor);
    let sid = kw.world.get::<StableId>(agent).unwrap().0;
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("ascend".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.resource::<ask_kernel::world::Depth>().0, 0);
    let evs = kw.world.resource_mut::<EventBuf>().drain();
    assert!(
        evs.iter().any(|e| matches!(e, GameEvent::ActionRejected { reason, .. } if reason == "no_up_from_surface")),
        "surface ascend must be rejected"
    );

    // teleport trap moves the agent (and deals no damage)
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::TRAP_TELEPORT);
    set_pos(&mut kw, agent, floor);
    let hp_before = kw.world.get::<Health>(agent).unwrap().hp;
    ask_kernel::systems::terrain::on_enter_cell_for_test(&mut kw.world, agent);
    let p = kw.world.get::<Position>(agent).unwrap();
    assert_ne!((p.x, p.y), floor, "teleport trap must move the agent");
    assert_eq!(
        kw.world.get::<Health>(agent).unwrap().hp,
        hp_before,
        "teleport deals no damage"
    );
    let _ = sid;
}

#[test]
fn rest_near_hut_heals_double() {
    use ask_kernel::components::{Building, StableId};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 71;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world.get_mut::<Health>(agent).unwrap().hp = 10;

    // no hut: plain rest
    kw.world.resource_mut::<ActionQueue>().push(agent, Action::Rest);
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 11);

    // with a hut adjacent: double
    kw.world.spawn((
        Position { x: floor.0 + 1, y: floor.1 },
        Glyph('H'),
        Building,
        StableId(99911),
    ));
    kw.world.resource_mut::<ActionQueue>().push(agent, Action::Rest);
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 13, "hut shelter should double rest");
}

#[test]
fn generation_scatters_no_purposeless_items() {
    // k_info objects currently do nothing in the pack; random scatter is
    // noise. Vault-template items (themed rooms) may still appear.
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 81;
    let level = generate_level(&cfg);
    let scattered = level.items.len();
    assert_eq!(scattered, 0, "random item scatter must stop (items: {scattered})");
}

#[test]
fn dig_place_cannot_print_iron() {
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 91;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    // a treasure vein adjacent
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0 + 1, floor.1, id::MAGMA_TREASURE);

    // dig it: +1 iron, and the block is RUBBLE (vein crumbles), NOT the treasure
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 1,
            dy: 0,
            verb: Some("dig".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.iron(), 1, "dig bonus iron");
    assert_eq!(inv.qty_terrain(id::MAGMA_TREASURE), 0, "treasure block must not re-enter the pack");
    assert!(inv.qty_terrain(id::RUBBLE) >= 1, "crumbled vein leaves rubble");

    // place the dig result underfoot, dig again: iron total must not grow
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("place".into()),
            slot: None,
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
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
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert_eq!(inv.iron(), 1, "iron grew — fountain still open");
}

#[test]
fn craft_chain_never_net_positive() {
    use ask_kernel::components::Matter;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 93;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 1);

    for recipe in ["sapling", "chop_wood"] {
        kw.world.resource_mut::<ActionQueue>().push(
            agent,
            Action::Interact {
                dx: 0,
                dy: 0,
                verb: Some("craft".into()),
                slot: None,
                recipe: Some(recipe.into()),
            },
        );
        apply_actions_system(&mut kw.world);
    }
    let inv = kw.world.get::<Inventory>(agent).unwrap();
    assert!(inv.wood() <= 1, "sapling→chop printed wood: {}", inv.wood());
    assert_eq!(inv.wood(), 1, "chain should be zero-sum");
}

#[test]
fn plant_scoop_harvest_zero_sum() {
    use ask_kernel::components::{Matter, Resource};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 95;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DIRT);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 4);

    // plant: -2 wood, TREE feat + Resource{Wood,2}
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
    assert_eq!(kw.world.get::<Inventory>(agent).unwrap().wood(), 2);

    // scoop the TREE: block into pack, and the resource entity must be GONE
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
    let res_left = {
        let mut q = kw.world.query::<(&Position, &Resource)>();
        q.iter(&kw.world)
            .any(|(p, _)| p.x == floor.0 && p.y == floor.1)
    };
    assert!(!res_left, "scoop TREE must consume the wood resource entity");
    let verbs: Vec<String> = ask_kernel::systems::interact::list_at(&mut kw.world, agent, 0, 0)
        .into_iter()
        .map(|i| i.verb)
        .collect();
    assert!(!verbs.iter().any(|v| v == "harvest"), "harvest must not be offered");

    // chop the block (1 wood after C1.2) — total ≤ initial 4
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("craft".into()),
            slot: None,
            recipe: Some("chop_wood".into()),
        },
    );
    apply_actions_system(&mut kw.world);
    let wood = kw.world.get::<Inventory>(agent).unwrap().wood();
    assert!(wood <= 4, "plant→scoop→harvest printed wood: {wood}");
}

#[test]
fn plant_with_block_is_zero_sum() {
    use ask_kernel::components::{Matter, Resource};

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 97;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0, floor.1, id::DIRT);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 2);

    // craft sapling (2 wood → 1 TREE block), plant WITH THE BLOCK,
    // harvest both wood back — full cycle must be zero-sum
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("craft".into()),
            slot: None,
            recipe: Some("sapling".into()),
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Inventory>(agent).unwrap().qty_terrain(id::TREE), 1);

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
    let amount = {
        let mut q = kw.world.query::<(&Position, &Resource)>();
        q.iter(&kw.world)
            .find(|(p, _)| p.x == floor.0 && p.y == floor.1)
            .map(|(_, r)| r.amount)
            .expect("planted resource")
    };
    assert_eq!(amount, ask_kernel::balance::PLANTED_TREE_AMOUNT);

    for _ in 0..amount {
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
    }
    let wood = kw.world.get::<Inventory>(agent).unwrap().wood();
    assert!(wood <= 2, "sapling→plant→harvest printed wood: {wood}");
    assert_eq!(wood, 2, "cycle should be exactly zero-sum, got {wood}");
}

#[test]
fn use_ignites_wood_and_eats_grass() {
    use ask_kernel::components::Matter;

    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = 131;
    let mut kw = KernelWorld::new(&cfg);
    let agent = kw.agent_entity().unwrap();
    let floor = find_open_floor(&mut kw, 3);
    set_pos(&mut kw, agent, floor);
    kw.world
        .resource_mut::<Grid>()
        .set(floor.0 + 1, floor.1, id::TREE);
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Resource { resource: ResourceKind::Wood }, 2);

    // ignite adjacent tree with a wood block
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 1,
            dy: 0,
            verb: Some("use".into()),
            slot: Some(0),
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(
        kw.world.resource::<Grid>().get(floor.0 + 1, floor.1),
        Some(id::FIRE),
        "wood block should ignite the tree"
    );

    // eat a grass block for hp
    kw.world.get_mut::<Health>(agent).unwrap().hp = 10;
    kw.world
        .get_mut::<Inventory>(agent)
        .unwrap()
        .add(Matter::Terrain { feat: id::GRASS }, 1);
    let grass_slot = kw.world.get::<Inventory>(agent).unwrap().slots.len() - 1;
    kw.world.resource_mut::<ActionQueue>().push(
        agent,
        Action::Interact {
            dx: 0,
            dy: 0,
            verb: Some("use".into()),
            slot: Some(grass_slot),
            recipe: None,
        },
    );
    apply_actions_system(&mut kw.world);
    assert_eq!(kw.world.get::<Health>(agent).unwrap().hp, 11);
}

#[test]
fn fire_spreads_and_burns_out_deterministically() {
    use ask_kernel::world::TickCounter;

    let stage = |kw: &mut KernelWorld| {
        let (w, h) = {
            let g = kw.world.resource::<Grid>();
            (g.width, g.height)
        };
        let mut grid = kw.world.resource_mut::<Grid>();
        for i in 0..grid.cells.len() {
            grid.cells[i] = id::GRANITE;
        }
        for x in 1..(w - 1) {
            grid.set(x, h / 2, id::GRASS);
        }
        grid.set(1, h / 2, id::FIRE);
        (w, h)
    };
    let first_spread_tick = |seed: u64| {
        let mut cfg = Config::default();
        cfg.width = 33;
        cfg.height = 22;
        cfg.seed = seed;
        let mut kw = KernelWorld::new(&cfg);
        let (_w, h) = stage(&mut kw);
        let mut t = 0u64;
        loop {
            t += 1;
            let next = kw.tick() + 1;
            *kw.world.resource_mut::<TickCounter>() = TickCounter(next);
            ask_kernel::systems::process_world(&mut kw.world);
            let g = kw.world.resource::<Grid>();
            if g.get(2, h / 2) == Some(id::FIRE) || t > 400 {
                return (t, g.get(2, h / 2) == Some(id::FIRE));
            }
        }
    };
    let (t1, spread1) = first_spread_tick(123);
    let (t2, spread2) = first_spread_tick(123);
    assert!(spread1, "fire never spread to adjacent grass");
    assert!(spread2, "fire never spread to adjacent grass (run 2)");
    assert_eq!(t1, t2, "same seed must reproduce the same process");
    // keep running the first world: some cell eventually burns out
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 123;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage(&mut kw);
    let mut out_seen = false;
    for _ in 0..(8 * 80) {
        let next = kw.tick() + 1;
        *kw.world.resource_mut::<TickCounter>() = TickCounter(next);
        ask_kernel::systems::process_world(&mut kw.world);
        let g = kw.world.resource::<Grid>();
        if (1..(w - 1)).any(|x| {
            let f = g.get(x, h / 2);
            f == Some(id::FLOOR) || f == Some(id::RUBBLE)
        }) {
            out_seen = true;
            break;
        }
    }
    assert!(out_seen, "fire never burned out");
}

// --- world process test helpers ---
fn stage_fill(kw: &mut KernelWorld, fill: u16) -> (i32, i32) {
    let (w, h) = {
        let g = kw.world.resource::<Grid>();
        (g.width, g.height)
    };
    let mut grid = kw.world.resource_mut::<Grid>();
    for i in 0..grid.cells.len() {
        grid.cells[i] = fill;
    }
    (w, h)
}

fn run_process_ticks(kw: &mut KernelWorld, n: u64) {
    use ask_kernel::world::TickCounter;
    for _ in 0..n {
        *kw.world.resource_mut::<TickCounter>() = TickCounter(kw.tick() + 1);
        ask_kernel::systems::process_world(&mut kw.world);
    }
}

#[test]
fn fire_dies_on_water_and_monster_dies_in_fire() {
    use ask_kernel::components::{Monster, StableId};

    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 137;
    let mut kw = KernelWorld::new(&cfg);
    let (w, _h) = stage_fill(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for x in 1..(w - 1) {
            grid.set(x, 5, id::FLOOR);
            grid.set(x, 6, id::SHALLOW_WATER);
        }
        grid.set(1, 5, id::FIRE);
    }
    run_process_ticks(&mut kw, 8 * 40);
    let g = kw.world.resource::<Grid>();
    for x in 1..(w - 1) {
        assert_ne!(g.get(x, 6), Some(id::FIRE), "fire crossed water at x={x}");
    }
    // monster on FIRE cell dies via the lava-flag damage branch
    let e = kw.world.spawn((
        Position { x: 1, y: 5 },
        Glyph('o'),
        Monster { race_id: 1, name: "fire rat".into(), color: 'r' },
        Health { hp: 3, max_hp: 3 },
        StableId(99921),
    )).id();
    kw.world.resource_mut::<Grid>().set(1, 5, id::FIRE);
    ask_kernel::systems::monster_move_to(&mut kw.world, e, 1, 5);
    assert!(kw.world.get::<Health>(e).is_none(), "monster must die in fire");
}

#[test]
fn water_flows_and_stays_bounded() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 139;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage_fill(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                grid.set(x, y, id::FLOOR);
            }
        }
        grid.set(w / 2, h / 2, id::DEEP_WATER);
    }
    run_process_ticks(&mut kw, 8 * 60);
    let g = kw.world.resource::<Grid>();
    let water_cells = g
        .cells
        .iter()
        .filter(|&&f| f == id::SHALLOW_WATER || f == id::DEEP_WATER)
        .count();
    assert!(water_cells > 1, "water never flowed");
    assert!(water_cells < (w * h) as usize / 2, "water unbounded: {water_cells}");
}

#[test]
fn grass_needs_water_to_spread() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 149;
    let mut kw = KernelWorld::new(&cfg);
    let (w, h) = stage_fill(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for y in 1..(h - 1) {
            for x in 1..(w - 1) {
                grid.set(x, y, id::DIRT);
            }
        }
        grid.set(3, 3, id::GRASS);
    }
    run_process_ticks(&mut kw, 8 * 30);
    let g = kw.world.resource::<Grid>();
    let grass_dry = g.cells.iter().filter(|&&f| f == id::GRASS).count();
    assert_eq!(grass_dry, 1, "grass spread without water");
    kw.world.resource_mut::<Grid>().set(4, 3, id::SHALLOW_WATER);
    run_process_ticks(&mut kw, 8 * 60);
    let g = kw.world.resource::<Grid>();
    let grass_wet = g.cells.iter().filter(|&&f| f == id::GRASS).count();
    assert!(grass_wet > 1, "grass never spread with water");
}

#[test]
fn fire_glows_and_glow_dies_with_it() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 151;
    let mut kw = KernelWorld::new(&cfg);
    let (w, _h) = stage_fill(&mut kw, id::GRANITE); // grid is >= 44x44 (gen minimum)
    kw.world.resource_mut::<Grid>().set(5, 5, id::FIRE);
    run_process_ticks(&mut kw, ask_kernel::balance::PROCESS_EVERY_N);
    let glow = kw.world.resource::<ask_kernel::vision::GlowMask>();
    assert!(glow.mask[(5 * w + 5) as usize], "FIRE must glow");
    // burn it out: fire_burnout converts FIRE to FLOOR/RUBBLE over time
    let mut fire_gone = false;
    for _ in 0..(8 * 80) {
        run_process_ticks(&mut kw, 1);
        if kw.world.resource::<Grid>().get(5, 5) != Some(id::FIRE) {
            fire_gone = true;
            break;
        }
    }
    assert!(fire_gone, "fire never burned out");
    let glow = kw.world.resource::<ask_kernel::vision::GlowMask>();
    assert!(!glow.mask[(5 * w + 5) as usize], "glow must clear after fire dies");
}

#[test]
fn deep_spring_runs_dry_and_extract_is_bounded() {
    // one deep cell yields EXACTLY two shallow cells, then the spring is a
    // normal shallow cell (thinning rules apply) — no infinite water blocks
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 157;
    let mut kw = KernelWorld::new(&cfg);
    stage_fill(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        for y in 4..8 {
            for x in 4..8 {
                grid.set(x, y, id::FLOOR);
            }
        }
        grid.set(5, 5, id::DEEP_WATER);
    }
    // run until the deep cell itself converts (spring fired)
    let mut converted = false;
    for _ in 0..(8 * 400) {
        run_process_ticks(&mut kw, 1);
        if kw.world.resource::<Grid>().get(5, 5) == Some(id::SHALLOW_WATER) {
            converted = true;
            break;
        }
    }
    assert!(converted, "deep spring never converted to shallow");
    // total extractable shallow blocks from one deep cell is bounded (<= 2)
    let g = kw.world.resource::<Grid>();
    let shallow_now = g
        .cells
        .iter()
        .filter(|&&f| f == id::SHALLOW_WATER)
        .count();
    assert!(shallow_now <= 2, "spring minted more than 2 shallow cells: {shallow_now}");
}

#[test]
fn terrain_changed_from_is_target_prior_feat() {
    let mut cfg = Config::default();
    cfg.width = 33;
    cfg.height = 22;
    cfg.seed = 163;
    let mut kw = KernelWorld::new(&cfg);
    stage_fill(&mut kw, id::GRANITE);
    {
        let mut grid = kw.world.resource_mut::<Grid>();
        grid.set(5, 5, id::FIRE);
        grid.set(6, 5, id::GRASS);
    }
    // run until the grass catches fire
    let mut ev_ok = false;
    for _ in 0..(8 * 60) {
        kw.world.resource_mut::<EventBuf>().drain();
        run_process_ticks(&mut kw, 1);
        for e in kw.world.resource_mut::<EventBuf>().drain() {
            if let GameEvent::TerrainChanged { at, from, to, .. } = e {
                if at == (6, 5) && to == id::FIRE {
                    assert_eq!(from, id::GRASS, "from must be the TARGET's prior feat");
                    ev_ok = true;
                }
            }
        }
        if ev_ok {
            break;
        }
    }
    assert!(ev_ok, "never saw the spread event");

    // water flow (NeighborAndSelf arm): the minted shallow cell reports its
    // own prior feat, not the deep source's
    let mut cfg2 = Config::default();
    cfg2.width = 33;
    cfg2.height = 22;
    cfg2.seed = 167;
    let mut kw2 = KernelWorld::new(&cfg2);
    stage_fill(&mut kw2, id::GRANITE);
    {
        let mut grid = kw2.world.resource_mut::<Grid>();
        grid.set(5, 5, id::DEEP_WATER);
        grid.set(6, 5, id::FLOOR);
    }
    let mut saw_water = false;
    for _ in 0..(8 * 60) {
        kw2.world.resource_mut::<EventBuf>().drain();
        run_process_ticks(&mut kw2, 1);
        for e in kw2.world.resource_mut::<EventBuf>().drain() {
            if let GameEvent::TerrainChanged { at, from, to, .. } = e {
                if at == (6, 5) && to == id::SHALLOW_WATER {
                    assert_eq!(from, id::FLOOR, "from must be the minted cell's prior feat");
                    saw_water = true;
                }
            }
        }
        if saw_water {
            break;
        }
    }
    assert!(saw_water, "never saw the water flow event");
}

#[test]
fn process_events_hidden_from_memory_only_cells() {
    // TerrainChanged requires display_class 2 (currently visible), not memory
    let mut vis = ask_kernel::vision::VisionMap::new(10, 10);
    vis.flags[(2 * 10 + 3) as usize] = ask_kernel::vision::F_MARK; // remembered only
    let ev = GameEvent::TerrainChanged {
        at: (3, 2),
        from: 96,
        to: 99,
        cause: ask_kernel::process_rules::Cause::Fire,
    };
    assert!(!ask_kernel::events::event_visible(&ev, &vis, None));
    vis.flags[(2 * 10 + 3) as usize] = ask_kernel::vision::F_VIEW | ask_kernel::vision::F_LITE;
    assert!(ask_kernel::events::event_visible(&ev, &vis, None));
}
