# FS-HDG Art Decouple Design

**Date:** 2026-07-16  
**Status:** approved for planning (elegant / decoupled)

## Problem

ASK currently bakes presentation into the simulation→client pipe:

- `f_info` carries frog glyph + 16-color letter
- `viewer` projects those into `tiles` / `tile_colors` strings every tick
- frontend themes only remap color letters to RGB

That makes “art reform” look like a skin swap. Real FS-HDG needs **material-driven reading**, denser chrome, and room to grow (Z-layers, new glyphs) without bloating `systems/*`.

## Goals

1. **Backend stays clean:** simulation code never imports art; only `FeatId` / flags / Matter.
2. **Frontend is extensible:** renderer consumes identity + a catalog; new art standards do not require kernel logic changes.
3. **Ship FS-HDG aesthetics** (material palette, square DOS grid, dense nested panels) without a rewrite of dig/move/craft.

## Non-Goals (this cycle)

- True multi-Z world simulation
- Tile atlases / sprite sheets
- Rewriting frog `f_info.txt` gameplay flags
- Breaking agent action API (`/api/me` action loop)

## Architecture

```
┌─────────────────────────────────────────────┐
│ Simulation (clean)                          │
│  Grid.cells: FeatId                         │
│  Entity: StableId, kind components, templates│
│  systems/* use flags only                   │
└───────────────────┬─────────────────────────┘
                    │ identity only
┌───────────────────▼─────────────────────────┐
│ Viewer contract                             │
│  feat_ids (compact) + vision                │
│  entities: id, kind, x, y, race_id?, kind_id?│
│  legacy tiles/tile_colors optional          │
└───────────────────┬─────────────────────────┘
                    │
┌───────────────────▼─────────────────────────┐
│ Art Catalog (data, not logic)               │
│  feat_id → glyph, material, layer, …        │
│  race_id / kind_id → entity look            │
│  baseline from f_info/r_info/k_info         │
│  overlay: art/fs_hdg_overlay.toml           │
└───────────────────┬─────────────────────────┘
                    │
┌───────────────────▼─────────────────────────┐
│ Frontend FS-HDG renderer + chrome           │
│  catalog + theme materials → pixels         │
│  nested sidebars, DOS font, square cells    │
└─────────────────────────────────────────────┘
```

## Hard rules

1. `systems/*` must not reference glyphs, materials, colors, or art modules.
2. Adding a new visual for existing terrain = catalog/overlay edit only.
3. Adding a new terrain with physics = `f_info` flags + systems (as today); art is a separate row.
4. Catalog is versioned (`catalog_version`) so clients can cache via `/api/art`.

## Data contracts

### Art catalog (`GET /api/art`)

```json
{
  "ok": true,
  "catalog_version": 1,
  "materials": {
    "basalt": "#555555",
    "granite": "#AAAAAA",
    "gold": "#FFD700",
    "aquifer": "#0055FF",
    "magma": "#FF4500",
    "organic": "#8B5A2B",
    "void": "#000000",
    "ui_primary": "#00FF66",
    "ui_warning": "#FFCC00",
    "ui_danger": "#FF3333",
    "ui_info": "#00E5FF"
  },
  "feats": {
    "1": { "glyph": ".", "material": "basalt", "layer": 0, "name": "FLOOR" },
    "56": { "glyph": "#", "material": "granite", "layer": 0, "name": "GRANITE" }
  },
  "entities": {
    "agent": { "glyph": "@", "material": "ui_warning" },
    "tree": { "glyph": "♣", "material": "organic" },
    "monster": { "by_race": true },
    "item": { "by_kind": true }
  },
  "races": { "12": { "glyph": "o", "material": "ui_danger", "name": "..." } },
  "objects": { "3": { "glyph": "!", "material": "ui_info", "name": "..." } }
}
```

### Snapshot (identity-first)

Keep existing fields for one release. Add:

- `catalog_version: u32`
- `feat_ids`: compact row-major encoding
  - `{ "enc": "u16le_b64", "w": W, "h": H, "data": "<base64>" }`
  - unseen cells still carry true feat id (server already knows them); client must paint void when `vision==' '`
- `entities[]` extra optional: `race_id`, `kind_id`, `template`

Legacy `tiles` / `tile_colors` remain filled for terminal/`view::render` and old clients until frontend migrates.

## Catalog generation

1. **Baseline:** for each `FeatInfo`, map frog color letter + flags → material heuristic  
   - water → aquifer, lava → magma, tree → organic, wall/granite → granite, dirt/grass → organic, default floor → basalt  
   - glyph from frog `G:` line (overrideable)
2. **Overlay:** `crates/ask-kernel/data/art/fs_hdg_overlay.toml` can replace glyph/material/layer per id
3. **Races/objects:** baseline from `r_info` / `k_info` glyph+color → material

No simulation system loads overlay.

## Frontend

1. Fetch `/api/art` once (or when `catalog_version` changes).
2. Map renderer: for each visible/memory cell, look up `feat_ids[y*w+x]` in catalog; apply material palette; dim memory.
3. Theme system becomes **material remaps + chrome**, not letter remaps.
4. Chrome: FS-HDG nested panels (tracker / selector / inspect / log) with double-line borders, DOS-like monospace, square cells.

## Migration

1. Ship catalog + `/api/art` + snapshot `feat_ids` without removing tiles.
2. Switch web client to feat_ids path.
3. Optional later: stop sending `tiles`/`tile_colors` to web (keep for ASCII terminal).

## Success criteria

- New FS-HDG look is material-readable on large maps.
- `rg "material|glyph|art::" crates/ask-kernel/src/systems` is empty.
- Adding overlay entries changes look without recompiling systems.
- Existing agent `/api/action` + `/api/me` loop unchanged.
