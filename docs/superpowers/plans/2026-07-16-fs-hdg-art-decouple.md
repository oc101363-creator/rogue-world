# FS-HDG Art Decouple Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple simulation identity from presentation so the web client can ship an FS-HDG material-driven art system without bloating backend systems.

**Architecture:** Grid and entities stay identity-only (`FeatId`, `StableId`, template ids). A pure-data **Art Catalog** is compiled from `f_info`/`r_info`/`k_info` plus an optional overlay TOML. Snapshots gain compact `feat_ids` + `catalog_version`. The frontend loads `/api/art` once and paints materials/glyphs itself. `systems/*` never import art.

**Tech Stack:** Rust (bevy_ecs, serde, axum, base64), TOML overlay, vanilla JS + ROT.js (kept initially as char blit), FS-HDG palette.

**Design doc:** `docs/superpowers/specs/2026-07-16-fs-hdg-art-decouple-design.md`

## Global Constraints

- `systems/*` must not reference art, materials, glyphs for presentation, or color CSS.
- Simulation continues to use `FeatId` and flag helpers only.
- Snapshot remains FOV-gated; unseen cells must paint as void client-side even if `feat_ids` are present.
- Max message length and messaging/auth systems are out of scope.
- Toolchain: `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" cargo …`
- Keep legacy `tiles`/`tile_colors` for one cycle (terminal + old clients).
- Catalog `catalog_version` bumps when overlay or baseline mapping changes.

## File Structure

| Path | Responsibility |
|------|----------------|
| `crates/ask-kernel/src/art.rs` | Catalog types, baseline mapping, overlay merge, encode helpers |
| `crates/ask-kernel/data/art/fs_hdg_overlay.toml` | Manual glyph/material overrides |
| `crates/ask-kernel/src/viewer.rs` | Emit `feat_ids` + `catalog_version`; entity template ids |
| `crates/ask-kernel/src/serve.rs` | `GET /api/art` |
| `crates/ask-kernel/src/lib.rs` | `pub mod art` |
| `crates/ask-kernel/static/art.js` | Client catalog cache + material resolve |
| `crates/ask-kernel/static/themes.js` | FS-HDG material themes (replace letter-only themes) |
| `crates/ask-kernel/static/app.js` | Render via catalog; chrome hooks |
| `crates/ask-kernel/static/app.css` | FS-HDG nested panels, square cells, double borders |
| `crates/ask-kernel/static/index.html` | Fonts, panel structure, cache bust |
| `crates/ask-kernel/tests/art_catalog.rs` | Catalog completeness + overlay tests |

---

### Task 1: Art catalog module + overlay data

**Files:**
- Create: `crates/ask-kernel/src/art.rs`
- Create: `crates/ask-kernel/data/art/fs_hdg_overlay.toml`
- Modify: `crates/ask-kernel/src/lib.rs`
- Modify: `crates/ask-kernel/Cargo.toml` (add `base64` if missing)
- Test: unit tests inside `art.rs`

**Interfaces:**
- Consumes: `f_info::table()`, `r_info::table()`, `k_info::table()`
- Produces:
  - `pub struct ArtCatalog { pub version: u32, … }`
  - `pub fn catalog() -> &'static ArtCatalog`
  - `pub fn encode_feat_ids_b64(cells: &[u16]) -> String`
  - `pub fn material_for_feat(info: &FeatInfo) -> &'static str` (baseline heuristic)

- [ ] **Step 1: Ensure base64 dependency**

In workspace or crate `Cargo.toml`, add:

```toml
base64 = "0.22"
```

If workspace already has it, use workspace dep. Check:

```bash
rg -n "base64" Cargo.toml crates/ask-kernel/Cargo.toml
```

- [ ] **Step 2: Write failing catalog test**

Create `crates/ask-kernel/src/art.rs` with tests only first (or empty module + tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_floor_and_granite() {
        let c = catalog();
        let floor = c.feats.get(&1).expect("FLOOR id 1");
        assert_eq!(floor.glyph, '.');
        assert!(!floor.material.is_empty());
        let gran = c.feats.get(&56).expect("GRANITE id 56");
        assert_eq!(gran.material, "granite");
    }

    #[test]
    fn encode_roundtrip_len() {
        let cells = vec![1u16, 56, 83, 0];
        let b64 = encode_feat_ids_b64(&cells);
        assert!(!b64.is_empty());
        let raw = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &b64,
        )
        .unwrap();
        assert_eq!(raw.len(), cells.len() * 2);
    }
}
```

Add `pub mod art;` to `lib.rs`.

- [ ] **Step 3: Run test to verify it fails**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo test -p ask-kernel catalog_has_floor -- --nocapture
```

