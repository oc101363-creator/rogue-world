# ASK Sandbox — Matter Pack + World Transformation

Date: 2026-07-16

## Goal

Agent can **substantially reshape the dungeon** with few action primitives and many data-driven combinations.

Not a combat game. Sandbox = dig / scoop / place / craft / plant / build / deconstruct.

## Primitives (stable)

```
move | interact{dx,dy,verb?,slot?,recipe?} | drop{index} | rest | idle
```

## Matter

```
Matter = Terrain{feat} | Resource{wood|iron} | Object{kind_id,name}
Inventory = Vec<Stack{matter, qty}>
```

## Verbs (discovered, not hard-coded into Action)

| verb | transforms |
|------|------------|
| dig | hard rock → rubble/floor; feat → pack (+iron from treasure veins) |
| scoop | soft surface (floor/dirt/grass/tree/water/lava/door…) → pack; leave successor |
| place | pack Terrain → grid; may return displaced surface to pack |
| harvest | Resource entity → Resource matter |
| plant | wood or TREE block → TREE feat + harvestable tree entity |
| build | wood → hut entity |
| deconstruct | hut → wood refund |
| craft | pack recipe → new matter (doors, veins, mountain, water, wood, iron…) |
| open/close/stairs/pickup | doors, depth, ground items |

## Recipes (table in `sandbox.rs`)

Examples: plank_door, sapling, compact_rock, crush_rock, ore_vein, mountain, smelt_iron, chop_wood, deep_pool, fill_floor, dirt_from_rubble, …

Adding a recipe = one table row. No new Action variant.

## Extract rules (table)

- dig_rule / scoop_rule map feat → leave + bonus
- can_place_on validates targets
- permanent walls never touchable

## API

- `GET /api/me` → pack (structured) + interactions + events
- `GET /api/actions` → primitives + recipe list
- `POST /api/action` with interact verb/recipe/slot
- Snapshot includes FOV + interactions
- Persist saves full pack slots

## Combinatorial power

Same 5 actions × many feats × recipes ⇒ tunnels, lakes, forests, walls, doors, mountains, ore plants, rebuilds — without growing the Action enum.
