//! Frog room/vault templates: `vaults.txt` + `rooms.txt`.
//!
//! Parses `L:` letter directives (feat name / MON / OBJ / TRAP) and `M:` maps.
//! Stamp yields terrain feat ids + monster/object spawn markers.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::f_info::id;
use crate::k_info;
use crate::r_info;

const VAULTS_TXT: &str = include_str!("../data/vaults.txt");
const ROOMS_TXT: &str = include_str!("../data/rooms.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateKind {
    LesserVault,
    GreaterVault,
    Room,
}

#[derive(Clone, Debug, Default)]
pub struct LetterRule {
    pub feat: Option<u16>,
    pub mon: Option<MonSpec>,
    pub obj: Option<ObjSpec>,
    pub trap: Option<u16>,
}

#[derive(Clone, Debug)]
pub enum MonSpec {
    Any,
    Glyph(char),
    Name(String),
}

#[derive(Clone, Debug)]
pub enum ObjSpec {
    Any,
    Name(String),
}

#[derive(Clone, Debug)]
pub struct MapTemplate {
    pub name: String,
    pub kind: TemplateKind,
    pub map: Vec<String>,
    pub letters: HashMap<char, LetterRule>,
}

impl MapTemplate {
    pub fn height(&self) -> i32 {
        self.map.len() as i32
    }

    pub fn width(&self) -> i32 {
        self.map.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32
    }
}

#[derive(Clone, Debug)]
pub struct SpawnMon {
    pub x: i32,
    pub y: i32,
    pub race_id: u16,
    pub glyph: char,
    pub color: char,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct SpawnObj {
    pub x: i32,
    pub y: i32,
    pub kind_id: u16,
    pub glyph: char,
    pub color: char,
    pub name: String,
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
        parse_into(VAULTS_TXT, &mut t);
        parse_into(ROOMS_TXT, &mut t);
        t
    })
}

fn parse_into(text: &str, t: &mut Tables) {
    let mut name = String::new();
    let mut kind: Option<TemplateKind> = None;
    let mut map: Vec<String> = Vec::new();
    let mut letters: HashMap<char, LetterRule> = HashMap::new();

    let flush = |t: &mut Tables,
                 name: &mut String,
                 kind: &mut Option<TemplateKind>,
                 map: &mut Vec<String>,
                 letters: &mut HashMap<char, LetterRule>| {
        if let Some(k) = kind.take() {
            if !map.is_empty() {
                let tmpl = MapTemplate {
                    name: std::mem::take(name),
                    kind: k,
                    map: std::mem::take(map),
                    letters: std::mem::take(letters),
                };
                match k {
                    TemplateKind::LesserVault => t.lesser.push(tmpl),
                    TemplateKind::GreaterVault => t.greater.push(tmpl),
                    TemplateKind::Room => t.rooms.push(tmpl),
                }
            }
        }
        map.clear();
        letters.clear();
        name.clear();
        *kind = None;
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("N:") {
            flush(t, &mut name, &mut kind, &mut map, &mut letters);
            name = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("T:") {
            if rest.starts_with("VAULT:GREATER") {
                kind = Some(TemplateKind::GreaterVault);
            } else if rest.starts_with("VAULT:LESSER") {
                kind = Some(TemplateKind::LesserVault);
            } else if rest.starts_with("ROOM:") {
                kind = Some(TemplateKind::Room);
            } else {
                kind = None;
                map.clear();
                letters.clear();
            }
        } else if let Some(rest) = line.strip_prefix("L:") {
            if kind.is_some() {
                if let Some((ch, rule)) = parse_l_line(rest) {
                    letters.insert(ch, rule);
                }
            }
        } else if let Some(rest) = line.strip_prefix("M:") {
            if kind.is_some() {
                map.push(rest.to_string());
            }
        }
    }
    flush(t, &mut name, &mut kind, &mut map, &mut letters);
}

/// Parse `L:<letter>:<Directives>+`
fn parse_l_line(rest: &str) -> Option<(char, LetterRule)> {
    let mut parts = rest.splitn(2, ':');
    let letter = parts.next()?.chars().next()?;
    let dirs = parts.next().unwrap_or("");
    let mut rule = LetterRule::default();

    // Split directives by `:` but keep parentheses contents
    for dir in split_directives(dirs) {
        let d = dir.trim();
        if d.is_empty() {
            continue;
        }
        if let Some(inner) = d.strip_prefix("MON(").and_then(|s| s.strip_suffix(')')) {
            rule.mon = Some(parse_mon(inner));
        } else if let Some(inner) = d.strip_prefix("OBJ(").and_then(|s| s.strip_suffix(')')) {
            rule.obj = Some(parse_obj(inner));
        } else if let Some(inner) = d.strip_prefix("TRAP(").and_then(|s| s.strip_suffix(')')) {
            // TRAP(TRAP_OPEN, 25) or TRAP(*)
            let name = inner.split(',').next().unwrap_or("").trim();
            rule.trap = Some(trap_name_to_id(name));
        } else if d.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()) {
            // bare FEAT name e.g. DEEP_WATER, TREE, RUBBLE
            if let Some(fid) = feat_name_to_id(d) {
                rule.feat = Some(fid);
            }
        }
    }
    Some((letter, rule))
}

