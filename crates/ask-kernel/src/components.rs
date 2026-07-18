use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::f_info::{self, FeatId};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph(pub char);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Agent;

/// Registered agent identity (name/purpose from skill onboarding).
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    pub purpose: String,
}

/// Unified carryable matter — sandbox pack atom.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Matter {
    /// f_info terrain block — dig out / place back.
    Terrain { feat: FeatId },
    /// Abstract resource (build fuel, etc.).
    Resource { resource: ResourceKind },
    /// k_info object kind.
    Object { kind_id: u16, name: String },
}

impl Matter {
    pub fn label(&self) -> String {
        match self {
            Matter::Terrain { feat } => f_info::table()
                .get(*feat)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("feat#{feat}")),
            Matter::Resource { resource } => match resource {
                ResourceKind::Wood => "wood".into(),
                ResourceKind::Iron => "iron".into(),
            },
            Matter::Object { name, .. } => name.clone(),
        }
    }

    pub fn glyph(&self) -> char {
        match self {
            Matter::Terrain { feat } => f_info::table().glyph(*feat),
            Matter::Resource {
                resource: ResourceKind::Wood,
            } => 'T',
            Matter::Resource {
                resource: ResourceKind::Iron,
            } => 'I',
            Matter::Object { name, .. } => name
                .chars()
                .find(|c| c.is_ascii_alphanumeric())
                .unwrap_or('?'),
        }
    }

    pub fn color(&self) -> char {
        match self {
            Matter::Terrain { feat } => f_info::table().color_letter(*feat),
            Matter::Resource {
                resource: ResourceKind::Wood,
            } => 'g',
            Matter::Resource {
                resource: ResourceKind::Iron,
            } => 'W',
            Matter::Object { .. } => 'w',
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub matter: Matter,
    pub qty: u32,
}

/// Agent pack — stacked matter only (no parallel wood/iron fields).
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Stack>,
}

