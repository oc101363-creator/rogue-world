//! Agent identity registry — unlimited self-serve registration via opaque tokens.
//!
//! Skill flow: agent provides name + purpose → server mints `ask1_…` token →
//! all actions/me use that token. Spectators paste token(s) to track on the map.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// Public registration record (safe to list without full token).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentPublic {
    pub agent_id: u64,
    pub name: String,
    pub purpose: String,
    pub x: i32,
    pub y: i32,
    pub alive: bool,
}

#[derive(Clone, Debug)]
struct AgentRecord {
    agent_id: u64,
    name: String,
    purpose: String,
    /// Full secret token (only returned once at register; stored for auth).
    token: String,
    x: i32,
    y: i32,
    alive: bool,
}

#[derive(Clone, Debug)]
pub struct PendingSpawn {
    pub name: String,
    pub purpose: String,
    /// Filled by sim thread after spawn.
    pub result: Arc<Mutex<Option<RegisterResult>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterResult {
    pub ok: bool,
    pub token: String,
    pub agent_id: u64,
    pub name: String,
    pub purpose: String,
    pub x: i32,
    pub y: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Default)]
pub struct AgentRegistry {
    inner: Arc<Mutex<RegInner>>,
}

#[derive(Default)]
struct RegInner {
    /// token → record
    by_token: HashMap<String, AgentRecord>,
    /// agent_id → token
    by_id: HashMap<u64, String>,
    pending_spawns: Vec<PendingSpawn>,
    /// server secret for token minting
    secret: u64,
    nonce: u64,
    /// omniscient developer spectator token (hard to guess, printed once on startup)
    dev_token: String,
}

impl AgentRegistry {
    pub fn new(world_seed: u64) -> Self {
        let mut inner = RegInner::default();
        // non-zero secret from seed + fixed salt
        inner.secret = world_seed
            .wrapping_mul(0xD1B54A32D192ED03)
            .wrapping_add(0xA0761D6478BD642F)
            | 1;
        inner.dev_token = mint_dev_token(inner.secret);
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// The omniscient developer token.
    pub fn dev_token(&self) -> String {
        self.inner.lock().expect("auth").dev_token.clone()
    }

    pub fn is_dev_token(&self, token: &str) -> bool {
        self.inner.lock().expect("auth").dev_token == token
    }

    pub fn dev_public(&self) -> AgentPublic {
        AgentPublic {
            agent_id: 0,
            name: "DEV".into(),
            purpose: "omniscient spectator".into(),
            x: 0,
            y: 0,
            alive: true,
        }
    }

    /// Queue a spawn; returns a handle that fills when sim processes it.
    pub fn request_register(
        &self,
        name: String,
        purpose: String,
    ) -> Arc<Mutex<Option<RegisterResult>>> {
        let result = Arc::new(Mutex::new(None));
        let mut g = self.inner.lock().expect("auth");
        g.pending_spawns.push(PendingSpawn {
            name,
            purpose,
            result: result.clone(),
        });
        result
    }

    pub fn drain_spawns(&self) -> Vec<PendingSpawn> {
        let mut g = self.inner.lock().expect("auth");
        std::mem::take(&mut g.pending_spawns)
    }

    pub fn bind_spawned(
        &self,
        name: String,
        purpose: String,
        agent_id: u64,
        x: i32,
        y: i32,
    ) -> String {
        let mut g = self.inner.lock().expect("auth");
        g.nonce = g.nonce.wrapping_add(1);
        let token = mint_token(g.secret, agent_id, g.nonce);
        let rec = AgentRecord {
            agent_id,
            name,
            purpose,
            token: token.clone(),
            x,
            y,
            alive: true,
        };
        g.by_id.insert(agent_id, token.clone());
        g.by_token.insert(token.clone(), rec);
        token
    }

    pub fn resolve_token(&self, token: &str) -> Option<u64> {
        let g = self.inner.lock().expect("auth");
        g.by_token
            .get(token)
            .filter(|r| r.alive)
            .map(|r| r.agent_id)
    }

    pub fn update_pose(&self, agent_id: u64, x: i32, y: i32, alive: bool) {
        let mut g = self.inner.lock().expect("auth");
        if let Some(tok) = g.by_id.get(&agent_id).cloned() {
            if let Some(r) = g.by_token.get_mut(&tok) {
                r.x = x;
                r.y = y;
                r.alive = alive;
            }
        }
    }

    pub fn public_for_token(&self, token: &str) -> Option<AgentPublic> {
        let g = self.inner.lock().expect("auth");
        if g.dev_token == token {
            return Some(self.dev_public());
        }
        g.by_token.get(token).map(|r| AgentPublic {
            agent_id: r.agent_id,
            name: r.name.clone(),
            purpose: r.purpose.clone(),
            x: r.x,
            y: r.y,
            alive: r.alive,
        })
    }

    pub fn list_public(&self) -> Vec<AgentPublic> {
        let g = self.inner.lock().expect("auth");
        let mut v: Vec<_> = g
            .by_token
            .values()
            .map(|r| AgentPublic {
                agent_id: r.agent_id,
                name: r.name.clone(),
                purpose: r.purpose.clone(),
                x: r.x,
                y: r.y,
                alive: r.alive,
            })
            .collect();
        v.sort_by_key(|a| a.agent_id);
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().expect("auth").by_token.len()
    }
}

/// Opaque token: `ask1_` + 32 hex chars (128-bit-ish fingerprint).
fn mint_token(secret: u64, agent_id: u64, nonce: u64) -> String {
    let mut a = secret
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(agent_id.wrapping_mul(0xBF58476D1CE4E5B9));
    let mut b = nonce
        .wrapping_mul(0x94D049BB133111EB)
        .wrapping_add(secret.rotate_left(13));
    // xorshift mix
    a ^= a >> 12;
    a ^= a << 25;
    a ^= a >> 27;
    b ^= b >> 12;
    b ^= b << 25;
    b ^= b >> 27;
    a = a.wrapping_mul(0x2545F4914F6CDD1D);
    b = b.wrapping_mul(0x2545F4914F6CDD1D);
    format!("ask1_{a:016x}{b:016x}")
}

/// Omniscient developer token: deterministic from the server secret but
/// distinct from agent tokens. Format `ask1_dev_` + 32 hex chars.
fn mint_dev_token(secret: u64) -> String {
    let mut a = secret.wrapping_mul(0x517CC1B727220A95);
    let mut b = secret.rotate_left(31).wrapping_mul(0xA13FC49EA56F8D3B);
    a ^= a >> 17;
    a ^= a << 31;
    a ^= a >> 8;
    b ^= b >> 19;
    b ^= b << 27;
    b ^= b >> 11;
    a = a.wrapping_mul(0x2545F4914F6CDD1D);
    b = b.wrapping_mul(0x2545F4914F6CDD1D);
    format!("ask1_dev_{a:016x}{b:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_unique_and_resolvable() {
        let reg = AgentRegistry::new(42);
        let t1 = reg.bind_spawned("A".into(), "p".into(), 1, 0, 0);
        let t2 = reg.bind_spawned("B".into(), "q".into(), 2, 1, 1);
        assert_ne!(t1, t2);
        assert!(t1.starts_with("ask1_"));
        assert_eq!(reg.resolve_token(&t1), Some(1));
        assert_eq!(reg.resolve_token(&t2), Some(2));
        assert_eq!(reg.resolve_token("nope"), None);
    }
}
