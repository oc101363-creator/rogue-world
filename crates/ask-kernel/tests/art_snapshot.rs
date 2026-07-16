use ask_kernel::config::Config;
use ask_kernel::viewer::{build_viewer_snapshot, build_viewer_snapshot_with};
use ask_kernel::vision::VisionMap;
use ask_kernel::world::KernelWorld;

fn decode_feats(snap: &ask_kernel::viewer::ViewerSnapshot) -> Vec<u16> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(&snap.feat_ids.data)
        .expect("b64");
    raw.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect()
}

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

/// FOV security: feat_ids must NOT leak terrain the viewer cannot see.
/// A fully-dark vision map ⇒ every cell masked to feat 0 (NONE).
#[test]
fn feat_ids_masked_by_vision() {
    let mut kw = KernelWorld::new(&Config::default());
    let (w, h) = {
        let g = kw.world.resource::<ask_kernel::grid::Grid>();
        (g.width, g.height)
    };
    let dark = VisionMap::new(w, h);
    let snap = build_viewer_snapshot_with(&mut kw.world, &[], &dark, Some(&[]), None, false);
    let feats = decode_feats(&snap);
    assert_eq!(feats.len(), (w * h) as usize);
    assert!(
        feats.iter().all(|&f| f == 0),
        "dark vision must mask every feat id to 0"
    );
}

/// Inverse: an all-visible vision map ⇒ feat ids match the real grid.
#[test]
fn feat_ids_unmasked_when_visible() {
    let mut kw = KernelWorld::new(&Config::default());
    let (w, h, real) = {
        let g = kw.world.resource::<ask_kernel::grid::Grid>();
        (g.width, g.height, g.cells.clone())
    };
    let mut vis = VisionMap::new(w, h);
    for f in vis.flags.iter_mut() {
        *f = ask_kernel::vision::F_VIEW | ask_kernel::vision::F_LITE;
    }
    let snap = build_viewer_snapshot_with(&mut kw.world, &[], &vis, Some(&[]), None, false);
    assert_eq!(decode_feats(&snap), real);
}
