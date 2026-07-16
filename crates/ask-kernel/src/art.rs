//! Presentation catalog — pure data.
//!
//! `systems/*` must never import this module for gameplay decisions.
//! Simulation uses FeatId + flags only; this maps identity → look.

use std::collections::HashMap;
use std::sync::OnceLock;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::f_info::{self, FeatInfo};
use crate::k_info;
use crate::r_info;

const OVERLAY_TOML: &str = include_str!("../data/art/fs_hdg_overlay.toml");

#[derive(Clone, Debug, Serialize)]
pub struct FeatLook {
    pub glyph: char,
    pub material: String,
    /// Original frog color letter — keeps 16-color variety when material is generic.
    pub color: char,
    pub layer: u8,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityLook {
    pub glyph: char,
    pub material: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtCatalog {
    pub catalog_version: u32,
    pub materials: HashMap<String, String>,
    /// JSON object keys are stringified feat ids.
    pub feats: HashMap<String, FeatLook>,
    pub races: HashMap<String, EntityLook>,
    pub objects: HashMap<String, EntityLook>,
    pub entity_defaults: HashMap<String, EntityLook>,
}

#[derive(Debug, Deserialize)]
struct OverlayFile {
    catalog_version: u32,
    materials: HashMap<String, String>,
    #[serde(default)]
    feats: HashMap<String, OverlayFeat>,
}

#[derive(Debug, Deserialize)]
struct OverlayFeat {
    glyph: Option<String>,
    material: Option<String>,
    layer: Option<u8>,
}

/// Map frog 16-color letters to distinct materials so the map keeps variety.
fn material_from_color(c: char) -> &'static str {
    match c {
        'D' => "void",
        'd' => "shadow",
        's' => "stone_dark",
        'w' => "floor",
        'W' => "stone_light",
        'g' | 'G' | 'L' => "plant",
        'b' | 'B' => "water",
        'r' | 'R' => "blood",
        'o' => "fire",
        'y' => "gold",
        'u' => "earth",
        'U' => "wood",
        'v' => "magic",
        // frog sometimes emits non-standard letters (seen as 'h' in wild maps)
        'h' | 'H' => "crystal",
        _ => "basalt",
    }
}

fn baseline_material(info: &FeatInfo) -> &'static str {
    // Semantic flags win first — these must read instantly.
    if info.lava {
        return "magma";
    }
    if info.water {
        return "aquifer";
    }
    if info.tree {
        return "plant";
    }
    if info.door {
        return "door";
    }
    if info.trap {
        return "trap";
    }
    if info.wall {
        // walls keep stone family but follow frog shade when possible
        return match info.color {
            's' | 'd' | 'D' => "stone_dark",
            'w' | 'W' => "granite",
            'u' | 'U' => "earth",
            'g' | 'G' => "plant",
            _ => "granite",
        };
    }

    let up = info.name.to_ascii_uppercase();
    if up.contains("FLOWER") {
        return "flower";
    }
    if up.contains("BRAKE") || up.contains("BUSH") {
        return "brake";
    }
    if up.contains("GOLD") || up.contains("TREASURE") || up.contains("VEIN") {
        return "gold";
    }
    if up.contains("DIRT") || up.contains("SOIL") || up.contains("MUD") {
        return "earth";
    }
    if up.contains("GRASS") {
        return "plant";
    }
    if up.contains("FLOOR") || up.contains("INVIS") || up.contains("OPEN") {
        // floors: keep shade from frog letter for variety
        return material_from_color(info.color);
    }

    // Everything else: preserve frog color diversity instead of collapsing to basalt.
    material_from_color(info.color)
}

