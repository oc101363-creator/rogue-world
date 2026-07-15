//! Frog room/vault templates: `vaults.txt` (VAULT) + `rooms.txt` (ROOM).
//!
//! Map letters → f_info ids. Monster/object letters become floor until r_info/k_info exist.

use std::sync::OnceLock;

use crate::f_info::id;

const VAULTS_TXT: &str = include_str!("../data/vaults.txt");
const ROOMS_TXT: &str = include_str!("../data/rooms.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    LesserVault,
    GreaterVault,
    Room,
}

#[derive(Clone, Debug)]
pub struct MapTemplate {
    pub name: String,
    pub kind: TemplateKind,
    pub map: Vec<String>,
}

impl MapTemplate {
    pub fn height(&self) -> i32 {
        self.map.len() as i32
    }

    pub fn width(&self) -> i32 {
        self.map.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32
    }
}

#[derive(Default)]
struct Tables {
    lesser: Vec<MapTemplate>,
    greater: Vec<MapTemplate>,
    rooms: Vec<MapTemplate>,
}

static TABLES: OnceLock<Tables> = OnceLock::new();

fn tables() -> &'static Tables {
    TABLES.get_or_init(|| {
        let mut t = Tables::default();
        parse_into(VAULTS_TXT, &mut t, true);
        parse_into(ROOMS_TXT, &mut t, false);
        t
    })
}

fn parse_into(text: &str, t: &mut Tables, vault_file: bool) {
    let mut name = String::new();
    let mut kind: Option<TemplateKind> = None;
    let mut map: Vec<String> = Vec::new();

    let flush = |t: &mut Tables, name: &mut String, kind: &mut Option<TemplateKind>, map: &mut Vec<String>| {
        if let Some(k) = kind.take() {
            if !map.is_empty() {
                let tmpl = MapTemplate {
                    name: std::mem::take(name),
                    kind: k,
                    map: std::mem::take(map),
                };
                match k {
                    TemplateKind::LesserVault => t.lesser.push(tmpl),
                    TemplateKind::GreaterVault => t.greater.push(tmpl),
                    TemplateKind::Room => t.rooms.push(tmpl),
                }
            }
        }
        map.clear();
        name.clear();
        *kind = None;
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("N:") {
            flush(t, &mut name, &mut kind, &mut map);
            name = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("T:") {
            // Only keep VAULT / ROOM lines; ignore WILD/AMBUSH for dungeon gen
            if rest.starts_with("VAULT:GREATER") {
                kind = Some(TemplateKind::GreaterVault);
            } else if rest.starts_with("VAULT:LESSER") {
                kind = Some(TemplateKind::LesserVault);
            } else if rest.starts_with("ROOM:") {
                kind = Some(TemplateKind::Room);
            } else {
                kind = None;
                map.clear();
            }
        } else if line.starts_with("M:") && kind.is_some() {
            map.push(line[2..].to_string());
        } else if vault_file {
            // ignore L:/W: etc.
        }
    }
    flush(t, &mut name, &mut kind, &mut map);
}

pub fn lesser_vaults() -> &'static [MapTemplate] {
    &tables().lesser
}
pub fn greater_vaults() -> &'static [MapTemplate] {
    &tables().greater
}
pub fn room_templates() -> &'static [MapTemplate] {
    &tables().rooms
}

pub fn letter_to_feat(ch: char) -> u16 {
    match ch {
        ' ' => id::GRANITE_SOLID,
        '#' => id::GRANITE,
        '%' => id::QUARTZ_VEIN,
        '.' => id::FLOOR,
        ',' => id::DIRT,
        ';' => id::GRASS,
        '+' => id::CLOSED_DOOR,
        '\'' => id::OPEN_DOOR,
        '^' => id::TRAP_PIT,
        '~' => id::SHALLOW_WATER,
        '<' => id::UP_STAIR,
        '>' => id::DOWN_STAIR,
        '*' => id::QUARTZ_TREASURE,
        '&' => id::MAGMA_TREASURE,
        ':' => id::RUBBLE,
        'x' | 'X' => id::GRANITE_INNER,
        'T' => id::TREE,
        _ => id::FLOOR,
    }
}

pub fn stamp_template(
    feats: &mut [u16],
    w: i32,
    h: i32,
    ox: i32,
    oy: i32,
    tmpl: &MapTemplate,
) -> bool {
    let vw = tmpl.width();
    let vh = tmpl.height();
    if ox < 1 || oy < 1 || ox + vw >= w - 1 || oy + vh >= h - 1 {
        return false;
    }
    for (row_i, row) in tmpl.map.iter().enumerate() {
        for (col_i, ch) in row.chars().enumerate() {
            let x = ox + col_i as i32;
            let y = oy + row_i as i32;
            let i = (y * w + x) as usize;
            if feats[i] == id::PERMANENT {
                continue;
            }
            feats[i] = letter_to_feat(ch);
        }
    }
    true
}

pub trait TemplateRng {
    fn pick(&mut self, n: usize) -> usize;
}

pub fn pick_lesser(rng: &mut impl TemplateRng) -> Option<&'static MapTemplate> {
    let t = lesser_vaults();
    if t.is_empty() {
        None
    } else {
        Some(&t[rng.pick(t.len())])
    }
}

pub fn pick_greater(rng: &mut impl TemplateRng) -> Option<&'static MapTemplate> {
    let t = greater_vaults();
    if t.is_empty() {
        None
    } else {
        Some(&t[rng.pick(t.len())])
    }
}

pub fn pick_room(rng: &mut impl TemplateRng) -> Option<&'static MapTemplate> {
    let t = room_templates();
    if t.is_empty() {
        None
    } else {
        Some(&t[rng.pick(t.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_vaults_and_rooms() {
        assert!(lesser_vaults().len() >= 50, "lesser={}", lesser_vaults().len());
        assert!(greater_vaults().len() >= 50, "greater={}", greater_vaults().len());
        assert!(room_templates().len() >= 50, "rooms={}", room_templates().len());
    }
}