Expected: compile error (`catalog` not found) or FAIL.

- [ ] **Step 4: Implement catalog + overlay**

`crates/ask-kernel/data/art/fs_hdg_overlay.toml`:

```toml
catalog_version = 1

[materials]
basalt = "#555555"
granite = "#AAAAAA"
gold = "#FFD700"
aquifer = "#0055FF"
magma = "#FF4500"
organic = "#8B5A2B"
void = "#000000"
ui_primary = "#00FF66"
ui_warning = "#FFCC00"
ui_danger = "#FF3333"
ui_info = "#00E5FF"
text_white = "#FFFFFF"
depth_shadow = "#2A2A2A"

# Optional per-feat overrides (id as string key)
[feats.96]
glyph = "♣"
material = "organic"

[feats.83]
glyph = "≈"
material = "aquifer"

[feats.85]
glyph = "≈"
material = "magma"
```

`crates/ask-kernel/src/art.rs` implementation sketch:

```rust
//! Presentation catalog — pure data. systems/* must not import this for gameplay.

use std::collections::HashMap;
use std::sync::OnceLock;

use base64::Engine;
use serde::Serialize;

use crate::f_info::{self, FeatInfo};
use crate::k_info;
use crate::r_info;

const OVERLAY_TOML: &str = include_str!("../data/art/fs_hdg_overlay.toml");

#[derive(Clone, Debug, Serialize)]
pub struct FeatLook {
    pub glyph: char,
    pub material: String,
    pub layer: u8,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EntityLook {
    pub glyph: char,
    pub material: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtCatalog {
    pub catalog_version: u32,
    pub materials: HashMap<String, String>,
    pub feats: HashMap<u16, FeatLook>,
    pub races: HashMap<u16, EntityLook>,
    pub objects: HashMap<u16, EntityLook>,
    pub entity_defaults: HashMap<String, EntityLook>,
}

#[derive(Debug, Deserialize)]
struct OverlayFile {
    catalog_version: u32,
    materials: HashMap<String, String>,
    #[serde(default)]
    feats: HashMap<String, OverlayFeat>,
}

#[derive(Debug, Deserialize)]
struct OverlayFeat {
    glyph: Option<String>,
    material: Option<String>,
    layer: Option<u8>,
}

use serde::Deserialize;

fn baseline_material(info: &FeatInfo) -> &'static str {
    if info.lava {
        "magma"
    } else if info.water {
        "aquifer"
    } else if info.tree {
        "organic"
    } else if info.wall || info.name.to_ascii_uppercase().contains("GRANITE") {
        "granite"
    } else if info.name.to_ascii_uppercase().contains("DIRT")
        || info.name.to_ascii_uppercase().contains("GRASS")
        || info.name.to_ascii_uppercase().contains("SOIL")
    {
        "organic"
    } else if info.name.to_ascii_uppercase().contains("GOLD")
        || info.name.to_ascii_uppercase().contains("TREASURE")
    {
        "gold"
    } else if info.wall {
        "granite"
    } else {
        "basalt"
    }
}

fn build_catalog() -> ArtCatalog {
    let overlay: OverlayFile = toml::from_str(OVERLAY_TOML).expect("art overlay");
    let mut materials = overlay.materials;
    // ensure required keys exist
    for (k, v) in [
        ("basalt", "#555555"),
        ("granite", "#AAAAAA"),
        ("gold", "#FFD700"),
        ("aquifer", "#0055FF"),
        ("magma", "#FF4500"),
        ("organic", "#8B5A2B"),
        ("void", "#000000"),
        ("ui_primary", "#00FF66"),
        ("ui_warning", "#FFCC00"),
        ("ui_danger", "#FF3333"),
        ("ui_info", "#00E5FF"),
        ("text_white", "#FFFFFF"),
    ] {
        materials.entry(k.into()).or_insert_with(|| v.into());
    }

    let mut feats = HashMap::new();
    let table = f_info::table();
    for id in 0..=table.max_id() {
        let Some(info) = table.get(id) else { continue };
        let mut look = FeatLook {
            glyph: info.glyph,
            material: baseline_material(info).into(),
            layer: 0,
            name: info.name.clone(),
        };
        if let Some(ov) = overlay.feats.get(&id.to_string()) {
            if let Some(g) = &ov.glyph {
                look.glyph = g.chars().next().unwrap_or(look.glyph);
            }
            if let Some(m) = &ov.material {
                look.material = m.clone();
            }
            if let Some(l) = ov.layer {
                look.layer = l;
            }
        }
        feats.insert(id, look);
    }

    // races / objects baseline from r_info / k_info — map glyph+name
    let mut races = HashMap::new();
    for (id, race) in r_info::table().iter() {
        races.insert(
            id,
            EntityLook {
                glyph: race.glyph,
                material: "ui_danger".into(),
                name: Some(race.name.clone()),
            },
        );
    }
    let mut objects = HashMap::new();
    for (id, obj) in k_info::table().iter() {
        objects.insert(
            id,
            EntityLook {
                glyph: obj.glyph,
                material: "ui_info".into(),
                name: Some(obj.name.clone()),
            },
        );
    }

    let mut entity_defaults = HashMap::new();
    entity_defaults.insert(
        "agent".into(),
        EntityLook {
            glyph: '@',
            material: "ui_warning".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "tree".into(),
        EntityLook {
            glyph: '♣',
            material: "organic".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "iron".into(),
        EntityLook {
            glyph: 'I',
            material: "granite".into(),
            name: None,
        },
    );
    entity_defaults.insert(
        "hut".into(),
        EntityLook {
            glyph: '⌂',
            material: "ui_warning".into(),
            name: None,
        },
    );

    ArtCatalog {
        catalog_version: overlay.catalog_version,
        materials,
        feats,
        races,
        objects,
        entity_defaults,
    }
}

pub fn catalog() -> &'static ArtCatalog {
    static C: OnceLock<ArtCatalog> = OnceLock::new();
    C.get_or_init(build_catalog)
}

pub fn encode_feat_ids_b64(cells: &[u16]) -> String {
    let mut bytes = Vec::with_capacity(cells.len() * 2);
    for &c in cells {
        bytes.extend_from_slice(&c.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
```

