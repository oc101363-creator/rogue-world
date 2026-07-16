use ask_kernel::config::Config;
use ask_kernel::viewer::build_viewer_snapshot;
use ask_kernel::world::KernelWorld;

#[test]
fn snapshot_includes_feat_ids_and_catalog_version() {
    let mut kw = KernelWorld::new(&Config::default());
    let snap = build_viewer_snapshot(&mut kw.world, &[]);
    assert!(snap.catalog_version >= 1);
    assert_eq!(snap.feat_ids.enc, "u16le_b64");
    assert_eq!(snap.feat_ids.w, snap.width);
    assert_eq!(snap.feat_ids.h, snap.height);
    assert!(!snap.feat_ids.data.is_empty());
    assert_eq!(snap.tiles.len() as i32, snap.height);
}

#[test]
fn art_catalog_has_core_feats() {
    let c = ask_kernel::art::catalog();
    assert!(c.feats.contains_key("1"));
    assert_eq!(c.feats.get("56").map(|f| f.material.as_str()), Some("granite"));
}
