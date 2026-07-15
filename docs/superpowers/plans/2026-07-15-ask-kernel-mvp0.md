# ASK Kernel MVP-0 Implementation Plan

> **For agentic workers:** Implement task-by-task. Parallelize independent modules after workspace scaffold.

**Goal:** Runnable Rust Agent Simulation Kernel CLI with grid, actions, mock agent, ASCII, save/load.

**Architecture:** bevy_ecs World + phased tick; no renderer; serde snapshots.

**Tech:** Rust stable, bevy_ecs, serde, serde_json, clap or manual args, tracing optional.

## Tasks

### Task 1: Workspace + crate scaffold
- Root `Cargo.toml` workspace
- `crates/ask-kernel` lib+bin
- Dependencies: bevy_ecs, serde, serde_json, anyhow
- README for ASK
- Commit

### Task 2: Grid + components + spawn
- terrain, flags, generate map
- components, spawn agent/trees/iron
- Commit

### Task 3: Actions + systems + tick loop
- Action queue, movement/harvest/build systems
- Phased tick
- Unit tests
- Commit

### Task 4: Mock agent + ASCII view + CLI
- mock policy, view, main args
- Commit

### Task 5: Persist save/load + stubs + demo run
- snapshot serde, gateway/memory/skill/org stubs
- `cargo test` && `cargo run -- --steps 40`
- Commit

## Parallelism
Wave1: Task1  
Wave2: Task2+partial Task3 types  
Wave3: Task3 complete + Task4 + Task5  