**Adapt** `r_info::table()` / `k_info::table()` iteration to whatever API those modules already expose (`.iter()`, `.by_id`, etc.). Read those modules and match existing patterns; do not invent fake iterators.

Add to `Cargo.toml` if needed:

```toml
toml = "0.8"
base64 = "0.22"
```

- [ ] **Step 5: Run tests**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo test -p ask-kernel art:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ask-kernel/src/art.rs crates/ask-kernel/src/lib.rs \
  crates/ask-kernel/data/art/fs_hdg_overlay.toml crates/ask-kernel/Cargo.toml Cargo.toml Cargo.lock
git commit -m "feat(art): decoupled art catalog from f_info + FS-HDG overlay"
```

---

### Task 2: Snapshot identity payload + `/api/art`

**Files:**
- Modify: `crates/ask-kernel/src/viewer.rs`
- Modify: `crates/ask-kernel/src/serve.rs`
- Modify: `crates/ask-kernel/src/components.rs` (only if Monster/Item fields need exposing — prefer read existing)
- Test: `crates/ask-kernel/tests/art_catalog.rs` or extend unit tests

**Interfaces:**
- Consumes: `art::catalog()`, `art::encode_feat_ids_b64`, `Grid.cells`
- Produces:
  - `ViewerSnapshot.catalog_version: u32`
  - `ViewerSnapshot.feat_ids: FeatIdsPayload`
  - `ViewerEntity.race_id: Option<u16>`, `kind_id: Option<u16>`
  - `GET /api/art` → JSON catalog

- [ ] **Step 1: Extend snapshot types**

In `viewer.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatIdsPayload {
    pub enc: &'static str, // "u16le_b64"
    pub w: i32,
    pub h: i32,
    pub data: String,
}

// On ViewerSnapshot add:
pub catalog_version: u32,
pub feat_ids: FeatIdsPayload,

// On ViewerEntity add:
#[serde(skip_serializing_if = "Option::is_none")]
pub race_id: Option<u16>,
#[serde(skip_serializing_if = "Option::is_none")]
pub kind_id: Option<u16>,
```

- [ ] **Step 2: Fill feat_ids in `build_viewer_snapshot_with`**

After reading grid:

```rust
use crate::art;