fn build_catalog() -> ArtCatalog {
    let overlay: OverlayFile = toml::from_str(OVERLAY_TOML).expect("parse art overlay");
    let mut materials = overlay.materials;

    // Full FS-HDG + frog-letter material set (richer than 6 buckets).
    for (k, v) in [
        ("void", "#000000"),
        ("shadow", "#2a2a2a"),
        ("stone_dark", "#6e6e6e"),
        ("floor", "#8b93a0"),
        ("stone_light", "#c5cdd6"),
        ("granite", "#a8b0b8"),
        ("crystal", "#9ad0ff"),
        ("plant", "#3dcc5a"),
        ("organic", "#7a9a45"),
        ("earth", "#a67c52"),
        ("wood", "#c4a574"),
        ("door", "#d2b48c"),
        ("aquifer", "#2f7bff"),
        ("water", "#3d9eff"),
        ("water_deep", "#1a4fff"),
        ("magma", "#ff4500"),
        ("fire", "#ff8c1a"),
        ("blood", "#e03535"),
        ("gold", "#ffd000"),
        ("magic", "#c44cff"),
        ("trap", "#ff6666"),
        ("flower", "#e89ad4"),
        ("brake", "#6b8f3c"),
        ("basalt", "#707884"),
        ("ui_primary", "#00ff66"),
        ("ui_warning", "#ffcc00"),
        ("ui_danger", "#ff3333"),
        ("ui_info", "#00e5ff"),
        ("text_white", "#ffffff"),
        ("depth_shadow", "#2a2a2a"),
    ] {
        materials.entry(k.into()).or_insert_with(|| v.into());
    }

    let mut feats = HashMap::new();
    let table = f_info::table();
    for id in 0..=table.max_id() {
        let Some(info) = table.get(id) else {
            continue;
        };
        let mut look = FeatLook {
            glyph: info.glyph,
            material: baseline_material(info).into(),
            color: info.color,
            layer: 0,
            name: info.name.clone(),
        };
        if let Some(ov) = overlay.feats.get(&id.to_string()) {
            if let Some(g) = &ov.glyph {
                look.glyph = g.chars().next().unwrap_or(look.glyph);
            }
            if let Some(m) = &ov.material {
                look.material = m.clone();
            }
            if let Some(l) = ov.layer {
                look.layer = l;
            }
        }
        feats.insert(id.to_string(), look);
    }

    let mut races = HashMap::new();
    for (id, race) in r_info::table().iter() {
        races.insert(
            id.to_string(),
            EntityLook {
                glyph: race.glyph,
                material: "ui_danger".into(),
                name: Some(race.name.clone()),
            },
        );
    }

    let mut objects = HashMap::new();
    for (id, obj) in k_info::table().iter() {
        objects.insert(
            id.to_string(),
            EntityLook {
                glyph: obj.glyph,
                material: "ui_info".into(),
                name: Some(obj.name.clone()),
            },
        );
    }

    let mut entity_defaults = HashMap::new();
    entity_defaults.insert(
        "agent".into(),
        EntityLook {
            glyph: '@',
            material: "ui_warning".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "tree".into(),
        EntityLook {
            glyph: '♣',
            material: "plant".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "iron".into(),
        EntityLook {
            glyph: 'I',
            material: "stone_light".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "hut".into(),
        EntityLook {
            glyph: '⌂',
            material: "ui_warning".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "monster".into(),
        EntityLook {
            glyph: 'o',
            material: "ui_danger".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "item".into(),
        EntityLook {
            glyph: '!',
            material: "ui_info".into(),
            name: None,
        },
    );

    ArtCatalog {
        catalog_version: overlay.catalog_version,
        materials,
        feats,
        races,
        objects,
        entity_defaults,
    }
}

pub fn catalog() -> &'static ArtCatalog {
    static C: OnceLock<ArtCatalog> = OnceLock::new();
    C.get_or_init(build_catalog)
}

/// Row-major u16 LE → standard base64.
pub fn encode_feat_ids_b64(cells: &[u16]) -> String {
    let mut bytes = Vec::with_capacity(cells.len() * 2);
    for &c in cells {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_floor_and_granite() {
        let c = catalog();
        let floor = c.feats.get("1").expect("FLOOR id 1");
        assert_eq!(floor.glyph, '.');
        assert_eq!(floor.material, "floor");
        let gran = c.feats.get("56").expect("GRANITE id 56");
        assert_eq!(gran.material, "granite");
    }

    #[test]
    fn overlay_tree_and_doors_keep_variety() {
        let c = catalog();
        let tree = c.feats.get("96").expect("TREE");
        assert_eq!(tree.glyph, '♣');
        assert_eq!(tree.material, "plant");
        let door = c.feats.get("32").expect("CLOSED_DOOR");
        assert_eq!(door.material, "door");
        let gold = c.feats.get("51").expect("QUARTZ_VEIN");
        assert_eq!(gold.material, "gold");
    }

    #[test]
    fn encode_roundtrip_len() {
        let cells = vec![1u16, 56, 83, 0];
        let b64 = encode_feat_ids_b64(&cells);
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .unwrap();
        assert_eq!(raw.len(), cells.len() * 2);
    }

    #[test]
    fn systems_do_not_import_art() {
        let systems = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/systems");
        for entry in std::fs::read_dir(systems).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().and_then(|s| s.to_str()) == Some("rs") {
                let s = std::fs::read_to_string(&p).unwrap();
                assert!(
                    !s.contains("crate::art") && !s.contains("use crate::art"),
                    "{} must not import art",
                    p.display()
                );
            }
        }
    }
}
