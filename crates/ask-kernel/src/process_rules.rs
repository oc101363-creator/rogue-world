//! World process rules — PURE DATA, zero ECS (same philosophy as sandbox.rs).
//!
//! 只写简单规则，不写手工结局。Gameplay (torch posts, moats, fire belts,
//! farms) is emergent from these rows, never hand-authored features.

use crate::f_info::{FeatId, FeatInfo};

#[derive(Clone, Copy, Debug)]
pub enum CellCond {
    FeatIs(FeatId),
    ShallowWater,
    Grass,
}

#[derive(Clone, Copy, Debug)]
pub enum NeighborCond {
    None,
    /// Any 4-neighbor that burns: tree/brake/door flags.
    Flammable,
    /// Any 4-neighbor equal to this feat.
    AnyFeat(FeatId),
    /// Water may flow here: walk && !water && !door.
    FlowTarget,
    /// Any 4-neighbor DIRT, with water within Manhattan distance 3.
    DirtWithWaterNear,
}

#[derive(Clone, Copy, Debug)]
pub enum ProcessAction {
    NeighborBecomes(FeatId),
    SelfBecomes(FeatId),
    SelfBecomesOneOf(&'static [(FeatId, u8)]),
    NeighborAndSelf {
        neighbor: FeatId,
        self_becomes: Option<(FeatId, u8)>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Fire,
    Water,
    Growth,
}

pub struct ProcessRule {
    pub name: &'static str,
    pub on: CellCond,
    pub neighbors: NeighborCond,
    pub action: ProcessAction,
    pub chance_pct: u8,
    pub cause: Cause,
}

/// Burns: any plant or woodwork (flags, never an id list).
/// NOTE: brake and grass have no parsed flag in f_info.rs, so they are
/// matched by id (both are frog plants — see their F:PLANT flag).
pub fn is_flammable(f: &FeatInfo) -> bool {
    f.tree || f.id == id::BRAKE || f.id == id::GRASS || f.door
}

/// Water may flow into this: open, not already water, not a door.
pub fn is_flow_target(f: &FeatInfo) -> bool {
    f.walk && !f.water && !f.door
}

use crate::balance as b;
use crate::f_info::id;

/// THE world processes. Order matters: fire, water, grass; first rule to
/// transform a cell wins that tick.
pub fn rules() -> &'static [ProcessRule] {
    &[
        ProcessRule {
            name: "fire_spread",
            on: CellCond::FeatIs(id::FIRE),
            neighbors: NeighborCond::Flammable,
            action: ProcessAction::NeighborBecomes(id::FIRE),
            chance_pct: b::FIRE_SPREAD_PCT,
            cause: Cause::Fire,
        },
        ProcessRule {
            name: "fire_burnout",
            on: CellCond::FeatIs(id::FIRE),
            neighbors: NeighborCond::None,
            action: ProcessAction::SelfBecomesOneOf(&[(id::FLOOR, 90), (id::RUBBLE, 10)]),
            chance_pct: b::FIRE_BURNOUT_PCT,
            cause: Cause::Fire,
        },
        ProcessRule {
            name: "fire_douse",
            on: CellCond::FeatIs(id::SHALLOW_WATER),
            neighbors: NeighborCond::AnyFeat(id::FIRE),
            action: ProcessAction::NeighborBecomes(id::FLOOR),
            chance_pct: 100,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_evaporate",
            on: CellCond::ShallowWater,
            neighbors: NeighborCond::AnyFeat(id::FIRE),
            action: ProcessAction::SelfBecomes(id::FLOOR),
            chance_pct: b::WATER_EVAPORATE_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_flow_deep",
            on: CellCond::FeatIs(id::DEEP_WATER),
            neighbors: NeighborCond::FlowTarget,
            action: ProcessAction::NeighborBecomes(id::SHALLOW_WATER),
            chance_pct: b::WATER_FLOW_DEEP_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "water_flow_shallow",
            on: CellCond::ShallowWater,
            neighbors: NeighborCond::FlowTarget,
            action: ProcessAction::NeighborAndSelf {
                neighbor: id::SHALLOW_WATER,
                self_becomes: Some((id::FLOOR, 100)),
            },
            chance_pct: b::WATER_FLOW_SHALLOW_PCT,
            cause: Cause::Water,
        },
        ProcessRule {
            name: "grass_spread",
            on: CellCond::Grass,
            neighbors: NeighborCond::DirtWithWaterNear,
            action: ProcessAction::NeighborBecomes(id::GRASS),
            chance_pct: b::GRASS_SPREAD_PCT,
            cause: Cause::Growth,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f_info;

    #[test]
    fn table_is_sane() {
        let t = f_info::table();
        let rs = rules();
        assert_eq!(rs.len(), 7);
        for r in rs {
            if let CellCond::FeatIs(f) = r.on {
                assert!(t.get(f).is_some(), "{}: bad feat", r.name);
            }
            match r.action {
                ProcessAction::NeighborBecomes(f) | ProcessAction::SelfBecomes(f) => {
                    assert!(t.get(f).is_some(), "{}: bad action feat", r.name)
                }
                ProcessAction::SelfBecomesOneOf(list) => {
                    for (f, _) in list {
                        assert!(t.get(*f).is_some());
                    }
                }
                ProcessAction::NeighborAndSelf { neighbor, self_becomes } => {
                    assert!(t.get(neighbor).is_some());
                    if let Some((f, _)) = self_becomes {
                        assert!(t.get(f).is_some());
                    }
                }
            }
        }
        let names: std::collections::HashSet<_> = rs.iter().map(|r| r.name).collect();
        assert_eq!(names.len(), 7, "rule names unique");
    }
}
