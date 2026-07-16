//! Save format v2: full world roundtrip + v1 compatibility.

use ask_kernel::components::{Agent, AgentProfile, Item, Matter, Monster, Position, StableId};
use ask_kernel::config::Config;
use ask_kernel::persist;
use ask_kernel::world::{Depth, KernelWorld, WorldSeed};
use bevy_ecs::prelude::With;

fn small_cfg(seed: u64) -> Config {
    let mut cfg = Config::default();
    cfg.width = 88;
    cfg.height = 66;
    cfg.seed = seed;
    cfg
}

#[test]
fn v2_roundtrip_keeps_monsters_items_identity_depth_seed() {
    let mut kw = KernelWorld::new(&small_cfg(21));
    let (sid2, _, _) = kw.spawn_agent("Archivist".into(), "remember".into()).expect("spawn");
    // hand the registered agent a pack + a message-free mailbox (mailbox empty here)
    {
        let e = {
            let mut q = kw
                .world
                .query_filtered::<(bevy_ecs::prelude::Entity, &StableId), With<Agent>>();
            q.iter(&kw.world)
                .find(|(_, s)| s.0 == sid2)
                .map(|(e, _)| e)
                .unwrap()
        };
        kw.world
            .get_mut::<ask_kernel::components::Inventory>(e)
            .unwrap()
            .add(Matter::Terrain { feat: 56 }, 3);
    }
    let monsters_before = {
        let mut q = kw.world.query::<&Monster>();
        q.iter(&kw.world).count()
    };
    let items_before = {
        let mut q = kw.world.query::<&Item>();
        q.iter(&kw.world).count()
    };

    let snap = persist::capture(&mut kw.world);
    assert_eq!(snap.version, 2);
    let json = serde_json::to_string(&snap).unwrap();
    let mut restored = persist::restore(serde_json::from_str(&json).unwrap());

    // monsters + items survived
    let monsters_after = {
        let mut q = restored.world.query::<&Monster>();
        q.iter(&restored.world).count()
    };
    let items_after = {
        let mut q = restored.world.query::<&Item>();
        q.iter(&restored.world).count()
    };
    assert_eq!(monsters_before, monsters_after, "monsters lost in roundtrip");
    assert_eq!(items_before, items_after, "items lost in roundtrip");

    // registered agent kept profile + pack
    let mut found = false;
    {
        let mut q = restored.world.query_filtered::<
            (&StableId, &Position, Option<&AgentProfile>, &ask_kernel::components::Inventory),
            With<Agent>,
        >();
        for (sid, _, pr, inv) in q.iter(&restored.world) {
            if sid.0 == sid2 {
                found = true;
                assert_eq!(pr.map(|p| p.name.as_str()), Some("Archivist"));
                assert_eq!(inv.qty_terrain(56), 3, "pack terrain stack lost");
            }
        }
    }
    assert!(found, "registered agent missing after roundtrip");

    // depth/seed carried
    assert_eq!(restored.world.resource::<Depth>().0, 0);
    assert_eq!(restored.world.resource::<WorldSeed>().0, 21);
    // glow mask rebuilt to full size
    let glow = restored.world.resource::<ask_kernel::vision::GlowMask>();
    assert_eq!(glow.mask.len(), (88 * 66) as usize);
}

#[test]
fn v1_save_without_version_still_loads() {
    // minimal v1-shaped document: no version/depth/seed/glow, no v2 entity kinds
    let v1 = serde_json::json!({
        "tick": 7,
        "width": 16,
        "height": 12,
        "hut_wood_cost": 3,
        "id_counter": 3,
        "cells": vec![1u16; 16 * 12],
        "entities": [
            { "kind": "agent", "id": 1, "x": 2, "y": 2, "wood": 4, "iron": 1 },
            { "kind": "tree", "id": 2, "x": 5, "y": 5, "amount": 3 }
        ]
    });
    let mut kw = persist::restore(serde_json::from_value(v1).unwrap());
    // legacy wood/iron folded into pack
    let mut wood = 0;
    {
        let mut q = kw
            .world
            .query_filtered::<&ask_kernel::components::Inventory, With<Agent>>();
        for inv in q.iter(&kw.world) {
            wood += inv.wood();
        }
    }
    assert_eq!(wood, 4);
    assert_eq!(kw.world.resource::<Depth>().0, 0);
    assert_eq!(kw.tick(), 7);
}
