//! Architecture guard rails — layering rules enforced as tests.
//!
//! The dependency direction is one-way:
//!   data tables (f/k/r_info) ← components ← rules (sandbox/systems)
//!     ← projections (describe/viewer/agent_view/inspect) ← serve
//! A violation fails CI before the cycle gets load-bearing. (This file
//! generalizes the older systems-must-not-import-art test.)

use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            out.extend(rs_files(&p));
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).expect("read source")
}

fn assert_clean(p: &Path, forbidden: &[&str]) {
    let s = read(p);
    for f in forbidden {
        assert!(
            !s.contains(f),
            "{} must not reference {f}",
            p.display()
        );
    }
}

#[test]
fn components_depends_on_nothing_above() {
    assert_clean(
        &manifest_dir().join("src/components.rs"),
        &[
            "crate::sandbox",
            "crate::systems",
            "crate::serve",
            "crate::viewer",
            "crate::agent_view",
            "crate::describe",
            "crate::player",
        ],
    );
}

#[test]
fn sandbox_is_a_rules_leaf() {
    assert_clean(
        &manifest_dir().join("src/sandbox.rs"),
        &["crate::systems", "crate::serve", "crate::viewer", "crate::agent_view"],
    );
}

#[test]
fn systems_never_depend_on_presentation_or_api() {
    let dir = manifest_dir().join("src/systems");
    for p in rs_files(&dir) {
        assert_clean(
            &p,
            &[
                "crate::art",
                "crate::serve",
                "crate::viewer",
                "crate::agent_view",
                "crate::describe",
            ],
        );
    }
}

#[test]
fn describe_is_a_pure_projection() {
    assert_clean(
        &manifest_dir().join("src/describe.rs"),
        &["crate::systems", "crate::serve", "crate::viewer", "crate::agent_view"],
    );
}

#[test]
fn projections_only_touch_systems_discovery() {
    // viewer/agent_view may call discovery (interact) but never mutating systems.
    for name in ["src/viewer.rs", "src/agent_view.rs"] {
        let p = manifest_dir().join(name);
        let s = read(&p);
        for (start, _) in s.match_indices("systems::") {
            let tail: String = s[start..].chars().take(40).collect();
            assert!(
                tail.starts_with("systems::interact"),
                "{name} references forbidden systems path: {tail}"
            );
        }
    }
}

#[test]
fn no_stub_modules_left_wired() {
    // gateway.rs was a dead stub; if it ever comes back it must be real.
    let dir = manifest_dir().join("src");
    for p in rs_files(&dir) {
        assert_clean(&p, &["crate::gateway"]);
    }
}