fn split_directives(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ':' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_mon(inner: &str) -> MonSpec {
    let which = inner.split(',').next().unwrap_or("*").trim();
    if which == "*" {
        MonSpec::Any
    } else if which.len() == 1 {
        MonSpec::Glyph(which.chars().next().unwrap())
    } else {
        // HOUND / UNDEAD summon-ish or name — treat as name contains
        MonSpec::Name(which.to_string())
    }
}

fn parse_obj(inner: &str) -> ObjSpec {
    let which = inner.split(',').next().unwrap_or("*").trim();
    if which == "*" {
        ObjSpec::Any
    } else {
        ObjSpec::Name(which.to_string())
    }
}

fn feat_name_to_id(name: &str) -> Option<u16> {
    match name {
        "FLOOR" => Some(id::FLOOR),
        "OPEN_DOOR" => Some(id::OPEN_DOOR),
        "BROKEN_DOOR" => Some(id::BROKEN_DOOR),
        "UP_STAIR" => Some(id::UP_STAIR),
        "DOWN_STAIR" => Some(id::DOWN_STAIR),
        "CLOSED_DOOR" => Some(id::CLOSED_DOOR),
        "SECRET_DOOR" => Some(id::SECRET_DOOR),
        "RUBBLE" => Some(id::RUBBLE),
        "MAGMA_VEIN" => Some(id::MAGMA_VEIN),
        "QUARTZ_VEIN" => Some(id::QUARTZ_VEIN),
        "GRANITE" => Some(id::GRANITE),
        "GRANITE_INNER" => Some(id::GRANITE_INNER),
        "GRANITE_OUTER" => Some(id::GRANITE_OUTER),
        "GRANITE_SOLID" => Some(id::GRANITE_SOLID),
        "PERMANENT" => Some(id::PERMANENT),
        "DEEP_WATER" => Some(id::DEEP_WATER),
        "SHALLOW_WATER" => Some(id::SHALLOW_WATER),
        "DEEP_LAVA" => Some(id::DEEP_LAVA),
        "SHALLOW_LAVA" => Some(id::SHALLOW_LAVA),
        "DARK_PIT" => Some(id::DARK_PIT),
        "DIRT" => Some(id::DIRT),
        "GRASS" => Some(id::GRASS),
        "BRAKE" => Some(id::BRAKE),
        "TREE" => Some(id::TREE),
        "MOUNTAIN" => Some(id::MOUNTAIN),
        _ => {
            // try f_info table by name
            let table = crate::f_info::table();
            for id in 0..=table.max_id() {
                if let Some(f) = table.get(id) {
                    if f.name.eq_ignore_ascii_case(name)
                        || f.name.replace(' ', "_").eq_ignore_ascii_case(name)
                    {
                        return Some(id);
                    }
                }
            }
            None
        }
    }
}

fn trap_name_to_id(name: &str) -> u16 {
    match name {
        "TRAP_TRAPDOOR" => id::TRAP_TRAPDOOR,
        "TRAP_PIT" => id::TRAP_PIT,
        "TRAP_SPIKED_PIT" => id::TRAP_SPIKED_PIT,
        "TRAP_POISON_PIT" => id::TRAP_POISON_PIT,
        "TRAP_TY_CURSE" => id::TRAP_TY_CURSE,
        "TRAP_TELEPORT" => id::TRAP_TELEPORT,
        "TRAP_FIRE" => id::TRAP_FIRE,
        "TRAP_ACID" => id::TRAP_ACID,
        "TRAP_SLOW" => id::TRAP_SLOW,
        "TRAP_SLEEP" => id::TRAP_SLEEP,
        "TRAP_OPEN" | "*" => id::TRAP_PIT,
        _ => id::TRAP_PIT,
    }
}

/// Default letter → feat when no L: override (classic angband vault charset).
pub fn default_letter_feat(ch: char) -> u16 {
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

pub fn lesser_vaults() -> &'static [MapTemplate] {
    &tables().lesser
}
pub fn greater_vaults() -> &'static [MapTemplate] {
    &tables().greater
}
pub fn room_templates() -> &'static [MapTemplate] {
    &tables().rooms
}

