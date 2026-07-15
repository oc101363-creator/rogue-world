//! Terrain features — Frog `f_info.txt` core set (glyph + color + MOVE).
//!
//! Not a full 188-feature dump: every entry here maps to a real f_info id/name/glyph.
//! Flags are only what ASK needs (walk / build / permanent).

use serde::{Deserialize, Serialize};

/// Angband 16-color letter from f_info `G:glyph:color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatColor {
    Black,
    White,
    Gray,
    Orange,
    Red,
    Green,
    Blue,
    Brown,
    DarkGray,
    LightGray,
    Violet,
    Yellow,
    LightRed,
    LightGreen,
    LightBlue,
    LightBrown,
}

impl FeatColor {
    /// CSS hex for the viewer (close to classic terminal palette).
    pub fn css(self) -> &'static str {
        match self {
            Self::Black => "#000000",
            Self::White => "#e8e8e8",
            Self::Gray => "#808080",
            Self::Orange => "#ff7f00",
            Self::Red => "#c41e3a",
            Self::Green => "#228b22",
            Self::Blue => "#1e90ff",
            Self::Brown => "#8b4513",
            Self::DarkGray => "#404040",
            Self::LightGray => "#c0c0c0",
            Self::Violet => "#c44cff",
            Self::Yellow => "#ffd700",
            Self::LightRed => "#ff6b6b",
            Self::LightGreen => "#90ee90",
            Self::LightBlue => "#87cefa",
            Self::LightBrown => "#deb887",
        }
    }

    /// Parse frog color letter (D w s o r g b u d W v y R G B U).
    pub fn from_frog(c: char) -> Self {
        match c {
            'D' => Self::Black,
            'w' => Self::White,
            's' => Self::Gray,
            'o' => Self::Orange,
            'r' => Self::Red,
            'g' => Self::Green,
            'b' => Self::Blue,
            'u' => Self::Brown,
            'd' => Self::DarkGray,
            'W' => Self::LightGray,
            'v' => Self::Violet,
            'y' => Self::Yellow,
            'R' => Self::LightRed,
            'G' => Self::LightGreen,
            'B' => Self::LightBlue,
            'U' => Self::LightBrown,
            _ => Self::White,
        }
    }
}

/// Core terrain kinds used in generation + movement.
/// Ids comment = frog f_info N: number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Feat {
    Floor = 1,           // N:1 FLOOR .
    OpenDoor = 4,        // N:4 '
    UpStair = 6,         // N:6 <
    DownStair = 7,       // N:7 >
    ClosedDoor = 32,     // N:32 +
    Rubble = 49,         // N:49 :
    MagmaVein = 50,      // N:50 %
    QuartzVein = 51,     // N:51 %
    MagmaTreasure = 54,  // N:54 *
    QuartzTreasure = 55, // N:55 *
    Granite = 56,        // N:56 #
    GraniteOuter = 58,   // N:58 # (room edge)
    Permanent = 60,      // N:60 #
    DeepWater = 83,      // N:83 ~
    ShallowWater = 84,   // N:84 ~
    DeepLava = 85,       // N:85 ~
    ShallowLava = 86,    // N:86 ~
    Dirt = 88,           // N:88 .
    Grass = 89,          // N:89 .
    Tree = 96,           // N:96 (terrain tree; entities may still use T)
    Mountain = 97,       // N:97
}

impl Feat {
    pub fn glyph(self) -> char {
        match self {
            Self::Floor | Self::Dirt | Self::Grass => '.',
            Self::OpenDoor => '\'',
            Self::UpStair => '<',
            Self::DownStair => '>',
            Self::ClosedDoor => '+',
            Self::Rubble => ':',
            Self::MagmaVein | Self::QuartzVein => '%',
            Self::MagmaTreasure | Self::QuartzTreasure => '*',
            Self::Granite | Self::GraniteOuter | Self::Permanent | Self::Mountain => '#',
            Self::DeepWater | Self::ShallowWater | Self::DeepLava | Self::ShallowLava => '~',
            Self::Tree => 'T',
        }
    }

    pub fn color(self) -> FeatColor {
        match self {
            Self::Floor => FeatColor::from_frog('w'),
            Self::OpenDoor => FeatColor::from_frog('U'),
            Self::UpStair | Self::DownStair => FeatColor::from_frog('w'),
            Self::ClosedDoor => FeatColor::from_frog('U'),
            Self::Rubble => FeatColor::from_frog('w'),
            Self::MagmaVein => FeatColor::from_frog('s'),
            Self::QuartzVein => FeatColor::from_frog('w'),
            Self::MagmaTreasure | Self::QuartzTreasure => FeatColor::from_frog('o'),
            Self::Granite | Self::GraniteOuter => FeatColor::from_frog('w'),
            Self::Permanent => FeatColor::from_frog('U'),
            Self::DeepWater => FeatColor::from_frog('b'),
            Self::ShallowWater => FeatColor::from_frog('B'),
            Self::DeepLava => FeatColor::from_frog('r'),
            Self::ShallowLava => FeatColor::from_frog('R'),
            Self::Dirt => FeatColor::from_frog('u'),
            Self::Grass => FeatColor::from_frog('g'),
            Self::Tree => FeatColor::from_frog('g'),
            Self::Mountain => FeatColor::from_frog('U'),
        }
    }

    /// Frog F:MOVE — agent can enter.
    pub fn walk(self) -> bool {
        matches!(
            self,
            Self::Floor
                | Self::OpenDoor
                | Self::UpStair
                | Self::DownStair
                | Self::Dirt
                | Self::Grass
                | Self::ShallowWater
                | Self::ShallowLava
                | Self::Tree
        )
    }

    /// Place buildings (ASK rule: dry walkable floor-like).
    pub fn build(self) -> bool {
        matches!(self, Self::Floor | Self::Dirt | Self::Grass)
    }

    pub fn is_wall(self) -> bool {
        !self.walk()
    }
}

/// Snapshot row with per-cell colors for the viewer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileRow {
    pub glyphs: String,
    /// Parallel color CSS strings, same length as glyphs.
    pub colors: Vec<String>,
    pub bg: Vec<String>,
}
