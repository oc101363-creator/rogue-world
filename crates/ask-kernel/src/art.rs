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
    /// JSON object keys will be stringified feat ids.
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

fn baseline_material(info: &FeatInfo) -> &'static str {
    let up = info.name.to_ascii_uppercase();
    if info.lava || up.contains("LAVA") {
        "magma"
    } else if info.water || up.contains("WATER") {
        "aquifer"
    } else if info.tree || up.contains("TREE") {
        "organic"
    } else if up.contains("GOLD") || up.contains("TREASURE") || up.contains("VEIN") {
        "gold"
    } else if info.wall
        || up.contains("GRANITE")
        || up.contains("QUARTZ")
        || up.contains("RUBBLE")
        || up.contains("WALL")
        || up.contains("MOUNTAIN")
    {
        "granite"
    } else if up.contains("DIRT") || up.contains("GRASS") || up.contains("SOIL") || up.contains("MUD")
    {
        "organic"
    } else {
        "basalt"
    }
}

fn build_catalog() -> ArtCatalog {
    let overlay: OverlayFile = toml::from_str(OVERLAY_TOML).expect("parse art overlay");
    let mut materials = overlay.materials;
    for (k, v) in [
        ("basalt", "#555555"),
        ("granite", "#AAAAAA"),
        ("gold", "#FFD700"),
        ("aquifer", "#0055FF"),
        ("magma", "#FF4500"),
        ("organic", "#8B5A2B"),
        ("void", "#000000"),
        ("ui_primary", "#00FF66"),
        ("ui_warning", "#FFCC00"),
        ("ui_danger", "#FF3333"),
        ("ui_info", "#00E5FF"),
        ("text_white", "#FFFFFF"),
        ("depth_shadow", "#2A2A2A"),
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
            material: "organic".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "iron".into(),
        EntityLook {
            glyph: 'I',
            material: "granite".into(),
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
        assert!(!floor.material.is_empty());
        let gran = c.feats.get("56").expect("GRANITE id 56");
        assert_eq!(gran.material, "granite");
    }

    #[test]
    fn overlay_tree_glyph() {
        let c = catalog();
        let tree = c.feats.get("96").expect("TREE");
        assert_eq!(tree.glyph, '♣');
        assert_eq!(tree.material, "organic");
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