let feat_ids = FeatIdsPayload {
    enc: "u16le_b64",
    w: width,
    h: height,
    data: art::encode_feat_ids_b64(&grid.cells),
};
let catalog_version = art::catalog().catalog_version;
```

Include in returned `ViewerSnapshot { …, catalog_version, feat_ids, … }`.

Still fill legacy `tiles`/`tile_colors` exactly as today.

- [ ] **Step 3: Entity template ids**

When pushing monsters:

```rust
race_id: Some(m.race_id),
kind_id: None,
```

When pushing items, if `Matter::Object { kind_id, .. }`:

```rust
kind_id: Some(kind_id),
race_id: None,
```

Agents/trees/huts: both `None` (use `entity_defaults`).

Set `race_id: None, kind_id: None` on all other entity constructions so the struct compiles.

- [ ] **Step 4: `/api/art` route**

In `serve.rs`:

```rust
.route("/api/art", get(api_art))

async fn api_art() -> impl IntoResponse {
    let c = crate::art::catalog();
    // Serialize with string keys for feats map if needed
    Json(serde_json::json!({
        "ok": true,
        "catalog_version": c.catalog_version,
        "materials": c.materials,
        "feats": c.feats, // if HashMap<u16,_> serializes as string keys in serde_json, OK
        "races": c.races,
        "objects": c.objects,
        "entity_defaults": c.entity_defaults,
    }))
}
```

If `HashMap<u16, _>` is awkward in JSON, convert to `HashMap<String, _>` in a `fn catalog_json() -> serde_json::Value`.

Log the new route in startup eprintln.

- [ ] **Step 5: Integration test**

```rust
// crates/ask-kernel/tests/art_snapshot.rs
use ask_kernel::config::Config;
use ask_kernel::viewer::build_viewer_snapshot;
use ask_kernel::world::KernelWorld;

#[test]
fn snapshot_includes_feat_ids_and_catalog_version() {
    let mut kw = KernelWorld::new(&Config::default());
    let snap = build_viewer_snapshot(&mut kw.world, &[]);
    assert!(snap.catalog_version >= 1);
    assert_eq!(snap.feat_ids.enc, "u16le_b64");
    assert_eq!(snap.feat_ids.w, snap.width);
    assert_eq!(snap.feat_ids.h, snap.height);
    assert!(!snap.feat_ids.data.is_empty());
    // legacy still present
    assert_eq!(snap.tiles.len() as i32, snap.height);
}
```

- [ ] **Step 6: Run tests**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo test -p ask-kernel --all-targets
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/ask-kernel/src/viewer.rs crates/ask-kernel/src/serve.rs \
  crates/ask-kernel/tests/art_snapshot.rs
git commit -m "feat(viewer): identity feat_ids in snapshot + GET /api/art"
```

---

### Task 3: Frontend art runtime (`art.js`)

**Files:**
- Create: `crates/ask-kernel/static/art.js`
- Modify: `crates/ask-kernel/static/index.html` (script tag)

**Interfaces:**
- Consumes: `GET /api/art`, snapshot `feat_ids`, `vision`, `entities`
- Produces:
  - `async function ensureArtCatalog()`
  - `function decodeFeatIds(payload) -> Uint16Array`
  - `function lookForFeat(id) -> {glyph, material, layer, name}`
  - `function lookForEntity(ent) -> {glyph, material}`
  - `function materialColor(material, theme) -> css`

- [ ] **Step 1: Implement `art.js`**

