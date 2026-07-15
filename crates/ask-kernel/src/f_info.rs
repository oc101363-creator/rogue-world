//! Load FrogComposband `f_info.txt` (full table).
//!
//! Format (per entry):
//!   N:id:NAME
//!   G:glyph:color[:LIT]
//!   F:FLAG | FLAG | ...
//!
//! Grid cells store `feat_id: u16` exactly as frog N numbers.

use std::sync::OnceLock;

/// Embedded copy of frog `lib/edit/f_info.txt`.
const F_INFO_TXT: &str = include_str!("../data/f_info.txt");

#[derive(Clone, Debug)]
pub struct FeatInfo {
    pub id: u16,
    pub name: String,
    pub glyph: char,
    /// Frog color letter (w, s, g, …)
    pub color: char,
    pub walk: bool,
    pub wall: bool,
    pub permanent: bool,
    pub door: bool,
    pub stairs: bool,
    pub trap: bool,
    pub water: bool,
    pub lava: bool,
    pub tree: bool,
    pub less: bool, // up stairs
    pub more: bool, // down stairs
}

#[derive(Clone, Debug)]
pub struct FeatTable {
    /// Indexed by feat id; missing ids are None.
    by_id: Vec<Option<FeatInfo>>,
}

impl FeatTable {
    pub fn get(&self, id: u16) -> Option<&FeatInfo> {
        self.by_id.get(id as usize).and_then(|x| x.as_ref())
    }

    pub fn glyph(&self, id: u16) -> char {
        self.get(id).map(|f| f.glyph).unwrap_or('?')
    }

    pub fn color_letter(&self, id: u16) -> char {
        self.get(id).map(|f| f.color).unwrap_or('w')
    }

    pub fn walk(&self, id: u16) -> bool {
        self.get(id).map(|f| f.walk).unwrap_or(false)
    }

    pub fn buildable(&self, id: u16) -> bool {
        // ASK: dry-ish floors for huts
        let Some(f) = self.get(id) else {
            return false;
        };
        f.walk && !f.water && !f.lava && !f.door && !f.stairs && !f.trap
    }

    pub fn is_trap(&self, id: u16) -> bool {
        self.get(id).map(|f| f.trap).unwrap_or(false)
    }

    pub fn is_closed_door(&self, id: u16) -> bool {
        matches!(id, id::CLOSED_DOOR | id::SECRET_DOOR)
            || self.get(id).map(|f| f.door && !f.walk).unwrap_or(false)
    }

    pub fn is_open_door(&self, id: u16) -> bool {
        matches!(id, id::OPEN_DOOR | id::BROKEN_DOOR)
            || self.get(id).map(|f| f.door && f.walk).unwrap_or(false)
    }

    pub fn count(&self) -> usize {
        self.by_id.iter().filter(|x| x.is_some()).count()
    }

    pub fn max_id(&self) -> u16 {
        self.by_id
            .iter()
            .rposition(|x| x.is_some())
            .map(|i| i as u16)
            .unwrap_or(0)
    }
}

/// Frog 16-color letter → CSS.
pub fn color_css(letter: char) -> &'static str {
    match letter {
        'D' => "#000000",
        'w' => "#e8e8e8",
        's' => "#808080",
        'o' => "#ff7f00",
        'r' => "#c41e3a",
        'g' => "#228b22",
        'b' => "#1e90ff",
        'u' => "#8b4513",
        'd' => "#404040",
        'W' => "#c0c0c0",
        'v' => "#c44cff",
        'y' => "#ffd700",
        'R' => "#ff6b6b",
        'G' => "#90ee90",
        'B' => "#87cefa",
        'U' => "#deb887",
        // frog sometimes uses L (e.g. TREE G:#:L) — treat as leafy green
        'L' | 'l' => "#3cb371",
        _ => "#cccccc",
    }
}

pub fn bg_css(letter: char, walk: bool, water: bool, lava: bool) -> &'static str {
    if lava {
        return "#2a0808";
    }
    if water {
        return "#0a1520";
    }
    if !walk {
        return "#0e0e10";
    }
    match letter {
        'g' | 'G' => "#0a160a",
        'u' | 'U' => "#16120a",
        'b' | 'B' => "#0a1218",
        _ => "#0c100c",
    }
}

/// Well-known feat ids from f_info (for generation).
pub mod id {
    pub const FLOOR: u16 = 1;
    pub const OPEN_DOOR: u16 = 4;
    pub const BROKEN_DOOR: u16 = 5;
    pub const UP_STAIR: u16 = 6;
    pub const DOWN_STAIR: u16 = 7;
    pub const CLOSED_DOOR: u16 = 32;
    pub const SECRET_DOOR: u16 = 48;
    pub const RUBBLE: u16 = 49;
    pub const MAGMA_VEIN: u16 = 50;
    pub const QUARTZ_VEIN: u16 = 51;
    pub const MAGMA_TREASURE: u16 = 54;
    pub const QUARTZ_TREASURE: u16 = 55;
    pub const GRANITE: u16 = 56;
    pub const GRANITE_INNER: u16 = 57;
    pub const GRANITE_OUTER: u16 = 58;
    pub const GRANITE_SOLID: u16 = 59;
    pub const PERMANENT: u16 = 60;
    pub const DEEP_WATER: u16 = 83;
    pub const SHALLOW_WATER: u16 = 84;
    pub const DEEP_LAVA: u16 = 85;
    pub const SHALLOW_LAVA: u16 = 86;
    pub const DARK_PIT: u16 = 87;
    pub const DIRT: u16 = 88;
    pub const GRASS: u16 = 89;
    pub const BRAKE: u16 = 94;
    pub const TREE: u16 = 96;
    pub const MOUNTAIN: u16 = 97;

