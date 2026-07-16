//! Player melee — frog bash / do_cmd_attack spirit (minimal).

use bevy_ecs::prelude::*;

use crate::components::{Health, Monster, Position};
use crate::events::{EventBuf, GameEvent};
use crate::systems::stable_id;

pub fn apply_attack(world: &mut World, agent: Entity, dx: i32, dy: i32) {
    let eid = stable_id(world, agent);
    if dx.abs() + dy.abs() != 1 {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "attack_not_adjacent".into(),
            });
        return;
    }
    let Some(pos) = world.get::<Position>(agent).copied() else {
        return;
    };
    let tx = pos.x + dx;
    let ty = pos.y + dy;

    let target = {
        let mut q = world.query::<(Entity, &Position, &Monster)>();
        q.iter(world)
            .find(|(_, p, _)| p.x == tx && p.y == ty)
            .map(|(e, _, m)| (e, m.name.clone()))
    };

    let Some((mon_e, name)) = target else {
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::ActionRejected {
                entity: eid,
                reason: "no_monster_there".into(),
            });
        return;
    };

    let damage = 3;
    if let Some(mut hp) = world.get_mut::<Health>(mon_e) {
        hp.damage(damage);
    }
    let mon_hp = world.get::<Health>(mon_e).map(|h| h.hp).unwrap_or(0);
    let mid = stable_id(world, mon_e);

    world
        .resource_mut::<EventBuf>()
        .push(GameEvent::PlayerAttacked {
            entity: eid,
            target: mid,
            damage,
            target_hp: mon_hp,
            name: name.clone(),
            at: (tx, ty),
        });

    if mon_hp <= 0 {
        world.despawn(mon_e);
        world
            .resource_mut::<EventBuf>()
            .push(GameEvent::MonsterKilled {
                entity: eid,
                monster: mid,
                name,
                at: (tx, ty),
            });
    }
}
