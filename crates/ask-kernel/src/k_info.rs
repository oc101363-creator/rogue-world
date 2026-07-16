//! Minimal frog `k_info.txt` loader — id, name, glyph, color for template OBJ().

use std::sync::OnceLock;

const K_INFO_TXT: &str = include_str!("../data/k_info.txt");

#[derive(Clone, Debug)]
pub struct ObjectKind {
    pub id: u16,
    pub name: String,
    pub glyph: char,
    pub color: char,
}

#[derive(Default)]
pub struct ObjectTable {
    by_id: Vec<Option<ObjectKind>>,
    by_glyph: std::collections::HashMap<char, Vec<u16>>,
    all_ids: Vec<u16>,
}

impl ObjectTable {
    pub fn get(&self, id: u16) -> Option<&ObjectKind> {
        self.by_id.get(id as usize).and_then(|x| x.as_ref())
    }

    pub fn count(&self) -> usize {
        self.all_ids.len()
    }

    pub fn pick_any(&self, rng_idx: usize) -> Option<&ObjectKind> {
        if self.all_ids.is_empty() {
            return None;
        }
        self.get(self.all_ids[rng_idx % self.all_ids.len()])
    }

    pub fn pick_by_glyph(&self, g: char, rng_idx: usize) -> Option<&ObjectKind> {
        let list = self.by_glyph.get(&g)?;
        if list.is_empty() {
            return None;
        }
        self.get(list[rng_idx % list.len()])
    }

    /// Iterate all loaded object kinds as (id, kind).
    pub fn iter(&self) -> impl Iterator<Item = (u16, &ObjectKind)> {
        self.by_id
            .iter()
            .enumerate()
            .filter_map(|(i, k)| k.as_ref().map(|kind| (i as u16, kind)))
    }

    pub fn find_name_contains(&self, needle: &str) -> Option<&ObjectKind> {
        let n = needle.to_ascii_lowercase();
        for id in &self.all_ids {
            if let Some(o) = self.get(*id) {
                if o.name.to_ascii_lowercase().contains(&n) {
                    return Some(o);
                }
            }
        }
        None
    }
}

static TABLE: OnceLock<ObjectTable> = OnceLock::new();

pub fn table() -> &'static ObjectTable {
    TABLE.get_or_init(parse)
}

fn parse() -> ObjectTable {
    let mut t = ObjectTable::default();
    let mut cur_id: Option<u16> = None;
    let mut cur_name = String::new();
    let mut glyph = '?';
    let mut color = 'w';
    let mut next_auto = 1u16;

    let flush = |t: &mut ObjectTable, id: u16, name: String, glyph: char, color: char| {
        if id == 0 {
            return;
        }
        // strip angband & ~
        let name = name.replace('&', "").replace('~', "").trim().to_string();
        let kind = ObjectKind {
            id,
            name,
            glyph,
            color,
        };
        let idx = id as usize;
        if t.by_id.len() <= idx {
            t.by_id.resize_with(idx + 1, || None);
        }
        t.by_glyph.entry(glyph).or_default().push(id);
        t.all_ids.push(id);
        t.by_id[idx] = Some(kind);
    };

    for line in K_INFO_TXT.lines() {
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
            // N:id:name or N:*:name
            let mut parts = rest.splitn(2, ':');
            let id_part = parts.next().unwrap_or("0");
            let name = parts.next().unwrap_or("").to_string();
            let id = if id_part == "*" {
                let id = next_auto;
                next_auto = next_auto.saturating_add(1);
                id
            } else {
                let id: u16 = id_part.parse().unwrap_or(0);
                if id >= next_auto {
                    next_auto = id.saturating_add(1);
                }
                id
            };
            cur_name = name;
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
    fn loads_objects() {
        let t = table();
        assert!(t.count() > 100, "count={}", t.count());
        assert!(t.pick_any(0).is_some());
    }
}