```javascript
/* Art catalog client — identity → presentation */

let artCatalog = null;

async function ensureArtCatalog(versionHint) {
  if (
    artCatalog &&
    (versionHint == null || artCatalog.catalog_version === versionHint)
  ) {
    return artCatalog;
  }
  const r = await fetch("/api/art");
  const d = await r.json();
  if (!d.ok) throw new Error("art catalog failed");
  artCatalog = d;
  return artCatalog;
}

function decodeFeatIds(payload) {
  if (!payload || payload.enc !== "u16le_b64") {
    return null;
  }
  const bin = atob(payload.data);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const n = bytes.length / 2;
  const out = new Uint16Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = bytes[i * 2] | (bytes[i * 2 + 1] << 8);
  }
  return out;
}

function featKey(id) {
  return String(id);
}

function lookForFeat(id) {
  const c = artCatalog;
  if (!c) return { glyph: "?", material: "void", layer: 0, name: "?" };
  const f = c.feats[featKey(id)] || c.feats[id];
  if (!f) return { glyph: "?", material: "basalt", layer: 0, name: "#" + id };
  return f;
}

function lookForEntity(ent) {
  const c = artCatalog;
  if (!c) return { glyph: ent.glyph || "?", material: "ui_info" };
  if (ent.kind === "monster" && ent.race_id != null && c.races) {
    const r = c.races[featKey(ent.race_id)] || c.races[ent.race_id];
    if (r) return r;
  }
  if (ent.kind === "item" && ent.kind_id != null && c.objects) {
    const o = c.objects[featKey(ent.kind_id)] || c.objects[ent.kind_id];
    if (o) return o;
  }
  const d = c.entity_defaults && c.entity_defaults[ent.kind];
  if (d) return d;
  return { glyph: ent.glyph || "?", material: "ui_info" };
}

function materialColor(material, theme) {
  if (theme && theme.materials && theme.materials[material]) {
    return theme.materials[material];
  }
  if (artCatalog && artCatalog.materials && artCatalog.materials[material]) {
    return artCatalog.materials[material];
  }
  return "#ffffff";
}
```

- [ ] **Step 2: Load script in `index.html` before `app.js`**

```html
<script src="/static/art.js?v=fshdg1"></script>
<script src="/static/themes.js?v=fshdg1"></script>
<script src="/static/app.js?v=fshdg1"></script>
```

- [ ] **Step 3: Manual check**

Serve locally or hit remote after later deploy:

```bash
curl -s http://127.0.0.1:8000/api/art | head -c 200
```

- [ ] **Step 4: Commit**

```bash
git add crates/ask-kernel/static/art.js crates/ask-kernel/static/index.html
git commit -m "feat(ui): art.js catalog client + feat_id decoder"
```

---

### Task 4: FS-HDG themes (material-based)

**Files:**
- Modify: `crates/ask-kernel/static/themes.js`

**Interfaces:**
- Consumes: material names from catalog
- Produces: `Theme` with `materials: Record<string,string>`, `ui`, `void`, `memoryFactor`

- [ ] **Step 1: Rewrite theme shape**

```javascript
/**
 * FS-HDG material themes.
 * Themes remap material ids → CSS; they no longer remap frog 16-color letters
 * as the primary path (letter maps kept only as legacy fallback).
 */

/** @typedef {{
 *  id: string,
 *  name: string,
 *  ui: { bg: string, hud: string, hudMuted: string, online: string, offline: string, accent: string },
 *  void: string,
 *  materials: Record<string, string>,
 *  memoryFactor: number,
 *  selection: string,
 *  letters?: Record<string, string>,
 *  cellBg?: Function,
 *  entities?: object,
 * }} Theme */

const FS_HDG_BASE_MATERIALS = {
  basalt: "#555555",
  granite: "#AAAAAA",
  gold: "#FFD700",
  aquifer: "#0055FF",
  magma: "#FF4500",
  organic: "#8B5A2B",
  void: "#000000",
  ui_primary: "#00FF66",
  ui_warning: "#FFCC00",
  ui_danger: "#FF3333",
  ui_info: "#00E5FF",
  text_white: "#FFFFFF",
  depth_shadow: "#2A2A2A",
};

const THEMES = [
  {
    id: "fs-hdg",
    name: "FS-HDG",
    ui: {
      bg: "#000000",
      hud: "#FFFFFF",
      hudMuted: "#00FF66",
      online: "#00FF66",
      offline: "#FF3333",
      accent: "#00E5FF",
    },
    void: "#000000",
    materials: { ...FS_HDG_BASE_MATERIALS },
    memoryFactor: 0.4,
    selection: "#003333",
    // legacy fallbacks for old path during migration
    letters: {
      D: "#000000", d: "#1e1e1e", s: "#808080", w: "#ffffff", W: "#ffffff",
      b: "#0055FF", B: "#00E5FF", g: "#00FF66", G: "#00FF66",
      r: "#FF3333", R: "#FF4500", o: "#FFCC00", y: "#FFD700",
      u: "#8B5A2B", U: "#AAAAAA", v: "#00E5FF", L: "#00FF66", l: "#00E5FF",
    },
    cellBg() { return "#000000"; },
    entities: {
      agent: "#FFCC00", tree: "#8B5A2B", iron: "#AAAAAA", hut: "#FFCC00",
      monster: "#FF3333", item: "#00E5FF", entityBg: "#000000",
    },
  },
  // Keep one quiet alt theme (Catppuccin materials remapped)
  {
    id: "catppuccin-mocha",
    name: "Catppuccin Mocha",
    ui: {
      bg: "#1e1e2e",
      hud: "#cdd6f4",
      hudMuted: "#a6e3a1",
      online: "#a6e3a1",
      offline: "#f38ba8",
      accent: "#89b4fa",
    },
    void: "#1e1e2e",
    materials: {
      basalt: "#45475a",
      granite: "#bac2de",
      gold: "#f9e2af",
      aquifer: "#89b4fa",
      magma: "#fab387",
      organic: "#a6e3a1",
      void: "#11111b",
      ui_primary: "#a6e3a1",
      ui_warning: "#f9e2af",
      ui_danger: "#f38ba8",
      ui_info: "#89dceb",
      text_white: "#cdd6f4",
      depth_shadow: "#313244",
    },
    memoryFactor: 0.45,
    selection: "#313244",
    letters: { /* minimal legacy */ D: "#11111b", w: "#cdd6f4", s: "#6c7086",
      g: "#a6e3a1", r: "#f38ba8", b: "#89b4fa", o: "#fab387", y: "#f9e2af",
      u: "#cba6f7", d: "#313244", W: "#bac2de", G: "#94e2d5", R: "#eba0ac",
      B: "#89dceb", U: "#f5e0dc", v: "#cba6f7", L: "#a6e3a1", l: "#94e2d5" },
    cellBg() { return "#1e1e2e"; },
    entities: {
      agent: "#f9e2af", tree: "#a6e3a1", iron: "#89dceb", hut: "#fab387",
      monster: "#f38ba8", item: "#cba6f7", entityBg: "#313244",
    },
  },
];

function getTheme(id) {
  return THEMES.find((t) => t.id === id) || THEMES[0];
}
```