    // Visible traps (f_info N:16–31) — frog place_trap / choose_random_trap
    pub const TRAP_TRAPDOOR: u16 = 16;
    pub const TRAP_PIT: u16 = 17;
    pub const TRAP_SPIKED_PIT: u16 = 18;
    pub const TRAP_POISON_PIT: u16 = 19;
    pub const TRAP_TY_CURSE: u16 = 20;
    pub const TRAP_TELEPORT: u16 = 21;
    pub const TRAP_FIRE: u16 = 22;
    pub const TRAP_ACID: u16 = 23;
    pub const TRAP_SLOW: u16 = 24;
    pub const TRAP_LOSE_STR: u16 = 25;
    pub const TRAP_LOSE_DEX: u16 = 26;
    pub const TRAP_LOSE_CON: u16 = 27;
    pub const TRAP_BLIND: u16 = 28;
    pub const TRAP_CONFUSE: u16 = 29;
    pub const TRAP_POISON: u16 = 30;
    pub const TRAP_SLEEP: u16 = 31;

    pub const TRAP_FEATS: [u16; 16] = [
        TRAP_TRAPDOOR,
        TRAP_PIT,
        TRAP_SPIKED_PIT,
        TRAP_POISON_PIT,
        TRAP_TY_CURSE,
        TRAP_TELEPORT,
        TRAP_FIRE,
        TRAP_ACID,
        TRAP_SLOW,
        TRAP_LOSE_STR,
        TRAP_LOSE_DEX,
        TRAP_LOSE_CON,
        TRAP_BLIND,
        TRAP_CONFUSE,
        TRAP_POISON,
        TRAP_SLEEP,
    ];
}

static TABLE: OnceLock<FeatTable> = OnceLock::new();

pub fn table() -> &'static FeatTable {
    TABLE.get_or_init(parse_f_info)
}

fn parse_f_info() -> FeatTable {
    let mut by_id: Vec<Option<FeatInfo>> = Vec::new();
    let mut cur_id: Option<u16> = None;
    let mut cur_name = String::new();
    let mut cur_glyph = '?';
    let mut cur_color = 'w';
    let mut flags = String::new();

    let flush = |by_id: &mut Vec<Option<FeatInfo>>,
                 id: u16,
                 name: String,
                 glyph: char,
                 color: char,
                 flags: &str| {
        let up = flags.to_ascii_uppercase();
        let walk = up.contains("MOVE");
        let wall = up.contains("WALL");
        let permanent = up.contains("PERMANENT");
        let door = up.contains("DOOR");
        let stairs = up.contains("STAIRS");
        let trap = up.contains("TRAP") || up.contains("HIT_TRAP");
        let water = up.contains("WATER") || name.to_ascii_uppercase().contains("WATER");
        let lava = up.contains("LAVA") || name.to_ascii_uppercase().contains("LAVA");
        let tree = up.contains("TREE") || name.to_ascii_uppercase().contains("TREE");
        let less = up.contains("LESS");
        let more = up.contains("MORE");
        // shallow water still MOVE in frog; deep water often not — trust F:MOVE only for walk
        let info = FeatInfo {
            id,
            name,
            glyph,
            color,
            walk,
            wall,
            permanent,
            door,
            stairs,
            trap,
            water,
            lava,
            tree,
            less,
            more,
        };
        let idx = id as usize;
        if by_id.len() <= idx {
            by_id.resize_with(idx + 1, || None);
        }
        by_id[idx] = Some(info);
    };

    for line in F_INFO_TXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("N:") {
            if let Some(id) = cur_id.take() {
                flush(
                    &mut by_id,
                    id,
                    std::mem::take(&mut cur_name),
                    cur_glyph,
                    cur_color,
                    &flags,
                );
                flags.clear();
                cur_glyph = '?';
                cur_color = 'w';
            }
            // N:id:NAME
            let mut parts = rest.splitn(2, ':');
            let id: u16 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            cur_name = parts.next().unwrap_or("").to_string();
            cur_id = Some(id);
        } else if let Some(rest) = line.strip_prefix("G:") {
            // G:glyph:color or G:glyph:color:LIT
            let mut chars = rest.chars();
            cur_glyph = chars.next().unwrap_or('?');
            // skip optional ':'
            let rest = rest.get(1..).unwrap_or("");
            let rest = rest.strip_prefix(':').unwrap_or(rest);
            cur_color = rest.chars().next().unwrap_or('w');
        } else if let Some(rest) = line.strip_prefix("F:") {
            if !flags.is_empty() {
                flags.push(' ');
            }
            flags.push_str(rest);
        }
    }
    if let Some(id) = cur_id.take() {
        flush(
            &mut by_id,
            id,
            std::mem::take(&mut cur_name),
            cur_glyph,
            cur_color,
            &flags,
        );
    }

    FeatTable { by_id }
}

/// Serializable cell is just frog feat id.
pub type FeatId = u16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_full_f_info() {
        let t = table();
        assert!(t.count() >= 180, "count={}", t.count());
        let floor = t.get(id::FLOOR).unwrap();
        assert_eq!(floor.glyph, '.');
        assert!(floor.walk);
        let granite = t.get(id::GRANITE).unwrap();
        assert_eq!(granite.glyph, '#');
        assert!(!granite.walk);
        let water = t.get(id::DEEP_WATER).unwrap();
        assert_eq!(water.glyph, '~');
    }
}
