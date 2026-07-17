//! Sandbox rules — table-driven matter transforms.
//!
//! Design: few verbs (dig/scoop/place/craft/plant/deconstruct) × rich Matter
//! tables = combinatorial world edits without new Action variants per feature.

use crate::components::{Inventory, Matter, ResourceKind};
use crate::f_info::{self, id, FeatId};

/// Dig/scoop: what happens when removing `feat` from the grid.
#[derive(Clone, Copy, Debug)]
pub struct ExtractRule {
    pub leave: FeatId,
    /// Extra matter granted beyond the removed feat itself.
    pub bonus_iron: u32,
}

/// Soft surfaces you scoop (pack the feat, leave something underneath).
pub fn scoop_rule(feat: FeatId) -> Option<ExtractRule> {
    if feat == id::PERMANENT {
        return None;
    }
    let table = f_info::table();
    let info = table.get(feat)?;
    if info.permanent || info.stairs {
        return None;
    }
    // doors scooped as blocks
    if table.is_closed_door(feat) || table.is_open_door(feat) {
        return Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        });
    }
    match feat {
        id::FLOOR => Some(ExtractRule {
            leave: id::DIRT,
            bonus_iron: 0,
        }),
        id::DIRT => Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        }),
        id::GRASS | id::BRAKE => Some(ExtractRule {
            leave: id::DIRT,
            bonus_iron: 0,
        }),
        id::TREE => Some(ExtractRule {
            leave: id::DIRT,
            bonus_iron: 0,
        }),
        id::SHALLOW_WATER | id::DEEP_WATER => Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        }),
        id::SHALLOW_LAVA | id::DEEP_LAVA => Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        }),
        id::DARK_PIT => Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        }),
        id::OPEN_DOOR | id::BROKEN_DOOR | id::CLOSED_DOOR | id::SECRET_DOOR => Some(ExtractRule {
            leave: id::FLOOR,
            bonus_iron: 0,
        }),
        // NOTE: no walkable catch-all — every scoopable surface is named
        // above. A generic "any soft ground" rule made matter semantics
        // untracked and fed the place-displacement printing loop.
        _ => None,
    }
}

/// Hard rock dig (may take two steps: rock→rubble→floor).
pub fn dig_rule(feat: FeatId) -> Option<ExtractRule> {
    if feat == id::PERMANENT {
        return None;
    }
    let table = f_info::table();
    if table.get(feat).map(|f| f.permanent).unwrap_or(false) {
        return None;
    }
    let hard = matches!(
        feat,
        id::RUBBLE
            | id::MAGMA_VEIN
            | id::QUARTZ_VEIN
            | id::MAGMA_TREASURE
            | id::QUARTZ_TREASURE
            | id::GRANITE
            | id::GRANITE_INNER
            | id::GRANITE_OUTER
            | id::GRANITE_SOLID
            | id::MOUNTAIN
    ) || table
        .get(feat)
        .map(|f| f.wall && !f.permanent)
        .unwrap_or(false);

    if !hard {
        return None;
    }

    let leave = match feat {
        id::RUBBLE => id::FLOOR,
        id::MAGMA_TREASURE | id::QUARTZ_TREASURE => id::FLOOR,
        id::MOUNTAIN => id::RUBBLE,
        _ => id::RUBBLE,
    };
    let bonus_iron = matches!(feat, id::MAGMA_TREASURE | id::QUARTZ_TREASURE) as u32;
    Some(ExtractRule { leave, bonus_iron })
}

pub fn is_diggable(feat: FeatId) -> bool {
    dig_rule(feat).is_some()
}

pub fn is_scoopable(feat: FeatId) -> bool {
    scoop_rule(feat).is_some()
}

/// Can we write `placing` onto current `cur` cell?
pub fn can_place_on(cur: FeatId, placing: FeatId, underfoot: bool) -> Result<(), &'static str> {
    let table = f_info::table();
    if cur == id::PERMANENT || table.get(cur).map(|f| f.permanent).unwrap_or(false) {
        return Err("permanent");
    }
    if table.get(cur).map(|f| f.stairs).unwrap_or(false) {
        return Err("stairs");
    }
    if table.is_closed_door(cur) || table.is_open_door(cur) {
        return Err("door");
    }
    // place onto walkable soft, rubble, or diggable rock (overwrite)
    let ok = table.walk(cur)
        || cur == id::RUBBLE
        || is_diggable(cur)
        || is_scoopable(cur)
        || cur == id::DARK_PIT;
    if !ok {
        return Err("not_placeable");
    }
    if underfoot && !table.walk(placing) {
        return Err("cannot_wall_self");
    }
    // lava underfoot hurts later via terrain; allow place
    Ok(())
}