Default theme id in `app.js` localStorage fallback: `"fs-hdg"`.

- [ ] **Step 2: Commit**

```bash
git add crates/ask-kernel/static/themes.js
git commit -m "feat(ui): FS-HDG material theme as primary art standard"
```

---

### Task 5: Render path uses feat_ids + catalog

**Files:**
- Modify: `crates/ask-kernel/static/app.js` (`drawSnap`, `applySnapshot`, connect bootstrap)

**Interfaces:**
- Consumes: `ensureArtCatalog`, `decodeFeatIds`, `lookForFeat`, `lookForEntity`, `materialColor`
- Produces: map painted from identity; legacy path only if `feat_ids` missing

- [ ] **Step 1: Bootstrap catalog on connect**

In `connect` / startup:

```javascript
ensureArtCatalog().catch(() => pushLog("ART: catalog load failed"));
```

Default theme:

```javascript
let theme = getTheme(localStorage.getItem(THEME_KEY) || "fs-hdg");
```

- [ ] **Step 2: Rewrite terrain loop in `drawSnap`**

```javascript
function drawSnap(snap) {
  if (!snap) return;
  mapW = snap.width;
  mapH = snap.height;
  const d = syncViewSize();
  clampCamera();
  const x0 = cam.tx;
  const y0 = cam.ty;
  const visRows = snap.vision || [];
  const feats = decodeFeatIds(snap.feat_ids);
  const useIdentity = !!(feats && artCatalog);

  d.clear();
  for (let vy = 0; vy < viewRows; vy++) {
    const wy = y0 + vy;
    const visRow = visRows[wy] || "";
    for (let vx = 0; vx < viewCols; vx++) {
      const wx = x0 + vx;
      if (wy < 0 || wx < 0 || wy >= mapH || wx >= mapW) {
        d.draw(vx, vy, " ", theme.void, theme.void);
        continue;
      }
      const vch = visRow[wx] || " ";
      if (vch === " " || vch === "\0") {
        d.draw(vx, vy, " ", theme.void, theme.void);
        continue;
      }
      let ch, fg, bg;
      if (useIdentity) {
        const fid = feats[wy * mapW + wx];
        const look = lookForFeat(fid);
        ch = look.glyph || "?";
        fg = materialColor(look.material, theme);
        bg = theme.void;
        if (look.material === "aquifer") bg = "#001028";
        if (look.material === "magma") bg = "#1a0800";
        if (look.material === "organic") bg = "#0a1008";
      } else {
        // legacy letter path
        const row = snap.tiles[wy] || "";
        const colorRow = (snap.tile_colors || [])[wy] || "";
        ch = row[wx] || " ";
        const letter = colorRow[wx] || "w";
        fg = letterFg(letter);
        bg = theme.cellBg ? theme.cellBg(letter, ch) : theme.void;
      }
      if (vch === "m") {
        const f = theme.memoryFactor || 0.4;
        fg = dimColor(fg, f);
        bg = dimColor(bg, f + 0.05);
      }
      d.draw(vx, vy, ch, fg, bg);
    }
  }

  // entities via catalog
  const ents = snap.entities || [];
  for (const ent of ents) {
    const vx = ent.x - x0;
    const vy = ent.y - y0;
    if (vx < 0 || vy < 0 || vx >= viewCols || vy >= viewRows) continue;
    if (ent.kind === "agent") {
      const trackedIds = new Set(
        tracked.map((t) => t.agent_id).filter((id) => id != null),
      );
      // visible foreign agents already filtered server-side; still style tracked
      const look = lookForEntity(ent);
      const tr = tracked.find((t) => t.agent_id === ent.id);
      let fg = tr ? tr.color : materialColor(look.material, theme);
      let bg = selectedAgentIds.has(ent.id)
        ? theme.selection || "#003333"
        : theme.void;
      const glyph =
        ent.kind === "agent" && ent.name
          ? /[A-Za-z]/.test(ent.name[0])
            ? ent.name[0].toUpperCase()
            : look.glyph || "@"
          : look.glyph || "@";
      // Only skip agents that are not visible path... server already gates.
      // Keep previous rule: untracked agents may show if in snapshot.
      d.draw(vx, vy, glyph, fg, bg);
    } else {
      const look = lookForEntity(ent);
      d.draw(
        vx,
        vy,
        look.glyph || ent.glyph || "?",
        materialColor(look.material, theme),
        theme.void,
      );
    }
  }
  // … keep tracked follow highlight + map margin code …
}
```

