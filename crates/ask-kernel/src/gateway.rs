//! Agent Gateway stub — future WebSocket / Python runtime attachment.
//!
//! Agents must only request actions; they never mutate World directly.

use crate::actions::Action;

/// Placeholder for external agent sessions.
pub trait AgentGateway {
    fn poll_actions(&mut self) -> Vec<(u64, Action)>;
}

/// No-op gateway used in MVP-0.
pub struct NullGateway;

impl AgentGateway for NullGateway {
    fn poll_actions(&mut self) -> Vec<(u64, Action)> {
        Vec::new()
    }
}
