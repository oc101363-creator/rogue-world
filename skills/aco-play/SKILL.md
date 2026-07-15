---
name: aco-play
description: >
  Play Agent Civilization OS via the aco CLI. Use when controlling an ACO world
  agent, observing the grid, moving, harvesting, focusing an agent, or joining
  a running World Server with shell tools.
---

# Play ACO (observe → act loop)

You control a world agent by shelling out to **`aco`**. The World Server is authoritative; you never invent state.

**Leading moves:** `observe`, `act`, `harvest`, `focus`, `tick`.

## Prerequisites

- World running (default `http://127.0.0.1:8080`)
- CLI available as `aco` on PATH, or:
  `pnpm --filter @aco/cli aco -- <args>`
  from the AgentGame repo root
- Optional env: `ACO_WORLD_URL`, `ACO_AGENT_ID` (default `agent-1`)

## Loop (every decision)

### 1. Status check

```bash
aco status
```

**Done when:** JSON has `"ok": true` (or top-level tick fields). If connection fails, stop and tell the user to start World (`pnpm dev:world`).

### 2. Observe

```bash
aco observe --agent agent-1
```

Read from the JSON:

| Field | Use |
|-------|-----|
| `tick` | Decision tick — pass to act if you set `--tick` |
| `self.x` `self.y` | Your cell |
| `self.inventory.ore` | Score / progress |
| `visible.tiles` | Terrain rows (`#` wall, `.` floor) |
| `visible.entities` | Agents + resources (`M` / type `resource`, `ore`) |
| `allowed_actions` | Only these verbs |
| `focused` | Whether you are the focused agent |
| `events` | What just happened |

**Done when:** You can state your position and the nearest resource (or that none remain).

### 3. Decide (rules of thumb)

1. If you stand on a resource with `ore > 0` → **harvest**
2. Else if any resource with `ore > 0` exists → **move** one step 4-way toward nearest (Manhattan; no diagonals)
3. Else → **idle** (or `say` a short status)

Constraints:

- At most **one** mutating action per tick: `move` | `harvest` | `idle`
- `say` may accompany only via separate act calls on later ticks in V1 CLI (one act per command)
- `move`: `|dx|+|dy|==1`, each in `-1|0|1`
- Never walk into `#`

### 4. Act

```bash
aco act move --dx 1 --dy 0 --agent agent-1
aco act harvest --agent agent-1
aco act idle --agent agent-1
aco act say --text "mining" --agent agent-1
```

**Done when:** Response has `"ok": true` and `"queued": true`.  
If `409` / `stale_or_future_tick`: re-`observe`, use the new `tick`, retry once.

### 5. Confirm (optional but preferred)

```bash
aco map
aco events --last 5
```

**Done when:** Map glyph `A`/`@` moved or inventory/events show `HARVESTED` / `MOVED`.

### 6. Repeat

Continue observe → decide → act until the user stops you or a stated goal is met (e.g. inventory ore ≥ N).

## Focus (player / commander)

```bash
aco focus agent-1
```

Marks the agent as selected for the web UI and sets `focused: true` on its next observation.

## Map legend

| Glyph | Meaning |
|-------|---------|
| `#` | Wall (impassable) |
| `.` | Floor |
| `M` | Resource (mine) |
| `A` | Agent |
| `@` | Focused agent |

## Failure handling

| Symptom | Action |
|---------|--------|
| `connection_failed` | World down — do not invent a map |
| `unknown agent` | `aco status` for valid ids |
| act `409` stale tick | Fresh `observe`, retry with new tick |
| `MOVE_FAILED` in events | Pick another direction; avoid walls |

## Reference

Full HTTP surface and examples: [references/api.md](references/api.md)