**Important:** Re-read current `drawSnap` agent filter rules and preserve: tracked-only agents were relaxed earlier to show all visible agents. Do not re-introduce “tracked only” for non-dev.

- [ ] **Step 3: Refresh catalog when version changes**

In `applySnapshot`:

```javascript
if (snap.catalog_version != null) {
  ensureArtCatalog(snap.catalog_version).catch(() => {});
}
```

- [ ] **Step 4: Manual visual check**

Local serve or remote after deploy: map should show material-colored water/lava/walls; trees as `♣` if overlay applied.

- [ ] **Step 5: Commit**

```bash
git add crates/ask-kernel/static/app.js
git commit -m "feat(ui): render map from feat_ids + art catalog"
```

---

### Task 6: FS-HDG chrome (layout / CSS / font)

**Files:**
- Modify: `crates/ask-kernel/static/app.css`
- Modify: `crates/ask-kernel/static/index.html`

**Interfaces:**
- Produces: dense command-center chrome matching FS-HDG tokens

- [ ] **Step 1: CSS tokens and panel chrome**

Update `:root` and panels:

```css
:root {
  --r80-primary: #00ff66;
  --r80-secondary: #ffcc00;
  --r80-tertiary: #ff3333;
  --r80-neutral: #000000;
  --r80-neutral-variant: #1c1c1c;
  --r80-text: #ffffff;
  --r80-accent: #00e5ff;
  --font-mono: "Perfect DOS VGA 437", "Courier New", Courier, monospace;
  --fs-ui: 13px;
  --panel-border: #00e5ff;
}
/* double-line feel via outline + border */
#tracker, #selector-panel, #theme-bar, #log, #inspect-popup {
  border: 2px solid var(--r80-accent);
  box-shadow: inset 0 0 0 1px var(--r80-neutral-variant);
}
#map canvas {
  image-rendering: pixelated;
}
```

Add Google fonts optional — or skip external font and use Courier with `font-size` locked for square cells (ROT already forceSquareRatio).

- [ ] **Step 2: Index title / help**

```html
<title>ASK // FS-HDG</title>
```

Help bar mention material art is catalog-driven (short).

- [ ] **Step 3: Commit**

```bash
git add crates/ask-kernel/static/app.css crates/ask-kernel/static/index.html
git commit -m "feat(ui): FS-HDG chrome tokens and dense panel borders"
```

