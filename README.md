# Agent Simulation Kernel (ASK)

A **digital world kernel** for Agents — not an RTS, not a content Roguelike.

> Organization → Agent → Action → World

Inspired by FrogComposband/Angband **simulation structure** (tick, grid, action→update, saveable world). Code is original.

## MVP-0

```bash
# needs recent Rust stable (1.78+)
cargo test -p ask-kernel
cargo run -p ask-kernel -- --steps 40
cargo run -p ask-kernel -- --watch --tick-ms 200
cargo run -p ask-kernel -- --steps 20 --save data/world.json
cargo run -p ask-kernel -- --load data/world.json --steps 10
```

## Docs

- Design: `docs/superpowers/specs/2026-07-15-ask-kernel-mvp0-design.md`
- Plan: `docs/superpowers/plans/2026-07-15-ask-kernel-mvp0.md`

## Layout

```
crates/ask-kernel/     Rust world kernel
frogcomposband-master/ Reference only (not linked)
docs/                  Specs and plans
```
