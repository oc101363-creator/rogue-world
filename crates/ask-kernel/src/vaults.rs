//! Load Frog `vaults.txt` room templates (M: maps).
//!
//! Map letters follow classic Angband vault convention (subset):
//!   `#` wall  `%` quartz/outer  `.` floor  `+` door  `'` open door
//!   `^` trap  `~` water  `<` `>` stairs  `*` treasure wall  `:` rubble
//!   `,` dirt  `;` grass-ish  spaces ignored as solid fill outside
//! Monsters/objects letters are treated as floor (ASK has no r_info yet).

use std::sync::OnceLock;

use crate::f_info::id;

const VAULTS_TXT: &str = include_str!("../data/vaults.txt");

#[derive(Clone, Debug)]
pub struct VaultTemplate {
    pub name: String,
    pub greater: bool,
    /// Rows of map letters (ragged OK; padded on stamp)
    pub map: Vec<String>,
}

impl VaultTemplate {
    pub fn height(&self) -> i32 {
        self.map.len() as i32
    }

    pub fn width(&self) -> i32 {
        self.map.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32
    }
}

static VAULTS: OnceLock<Vec<VaultTemplate>> = OnceLock::new();

pub fn table() -> &'static [VaultTemplate] {
    VAULTS.get_or_init(parse_vaults)
}

fn parse_vaults() -> Vec<VaultTemplate> {
    let mut out = Vec::new();
    let mut name = String::new();
    let mut greater = false;
    let mut map: Vec<String> = Vec::new();
    let mut in_vault = false;

    let flush = |out: &mut Vec<VaultTemplate>,
                 name: &mut String,
                 greater: bool,
                 map: &mut Vec<String>,
                 in_vault: &mut bool| {
        if *in_vault && !map.is_empty() {
            out.push(VaultTemplate {
                name: std::mem::take(name),
                greater,
                map: std::mem::take(map),
            });
        }
        *in_vault = false;
        map.clear();
        name.clear();
    };

    for line in VAULTS_TXT.lines() {
        if line.starts_with('N') && line.get(1..2) == Some(":") {
            flush(&mut out, &mut name, greater, &mut map, &mut in_vault);
            name = line[2..].to_string();
            greater = false;
            in_vault = false;
        } else if line.starts_with("T:VAULT:") {
            greater = line.contains("GREATER");
            in_vault = true;
        } else if line.starts_with("T:") {
            // non-vault template in same file style — skip
            in_vault = false;
            map.clear();
        } else if line.starts_with("M:") && in_vault {
            map.push(line[2..].to_string());
        }
    }
    flush(&mut out, &mut name, greater, &mut map, &mut in_vault);
    out
}

/// Map a vault letter to a frog feat id (monsters/objects → floor).
pub fn letter_to_feat(ch: char) -> u16 {
    match ch {
        ' ' => id::GRANITE_SOLID, // outside padding often space in some vaults
        '#' => id::GRANITE,
        '%' => id::QUARTZ_VEIN,
        '.' => id::FLOOR,
        ',' => id::DIRT,
        ';' => id::GRASS,
        '+' => id::CLOSED_DOOR,
        '\'' => id::OPEN_DOOR,
        '^' => id::TRAP_PIT, // generic visible trap; alloc may diversify later
        '~' => id::SHALLOW_WATER,
        '<' => id::UP_STAIR,
        '>' => id::DOWN_STAIR,
        '*' => id::QUARTZ_TREASURE,
        '&' => id::MAGMA_TREASURE,
        ':' => id::RUBBLE,
        'x' | 'X' => id::GRANITE_INNER,
        // digits / letters for monsters & objects → open floor
        _ => id::FLOOR,
    }
}

/// Stamp vault onto feat grid; top-left at (ox, oy). Returns false if OOB.
pub fn stamp_vault(feats: &mut [u16], w: i32, h: i32, ox: i32, oy: i32, vault: &VaultTemplate) -> bool {
    let vw = vault.width();
    let vh = vault.height();
    if ox < 1 || oy < 1 || ox + vw >= w - 1 || oy + vh >= h - 1 {
        return false;
    }
    for (row_i, row) in vault.map.iter().enumerate() {
        for (col_i, ch) in row.chars().enumerate() {
            let x = ox + col_i as i32;
            let y = oy + row_i as i32;
            let feat = letter_to_feat(ch);
            // don't overwrite permanent border
            let i = (y * w + x) as usize;
            if feats[i] == id::PERMANENT {
                continue;
            }
            feats[i] = feat;
        }
    }
    true
}

pub fn pick_vault(rng: &mut impl VaultRng, greater: bool) -> Option<&'static VaultTemplate> {
    let all = table();
    let candidates: Vec<_> = all.iter().filter(|v| v.greater == greater).collect();
    if candidates.is_empty() {
        return None;
    }
    let i = rng.vault_index(candidates.len());
    Some(candidates[i])
}

pub trait VaultRng {
    fn vault_index(&mut self, n: usize) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_vaults() {
        let t = table();
        assert!(t.len() >= 100, "vaults={}", t.len());
        assert!(t.iter().any(|v| v.greater));
        assert!(t.iter().any(|v| !v.greater));
        assert!(t.iter().any(|v| v.width() >= 10 && v.height() >= 8));
    }
}