---

### Task 7: Guardrails + docs + skill

**Files:**
- Create: `crates/ask-kernel/tests/art_no_systems_import.rs` (optional compile-time check via grep in CI script)
- Modify: `.claude/skills/ask-sandbox/SKILL.md`
- Modify: `docs/superpowers/specs/2026-07-16-fs-hdg-art-decouple-design.md` if needed

- [ ] **Step 1: Add a simple repo check script or test comment**

In `art.rs` module docs already say systems must not import. Add test:

```rust
#[test]
fn systems_do_not_import_art() {
    // Lightweight guard: scan source text
    let systems = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/systems");
    for entry in std::fs::read_dir(systems).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            let s = std::fs::read_to_string(&p).unwrap();
            assert!(
                !s.contains("crate::art") && !s.contains("use crate::art"),
                "{} must not import art",
                p.display()
            );
        }
    }
}
```

- [ ] **Step 2: Skill API table**

Add:

```markdown
| `GET /api/art` | presentation catalog (materials, feat looks) |
```

Note: map presentation is catalog-driven; agents still use `/api/me` only.

- [ ] **Step 3: Full test suite**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo test -p ask-kernel --all-targets
```

- [ ] **Step 4: Commit**

```bash
git add crates/ask-kernel/src/art.rs .claude/skills/ask-sandbox/SKILL.md \
  crates/ask-kernel/tests docs/superpowers/specs/2026-07-16-fs-hdg-art-decouple-design.md
git commit -m "docs+test: art decoupling guardrails and skill API"
```

---

### Task 8: Build, smoke, deploy

**Files:** none new

- [ ] **Step 1: Release build**

```bash
PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
  cargo build --release -p ask-kernel
```

- [ ] **Step 2: Local smoke**

```bash
./target/release/ask-kernel --serve --port 8080 &
sleep 1
curl -s http://127.0.0.1:8080/api/art | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["ok"], d["catalog_version"], len(d["feats"]))'
curl -s 'http://127.0.0.1:8080/api/snapshot?token=ask1_dev_…' | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get("catalog_version"), d.get("feat_ids",{}).get("enc"))'
```

- [ ] **Step 3: Deploy remote**

```bash
tar czf /tmp/ask-kernel-code.tar.gz --exclude=target --exclude=.git --exclude=.claude .
sshpass -p 'Odyssey20031213' scp -o StrictHostKeyChecking=no /tmp/ask-kernel-code.tar.gz root@111.231.50.85:/tmp/
sshpass -p 'Odyssey20031213' ssh -o StrictHostKeyChecking=no root@111.231.50.85 \
  'cd /root/AgentGame && tar xzf /tmp/ask-kernel-code.tar.gz --overwrite && source $HOME/.cargo/env && cargo build --release -p ask-kernel && systemctl restart ask-kernel'
```

- [ ] **Step 4: Remote smoke**

```bash
curl -s http://111.231.50.85:8000/api/art | head -c 300
curl -s http://111.231.50.85:8000/api/status
```

- [ ] **Step 5: Commit skill tarball if packaged**

```bash
tar czf ask-sandbox-skill.tar.gz -C .claude/skills ask-sandbox
git add ask-sandbox-skill.tar.gz
git commit -m "chore: package skill after FS-HDG art decouple"
```

---

## Self-Review

**1. Spec coverage**
- Identity snapshot + catalog: Tasks 1–2  
- Backend clean systems: Task 1 rules + Task 7 guard  
- Frontend extensible renderer: Tasks 3–5  
- FS-HDG materials/chrome: Tasks 4, 6  
- Deploy: Task 8  
- Z-axis: explicitly non-goal this cycle (catalog `layer` reserved)

**2. Placeholder scan:** none intentional; r_info/k_info iteration must match real APIs when implementing Task 1.

**3. Type consistency**
- `FeatIdsPayload.enc = "u16le_b64"` everywhere  
- `catalog_version: u32`  
- Entity `race_id`/`kind_id` optional u16  

---

## Execution Handoff

Plan complete and saved to:

- Design: `docs/superpowers/specs/2026-07-16-fs-hdg-art-decouple-design.md`
- Plan: `docs/superpowers/plans/2026-07-16-fs-hdg-art-decouple.md`

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — this session with `executing-plans`, batch checkpoints  

Which approach?