pub trait TemplateRng {
    fn pick(&mut self, n: usize) -> usize;
    fn next_u64(&mut self) -> u64;
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

/// Stamp template: terrain + collect mon/obj spawns.
pub fn stamp_template(
    feats: &mut [u16],
    w: i32,
    h: i32,
    ox: i32,
    oy: i32,
    tmpl: &MapTemplate,
    rng: &mut impl TemplateRng,
    mons: &mut Vec<SpawnMon>,
    objs: &mut Vec<SpawnObj>,
) -> bool {
    let vw = tmpl.width();
    let vh = tmpl.height();
    if ox < 1 || oy < 1 || ox + vw >= w - 1 || oy + vh >= h - 1 {
        return false;
    }
    let rtab = r_info::table();
    let ktab = k_info::table();

    for (row_i, row) in tmpl.map.iter().enumerate() {
        for (col_i, ch) in row.chars().enumerate() {
            let x = ox + col_i as i32;
            let y = oy + row_i as i32;
            let i = (y * w + x) as usize;
            if feats[i] == id::PERMANENT {
                continue;
            }

            let rule = tmpl.letters.get(&ch);
            // terrain
            let feat = if let Some(r) = rule {
                if let Some(fid) = r.feat {
                    fid
                } else if let Some(tid) = r.trap {
                    tid
                } else {
                    // mon/obj squares default to floor unless letter has default terrain meaning
                    if r.mon.is_some() || r.obj.is_some() {
                        id::FLOOR
                    } else {
                        default_letter_feat(ch)
                    }
                }
            } else {
                default_letter_feat(ch)
            };
            feats[i] = feat;

            // monster
            if let Some(r) = rule {
                if let Some(spec) = &r.mon {
                    let race = match spec {
                        MonSpec::Any => rtab.pick_any(rng.next_u64() as usize),
                        MonSpec::Glyph(g) => rtab.pick_by_glyph(*g, rng.next_u64() as usize),
                        MonSpec::Name(n) => rtab
                            .find_name_contains(n)
                            .or_else(|| rtab.pick_any(rng.next_u64() as usize)),
                    };
                    if let Some(race) = race {
                        mons.push(SpawnMon {
                            x,
                            y,
                            race_id: race.id,
                            glyph: race.glyph,
                            color: race.color,
                            name: race.name.clone(),
                        });
                    }
                }
                if let Some(spec) = &r.obj {
                    let kind = match spec {
                        ObjSpec::Any => ktab.pick_any(rng.next_u64() as usize),
                        ObjSpec::Name(n) => {
                            if n.eq_ignore_ascii_case("GOLD") {
                                ktab.find_name_contains("gold")
                                    .or_else(|| ktab.pick_any(rng.next_u64() as usize))
                            } else {
                                ktab.find_name_contains(n)
                                    .or_else(|| ktab.pick_any(rng.next_u64() as usize))
                            }
                        }
                    };
                    if let Some(kind) = kind {
                        objs.push(SpawnObj {
                            x,
                            y,
                            kind_id: kind.id,
                            glyph: kind.glyph,
                            color: kind.color,
                            name: kind.name.clone(),
                        });
                    }
                }
            } else {
                // no L: rule — monster-ish letters by glyph convention
                // digits often objects in vaults; letters often monsters — floor already set
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_templates_and_l_rules() {
        assert!(lesser_vaults().len() >= 50);
        assert!(greater_vaults().len() >= 50);
        assert!(room_templates().len() >= 50);
        // rooms.txt has L:C:MON(...)
        let has_l = room_templates().iter().any(|t| !t.letters.is_empty());
        assert!(has_l, "expected L: rules on some room templates");
    }
}
