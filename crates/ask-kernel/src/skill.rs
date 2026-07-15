//! Skill as declarative policy — not executable plugins.
//! MVP-0: trait only.

use crate::actions::Action;
use crate::events::GameEvent;

pub trait SkillPort {
    fn on_events(&self, events: &[GameEvent]) -> Vec<Action>;
}

pub struct NullSkills;

impl SkillPort for NullSkills {
    fn on_events(&self, _events: &[GameEvent]) -> Vec<Action> {
        Vec::new()
    }
}
