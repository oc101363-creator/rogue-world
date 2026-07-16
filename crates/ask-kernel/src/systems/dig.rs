//! Dig / scoop / place — terrain ↔ pack via sandbox rules.

use bevy_ecs::prelude::*;

use crate::components::{Inventory, Matter, Position, ResourceKind};
use crate::events::{EventBuf, GameEvent};
use crate::grid::Grid;
use crate::sandbox::{self, can_place_on};
use crate::systems::stable_id;

pub use crate::sandbox::{is_diggable, is_scoopable};

fn apply_extract(world: &mut World, agent: Entity, dx: i32, dy: i32, mode: ExtractMode) {
    let eid = stable_id(world, agent);
    if !((dx == 0 && dy == 0) || dx.abs() + dy.abs() == 1) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "extract_range".into(),
            });
        return;
    }
    // dig hard rock: adjacent only (not underfoot wall-self issues — can dig adj)
    if matches!(mode, ExtractMode::Dig) && dx == 0 && dy == 0 {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "dig_not_underfoot".into(),
            });
        return;
    }
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let Some(feat) = world.resource::<Grid>().get(tx, ty) else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "oob".into(),
            });
        return;
    };

    let rule = match mode {
        ExtractMode::Dig => sandbox::dig_rule(feat),
        ExtractMode::Scoop => sandbox::scoop_rule(feat),
    };
    let Some(rule) = rule else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: match mode {
                    ExtractMode::Dig => "not_diggable",
                    ExtractMode::Scoop => "not_scoopable",
                }
                .into(),
            });
        return;
    };

    // scoop underfoot OK even if result walkable; dig never underfoot
    world.resource_mut::<Grid>().set(tx, ty, rule.leave);

    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        inv.add(Matter::Terrain { feat }, 1);
        if rule.bonus_iron > 0 {
            inv.add(
                Matter::Resource {
                    resource: ResourceKind::Iron,
                },
                rule.bonus_iron,
            );
        }
    }

    let ev = match mode {
        ExtractMode::Dig => GameEvent::Dug {
            entity: eid,
            at: (tx, ty),
            from_feat: feat,
            to_feat: rule.leave,
        },
        ExtractMode::Scoop => GameEvent::Scooped {
            entity: eid,
            at: (tx, ty),
            from_feat: feat,
            to_feat: rule.leave,
        },
    };
    world.resource_mut::<EventBuf>().push(ev);
}

enum ExtractMode {
    Dig,
    Scoop,
}

pub fn apply_dig(world: &mut World, agent: Entity, dx: i32, dy: i32) {
    apply_extract(world, agent, dx, dy, ExtractMode::Dig);
}

pub fn apply_scoop(world: &mut World, agent: Entity, dx: i32, dy: i32) {
    apply_extract(world, agent, dx, dy, ExtractMode::Scoop);
}

pub fn apply_place(world: &mut World, agent: Entity, dx: i32, dy: i32, slot: Option<usize>) {
    let eid = stable_id(world, agent);
    if !((dx == 0 && dy == 0) || dx.abs() + dy.abs() == 1) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "place_range".into(),
            });
        return;
    }
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let (tx, ty) = (pos.x + dx, pos.y + dy);
    let underfoot = dx == 0 && dy == 0;

    let slot_i = {
        let Some(inv) = world.get::<Inventory>(agent) else {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "no_inventory".into(),
                });
            return;
        };
        slot.or_else(|| inv.first_terrain_slot())
    };
    let Some(slot_i) = slot_i else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "no_terrain_in_pack".into(),
            });
        return;
    };

    let matter = world
        .get::<Inventory>(agent)
        .and_then(|inv| inv.slots.get(slot_i).map(|s| s.matter.clone()));
    let Some(Matter::Terrain { feat }) = matter else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "slot_not_terrain".into(),
            });
        return;
    };

    let Some(cur) = world.resource::<Grid>().get(tx, ty) else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "oob".into(),
            });
        return;
    };

    if let Err(reason) = can_place_on(cur, feat, underfoot) {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: reason.into(),
            });
        return;
    }

    // Overwriting diggable/scoopable ground returns the displaced block to
    // the pack — except plain floor, which would flood the pack.
    let displaced = if cur != feat
        && cur != crate::f_info::id::FLOOR
        && (sandbox::is_diggable(cur) || sandbox::is_scoopable(cur))
    {
        Some(cur)
    } else {
        None
    };

    if let Some(mut inv) = world.get_mut::<Inventory>(agent) {
        if inv.take_one(slot_i).is_none() {
            world
                .resource_mut::<EventBuf>()
                .push(GameEvent::ActionRejected {
                    entity: eid,
                    reason: "slot_empty".into(),
                });
            return;
        }
        if let Some(d) = displaced {
            inv.add(Matter::Terrain { feat: d }, 1);
        }
    }

    world.resource_mut::<Grid>().set(tx, ty, feat);
    world.resource_mut::<EventBuf>().push(GameEvent::Placed {
        entity: eid,
        at: (tx, ty),
        feat,
    });
}