impl Inventory {
    pub fn qty_resource(&self, kind: ResourceKind) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| match &s.matter {
                Matter::Resource { resource } if *resource == kind => Some(s.qty),
                _ => None,
            })
            .sum()
    }

    pub fn wood(&self) -> u32 {
        self.qty_resource(ResourceKind::Wood)
    }

    pub fn iron(&self) -> u32 {
        self.qty_resource(ResourceKind::Iron)
    }

    /// Add qty of matter, stacking into an existing equal stack when possible.
    pub fn add(&mut self, matter: Matter, qty: u32) {
        if qty == 0 {
            return;
        }
        if let Some(s) = self.slots.iter_mut().find(|s| s.matter == matter) {
            s.qty = s.qty.saturating_add(qty);
            return;
        }
        self.slots.push(Stack { matter, qty });
    }

    /// Remove qty of an exact matter kind. Returns false if not enough.
    pub fn remove_matter(&mut self, matter: &Matter, qty: u32) -> bool {
        let Some(i) = self.slots.iter().position(|s| &s.matter == matter) else {
            return false;
        };
        if self.slots[i].qty < qty {
            return false;
        }
        self.slots[i].qty -= qty;
        if self.slots[i].qty == 0 {
            self.slots.remove(i);
        }
        true
    }

    /// Remove qty across all stacks matching `pred` (THE one consume loop —
    /// resource/terrain/rock helpers are one-liners over this).
    pub fn remove_matching(&mut self, pred: impl Fn(&Matter) -> bool, qty: u32) -> bool {
        let have: u32 = self
            .slots
            .iter()
            .filter(|s| pred(&s.matter))
            .map(|s| s.qty)
            .sum();
        if have < qty {
            return false;
        }
        let mut left = qty;
        self.slots.retain_mut(|s| {
            if left == 0 || !pred(&s.matter) {
                return true;
            }
            if s.qty <= left {
                left -= s.qty;
                false
            } else {
                s.qty -= left;
                left = 0;
                true
            }
        });
        left == 0
    }

    /// Remove qty of a resource kind (any matching stack).
    pub fn remove_resource(&mut self, kind: ResourceKind, qty: u32) -> bool {
        self.remove_matching(
            |m| matches!(m, Matter::Resource { resource } if *resource == kind),
            qty,
        )
    }

    /// Take one unit from slot index; returns the matter.
    pub fn take_one(&mut self, index: usize) -> Option<Matter> {
        if index >= self.slots.len() {
            return None;
        }
        let matter = self.slots[index].matter.clone();
        self.slots[index].qty -= 1;
        if self.slots[index].qty == 0 {
            self.slots.remove(index);
        }
        Some(matter)
    }

    /// First terrain stack index, if any.
    pub fn first_terrain_slot(&self) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| matches!(s.matter, Matter::Terrain { .. }))
    }

    pub fn qty_terrain(&self, feat: FeatId) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| match &s.matter {
                Matter::Terrain { feat: f } if *f == feat => Some(s.qty),
                _ => None,
            })
            .sum()
    }

    pub fn remove_terrain(&mut self, feat: FeatId, qty: u32) -> bool {
        self.remove_matter(&Matter::Terrain { feat }, qty)
    }

    pub fn as_view(&self) -> Vec<(Matter, u32)> {
        self.slots
            .iter()
            .map(|s| (s.matter.clone(), s.qty))
            .collect()
    }

    /// Serialize pack for API.
    pub fn to_api(&self) -> Vec<serde_json::Value> {
        self.slots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                serde_json::json!({
                    "slot": i,
                    "qty": s.qty,
                    "label": s.matter.label(),
                    "glyph": s.matter.glyph().to_string(),
                    "matter": s.matter,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Wood,
    Iron,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Resource {
    pub kind: ResourceKind,
    pub amount: u32,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Building;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableId(pub u64);

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Health {
    pub hp: i32,
    pub max_hp: i32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            hp: crate::balance::AGENT_HP,
            max_hp: crate::balance::AGENT_HP,
        }
    }
}

impl Health {
    pub fn damage(&mut self, n: i32) {
        self.hp = (self.hp - n).max(0);
    }
}

/// Per-agent explored-cell memory (server-side truth; never sent raw to clients).
/// MARK flag per cell + the feat id AS REMEMBERED — fog never shows live edits.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct VisionMemory {
    pub width: i32,
    pub height: i32,
    /// Only MARK flag is meaningful here.
    pub flags: Vec<u8>,
    /// Remembered feat per cell (valid where MARK is set).
    pub feats: Vec<u16>,
}

impl VisionMemory {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            flags: vec![0; (width * height) as usize],
            feats: vec![0; (width * height) as usize],
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            None
        } else {
            Some((y * self.width + x) as usize)
        }
    }

    pub fn mark(&mut self, x: i32, y: i32, feat: u16) {
        if let Some(i) = self.idx(x, y) {
            self.flags[i] |= crate::vision::F_MARK;
            self.feats[i] = feat;
        }
    }

    pub fn is_mark(&self, x: i32, y: i32) -> bool {
        self.idx(x, y)
            .map(|i| self.flags[i] & crate::vision::F_MARK != 0)
            .unwrap_or(false)
    }

    /// Remembered feat id at (x, y) — Some only where the cell is marked.
    pub fn feat(&self, x: i32, y: i32) -> Option<u16> {
        if self.is_mark(x, y) {
            self.idx(x, y).map(|i| self.feats[i])
        } else {
            None
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct Monster {
    pub race_id: u16,
    pub name: String,
    pub color: char,
}

/// Ground object — carries matter (what you get when you pick it up).
#[derive(Component, Clone, Debug)]
pub struct Item {
    pub matter: Matter,
    pub qty: u32,
}

impl Item {
    pub fn name(&self) -> String {
        if self.qty > 1 {
            format!("{}×{}", self.matter.label(), self.qty)
        } else {
            self.matter.label()
        }
    }

    pub fn glyph(&self) -> char {
        self.matter.glyph()
    }

    pub fn color(&self) -> char {
        self.matter.color()
    }
}

/// One message delivered to an agent from an external player/spectator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub id: u64,
    pub from: String,
    pub text: String,
    pub sent_tick: u64,
    pub read: bool,
}

/// Per-agent inbox. Lives on every entity with `Agent`.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentMailbox {
    pub messages: Vec<Envelope>,
}

impl AgentMailbox {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a message, dropping oldest if the cap is exceeded.
    pub fn push(&mut self, env: Envelope) {
        const CAP: usize = 32;
        self.messages.push(env);
        if self.messages.len() > CAP {
            let drop = self.messages.len() - CAP;
            self.messages.drain(0..drop);
        }
    }

    pub fn unread(&self) -> Vec<&Envelope> {
        self.messages.iter().filter(|m| !m.read).collect()
    }

