//! Minimal frog `r_info.txt` loader — id, name, glyph, color for template MON().

use std::sync::OnceLock;

const R_INFO_TXT: &str = include_str!("../data/r_info.txt");

#[derive(Clone, Debug)]
pub struct MonsterRace {
    pub id: u16,
    pub name: String,
    pub glyph: char,
    pub color: char,
}

#[derive(Default)]
pub struct MonsterTable {
    by_id: Vec<Option<MonsterRace>>,
    /// glyph → race ids (for MON(o) style)
    by_glyph: std::collections::HashMap<char, Vec<u16>>,
    /// lowercase name → id (first match)
    by_name: std::collections::HashMap<String, u16>,
}

impl MonsterTable {
    pub fn get(&self, id: u16) -> Option<&MonsterRace> {
        self.by_id.get(id as usize).and_then(|x| x.as_ref())
    }

    pub fn count(&self) -> usize {
        self.by_id.iter().filter(|x| x.is_some()).count()
    }

    pub fn pick_by_glyph(&self, g: char, rng_idx: usize) -> Option<&MonsterRace> {
        let list = self.by_glyph.get(&g)?;
        if list.is_empty() {
            return None;
        }
        self.get(list[rng_idx % list.len()])
    }

    pub fn pick_any(&self, rng_idx: usize) -> Option<&MonsterRace> {
        let ids: Vec<u16> = self
            .by_id
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_ref().map(|_| i as u16))
            .filter(|&id| id != 0)
            .collect();
        if ids.is_empty() {
            return None;
        }
        self.get(ids[rng_idx % ids.len()])
    }

    pub fn find_name_contains(&self, needle: &str) -> Option<&MonsterRace> {
        let n = needle.to_ascii_lowercase();
        self.by_name
            .iter()
            .find(|(name, _)| name.contains(&n))
            .and_then(|(_, id)| self.get(*id))
    }

    /// Iterate all loaded races as (id, race).
    pub fn iter(&self) -> impl Iterator<Item = (u16, &MonsterRace)> {
        self.by_id
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.as_ref().map(|race| (i as u16, race)))
    }
}

static TABLE: OnceLock<MonsterTable> = OnceLock::new();

pub fn table() -> &'static MonsterTable {
    TABLE.get_or_init(parse)
}

fn parse() -> MonsterTable {
    let mut t = MonsterTable::default();
    let mut cur_id: Option<u16> = None;
    let mut cur_name = String::new();
    let mut glyph = '?';
    let mut color = 'w';

    let flush = |t: &mut MonsterTable, id: u16, name: String, glyph: char, color: char| {
        if id == 0 {
            return; // player
        }
        let race = MonsterRace {
            id,
            name: name.clone(),
            glyph,
            color,
        };
        let idx = id as usize;
        if t.by_id.len() <= idx {
            t.by_id.resize_with(idx + 1, || None);
        }
        t.by_glyph.entry(glyph).or_default().push(id);
        t.by_name.entry(name.to_ascii_lowercase()).or_insert(id);
        t.by_id[idx] = Some(race);
    };

    for line in R_INFO_TXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("N:") {
            if let Some(id) = cur_id.take() {
                flush(&mut t, id, std::mem::take(&mut cur_name), glyph, color);
                glyph = '?';
                color = 'w';
            }
            // N:id:name
            let mut parts = rest.splitn(2, ':');
            let id: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            cur_name = parts.next().unwrap_or("").to_string();
            cur_id = Some(id);
        } else if let Some(rest) = line.strip_prefix("G:") {
            let mut chs = rest.chars();
            glyph = chs.next().unwrap_or('?');
            let rest = rest.get(1..).unwrap_or("");
            let rest = rest.strip_prefix(':').unwrap_or(rest);
            color = rest.chars().next().unwrap_or('w');
        }
    }
    if let Some(id) = cur_id.take() {
        flush(&mut t, id, cur_name, glyph, color);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_monsters() {
        let t = table();
        assert!(t.count() > 500, "count={}", t.count());
        assert!(t.pick_by_glyph('o', 0).is_some() || t.pick_any(0).is_some());
    }
}