// ----- Craft recipes (matter → matter), pure data -----

#[derive(Clone, Debug)]
pub struct Recipe {
    pub id: &'static str,
    /// Product name WITHOUT counts (the label's counts come from `needs`,
    /// so the two can never drift apart).
    pub name: &'static str,
    /// (matcher kind tag, count) — see RecipeNeed
    pub needs: &'static [RecipeNeed],
    pub output: RecipeOut,
}

impl Recipe {
    /// Display label, e.g. "craft closed door (2 wood)" — generated.
    pub fn label(&self) -> String {
        let needs = self
            .needs
            .iter()
            .map(need_label)
            .collect::<Vec<_>>()
            .join(" + ");
        format!("craft {} ({})", self.name, needs)
    }
}

pub fn need_label(need: &RecipeNeed) -> String {
    match *need {
        RecipeNeed::Wood(n) => format!("{n} wood"),
        RecipeNeed::Iron(n) => format!("{n} iron"),
        RecipeNeed::Terrain(f, n) => {
            let name = f_info::table()
                .get(f)
                .map(|i| i.name.to_lowercase())
                .unwrap_or_else(|| "terrain".into());
            format!("{n} {name}")
        }
        RecipeNeed::AnyRock(n) => format!("{n} rock"),
        RecipeNeed::AnyTerrain(n) => format!("{n} terrain"),
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RecipeNeed {
    Wood(u32),
    Iron(u32),
    Terrain(FeatId, u32),
    /// Any terrain wall-ish (granite family)
    AnyRock(u32),
    AnyTerrain(u32),
}

#[derive(Clone, Copy, Debug)]
pub enum RecipeOut {
    Terrain(FeatId),
    Resource(ResourceKind, u32),
    /// Multiple outputs
    Multi(&'static [RecipeOut]),
}

pub fn recipes() -> &'static [Recipe] {
    &[
        Recipe {
            id: "plank_door",
            name: "closed door",
            needs: &[RecipeNeed::Wood(2)],
            output: RecipeOut::Terrain(id::CLOSED_DOOR),
        },
        Recipe {
            id: "open_door",
            name: "open door",
            needs: &[RecipeNeed::Wood(2)],
            output: RecipeOut::Terrain(id::OPEN_DOOR),
        },
        Recipe {
            id: "sapling",
            name: "tree block",
            needs: &[RecipeNeed::Wood(1)],
            output: RecipeOut::Terrain(id::TREE),
        },
        Recipe {
            id: "grass_seed",
            name: "grass",
            needs: &[RecipeNeed::Terrain(id::DIRT, 2)],
            output: RecipeOut::Terrain(id::GRASS),
        },
        Recipe {
            id: "compact_rock",
            name: "granite",
            needs: &[RecipeNeed::Terrain(id::RUBBLE, 2)],
            output: RecipeOut::Terrain(id::GRANITE),
        },
        Recipe {
            id: "crush_rock",
            name: "rubble",
            needs: &[RecipeNeed::AnyRock(1)],
            output: RecipeOut::Terrain(id::RUBBLE),
        },
        Recipe {
            id: "ore_vein",
            name: "ore vein",
            needs: &[RecipeNeed::Iron(1), RecipeNeed::Terrain(id::GRANITE, 1)],
            output: RecipeOut::Terrain(id::MAGMA_TREASURE),
        },
        Recipe {
            id: "quartz_vein",
            name: "quartz vein",
            needs: &[RecipeNeed::Iron(1), RecipeNeed::Terrain(id::RUBBLE, 1)],
            output: RecipeOut::Terrain(id::QUARTZ_VEIN),
        },
        Recipe {
            id: "shallow_pool",
            name: "shallow water",
            needs: &[RecipeNeed::Terrain(id::DEEP_WATER, 1)],
            output: RecipeOut::Terrain(id::SHALLOW_WATER),
        },
        Recipe {
            id: "deep_pool",
            name: "deep water",
            needs: &[RecipeNeed::Terrain(id::SHALLOW_WATER, 2)],
            output: RecipeOut::Terrain(id::DEEP_WATER),
        },
        Recipe {
            id: "fill_floor",
            name: "floor tile",
            needs: &[RecipeNeed::Terrain(id::DIRT, 1)],
            output: RecipeOut::Terrain(id::FLOOR),
        },
        Recipe {
            id: "mountain",
            name: "mountain",
            needs: &[RecipeNeed::Terrain(id::GRANITE, 3)],
            output: RecipeOut::Terrain(id::MOUNTAIN),
        },
        Recipe {
            id: "smelt_iron",
            name: "iron",
            needs: &[RecipeNeed::Terrain(id::MAGMA_TREASURE, 1)],
            output: RecipeOut::Resource(ResourceKind::Iron, 2),
        },
        Recipe {
            id: "chop_wood",
            name: "wood",
            needs: &[RecipeNeed::Terrain(id::TREE, 1)],
            output: RecipeOut::Resource(ResourceKind::Wood, 2),
        },
        Recipe {
            id: "lava_cool",
            name: "rubble",
            needs: &[RecipeNeed::Terrain(id::SHALLOW_LAVA, 1)],
            output: RecipeOut::Terrain(id::RUBBLE),
        },
        Recipe {
            id: "dirt_from_rubble",
            name: "dirt",
            needs: &[RecipeNeed::Terrain(id::RUBBLE, 1)],
            output: RecipeOut::Terrain(id::DIRT),
        },
    ]
}

pub fn recipe_by_id(id: &str) -> Option<&'static Recipe> {
    recipes().iter().find(|r| r.id == id)
}

/// Count matching matter in pack for a need.
pub fn count_need(slots: &[(Matter, u32)], need: &RecipeNeed) -> u32 {
    match *need {
        RecipeNeed::Wood(_) => slots
            .iter()
            .filter_map(|(m, q)| match m {
                Matter::Resource {
                    resource: ResourceKind::Wood,
                } => Some(*q),
                _ => None,
            })
            .sum(),
        RecipeNeed::Iron(_) => slots
            .iter()
            .filter_map(|(m, q)| match m {
                Matter::Resource {
                    resource: ResourceKind::Iron,
                } => Some(*q),
                _ => None,
            })
            .sum(),
        RecipeNeed::Terrain(feat, _) => slots
            .iter()
            .filter_map(|(m, q)| match m {
                Matter::Terrain { feat: f } if *f == feat => Some(*q),
                _ => None,
            })
            .sum(),
        RecipeNeed::AnyRock(_) => slots
            .iter()
            .filter_map(|(m, q)| match m {
                Matter::Terrain { feat: f } if is_rock_feat(*f) => Some(*q),
                _ => None,
            })
            .sum(),
        RecipeNeed::AnyTerrain(_) => slots
            .iter()
            .filter_map(|(m, q)| match m {
                Matter::Terrain { .. } => Some(*q),
                _ => None,
            })
            .sum(),
    }
}

pub fn need_required(need: &RecipeNeed) -> u32 {
    match *need {
        RecipeNeed::Wood(n)
        | RecipeNeed::Iron(n)
        | RecipeNeed::Terrain(_, n)
        | RecipeNeed::AnyRock(n)
        | RecipeNeed::AnyTerrain(n) => n,
    }
}

pub fn can_craft(slots: &[(Matter, u32)], recipe: &Recipe) -> bool {
    recipe
        .needs
        .iter()
        .all(|n| count_need(slots, n) >= need_required(n))
}

/// Flatten inventory to (matter, qty) for recipe checks.
pub fn pack_view(slots: &[crate::components::Stack]) -> Vec<(Matter, u32)> {
    slots.iter().map(|s| (s.matter.clone(), s.qty)).collect()
}

/// Remove qty of any rock-family terrain from a pack.
/// Lives here (not on Inventory) so components never depends on rules.
pub fn remove_any_rock(inv: &mut Inventory, qty: u32) -> bool {
    inv.remove_matching(
        |m| matches!(m, Matter::Terrain { feat } if is_rock_feat(*feat)),
        qty,
    )
}

pub fn is_rock_feat(feat: FeatId) -> bool {
    matches!(
        feat,
        id::GRANITE
            | id::GRANITE_INNER
            | id::GRANITE_OUTER
            | id::GRANITE_SOLID
            | id::MOUNTAIN
            | id::MAGMA_VEIN
            | id::QUARTZ_VEIN
    )
}

/// Outputs as concrete Matter list.
pub fn expand_output(out: &RecipeOut) -> Vec<(Matter, u32)> {
    match *out {
        RecipeOut::Terrain(f) => vec![(Matter::Terrain { feat: f }, 1)],
        RecipeOut::Resource(k, n) => vec![(Matter::Resource { resource: k }, n)],
        RecipeOut::Multi(list) => list.iter().flat_map(expand_output).collect(),
    }
}