    pub fn mark_read(&mut self, ids: &[u64]) {
        for m in &mut self.messages {
            if ids.contains(&m.id) {
                m.read = true;
            }
        }
    }
}

/// Global monotonic id source for Envelopes.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MessageCounter(pub u64);

/// Delivery record for one sent message (one recipient per record).
/// Channel metadata — the kernel knows DELIVERY, never meaning.
#[derive(Clone, Debug)]
pub struct MessageRecord {
    pub id: u64,
    /// Gating key (the sender's token) — never leaves the server.
    pub sender_key: String,
    pub from: String,
    pub target: u64,
    pub sent_tick: u64,
    pub read_tick: Option<u64>,
}

/// Delivery ledger: who was sent what, and whether they've read it.
/// Ring-capped and ephemeral (like EventInbox) — receipts are operational
/// state, not world state; a restart simply forgets them.
#[derive(Resource, Default)]
pub struct MessageLedger {
    pub records: std::collections::VecDeque<MessageRecord>,
}

impl MessageLedger {
    pub const CAP: usize = 100;

    pub fn record(&mut self, rec: MessageRecord) {
        if self.records.len() >= Self::CAP {
            self.records.pop_front();
        }
        self.records.push_back(rec);
    }

    /// Stamp read ticks for envelopes `ids` just consumed by `agent_id`.
    pub fn mark_read(&mut self, agent_id: u64, ids: &[u64], tick: u64) {
        for rec in self.records.iter_mut() {
            if rec.target == agent_id && rec.read_tick.is_none() && ids.contains(&rec.id) {
                rec.read_tick = Some(tick);
            }
        }
    }

    pub fn status(&self, ids: &[u64]) -> Vec<&MessageRecord> {
        self.records
            .iter()
            .filter(|r| ids.contains(&r.id))
            .collect()
    }
}

/// Reserved target id for messages FROM agents TO the operator.
/// Pseudo-entity by design: never in the agents table, never has FOV,
/// exempt from the visibility gate (you may always talk to your operator).
pub const OPERATOR_TARGET: u64 = 0;

/// Operator inbox — dev-token-only, consume-on-read, ring-capped.
#[derive(Resource, Default)]
pub struct OperatorInbox {
    pub messages: std::collections::VecDeque<Envelope>,
}

impl OperatorInbox {
    pub const CAP: usize = 100;

    pub fn push(&mut self, env: Envelope) {
        if self.messages.len() >= Self::CAP {
            self.messages.pop_front();
        }
        self.messages.push_back(env);
    }

    pub fn drain(&mut self) -> Vec<Envelope> {
        self.messages.drain(..).collect()
    }
}

/// Per-agent feedback feed — the events THIS agent perceived, held until its
/// next view (consume-on-read), stamped with the tick they happened on.
///
/// Unlike the spectator event ring (age-capped, shared), this never expires:
/// an agent whose think time spans many ticks (LLM latency ≫ tick) still
/// finds its own act feedback whenever it comes back. Filled once per tick
/// by `events::distribute_feedback` with push-time FOV filtering.
#[derive(Component, Clone, Debug, Default)]
pub struct EventInbox {
    pub events: std::collections::VecDeque<(u64, crate::events::GameEvent)>,
}

impl EventInbox {
    /// Generous bound for an agent that never views; oldest drop first.
    pub const CAP: usize = 200;

    pub fn push(&mut self, tick: u64, ev: crate::events::GameEvent) {
        if self.events.len() >= Self::CAP {
            self.events.pop_front();
        }
        self.events.push_back((tick, ev));
    }

    pub fn drain(&mut self) -> Vec<(u64, crate::events::GameEvent)> {
        self.events.drain(..).collect()
    }
}

#[cfg(test)]
mod mailbox_tests {
    use super::*;

    #[test]
    fn mailbox_keeps_unread_and_caps_at_32() {
        let mut mb = AgentMailbox::new();
        for i in 0..40 {
            mb.push(Envelope {
                id: 100 + i as u64,
                from: "anon".into(),
                text: format!("msg {i}"),
                sent_tick: i as u64,
                read: false,
            });
        }
        assert_eq!(mb.messages.len(), 32);
        // oldest messages dropped on overflow
        assert_eq!(mb.messages[0].id, 108);
        assert_eq!(mb.unread().len(), 32);
        mb.mark_read(&[108, 109]);
        assert_eq!(mb.unread().len(), 30);
    }
}
