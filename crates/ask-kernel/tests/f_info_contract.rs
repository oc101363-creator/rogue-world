//! f_info id contract — the hardcoded `f_info::id` constants MUST keep
//! matching data/f_info.txt. If someone renumbers the data file, this test
//! fails loudly instead of the whole game silently breaking.

use ask_kernel::f_info::{id, table};

#[test]
fn hardcoded_ids_match_data_file() {
    let t = table();
    let cases: &[(u16, &str)] = &[
        (id::FLOOR, "FLOOR"),
        (id::OPEN_DOOR, "OPEN_DOOR"),
        (id::BROKEN_DOOR, "BROKEN_DOOR"),
        (id::UP_STAIR, "UP_STAIR"),
        (id::DOWN_STAIR, "DOWN_STAIR"),
        (id::CLOSED_DOOR, "CLOSED_DOOR"),
        (id::RUBBLE, "RUBBLE"),
        (id::MAGMA_VEIN, "MAGMA_VEIN"),
        (id::QUARTZ_VEIN, "QUARTZ_VEIN"),
        (id::GRANITE, "GRANITE"),
        (id::PERMANENT, "PERMANENT"),
        (id::DIRT, "DIRT"),
        (id::GRASS, "GRASS"),
        (id::TREE, "TREE"),
        (id::MOUNTAIN, "MOUNTAIN"),
    ];
    for &(feat, want) in cases {
        let info = t.get(feat).unwrap_or_else(|| panic!("id {feat} missing"));
        assert_eq!(
            info.name, want,
            "f_info id {feat} is '{}' in data, code expects '{want}'",
            info.name
        );
    }
}

#[test]
fn semantic_flags_match_constants() {
    let t = table();
    assert!(t.get(id::PERMANENT).unwrap().permanent);
    assert!(t.is_closed_door(id::CLOSED_DOOR));
    assert!(t.is_open_door(id::OPEN_DOOR));
    assert!(t.get(id::TREE).unwrap().tree);
    assert!(t.get(id::SHALLOW_WATER).unwrap().water);
    assert!(t.get(id::DEEP_LAVA).unwrap().lava);
    assert!(t.get(id::MOUNTAIN).unwrap().wall);
    assert!(t.get(id::FLOOR).unwrap().walk);
    // trap constants really are traps
    for &trap in &id::TRAP_FEATS {
        assert!(t.is_trap(trap), "id {trap} expected trap flag");
    }
}

#[test]
fn fire_feat_contract() {
    let t = table();
    let f = t.get(id::FIRE).expect("FIRE feat must exist");
    assert_eq!(f.name, "FIRE");
    assert!(f.walk, "FIRE is enterable");
    assert!(f.lava, "FIRE reuses lava damage branch");
    assert_eq!(f.glyph, '!');
}
