//! World memory ports (Personal / Organization / Civilization / Global).
//! MVP-0: trait only.

pub trait MemoryPort {
    fn remember(&mut self, scope: MemoryScope, content: &str);
    fn recall(&self, scope: MemoryScope, limit: usize) -> Vec<String>;
}

#[derive(Clone, Copy, Debug)]
pub enum MemoryScope {
    Personal { agent_id: u64 },
    Organization { org_id: u64 },
    Civilization { civ_id: u64 },
    Global,
}

pub struct NullMemory;

impl MemoryPort for NullMemory {
    fn remember(&mut self, _scope: MemoryScope, _content: &str) {}
    fn recall(&self, _scope: MemoryScope, _limit: usize) -> Vec<String> {
        Vec::new()
    }
}
