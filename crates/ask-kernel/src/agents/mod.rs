pub mod mock;

use bevy_ecs::prelude::*;

use crate::actions::Action;

/// Future: WS / Python gateway implements this.
pub trait AgentPolicy: Send + Sync {
    /// `world` is only read; `&mut` is required by bevy_ecs query API.
    fn decide(&mut self, world: &mut World, agent: Entity) -> Action;
}
