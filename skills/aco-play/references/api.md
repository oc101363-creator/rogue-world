# ACO World HTTP API + CLI

Base URL: `ACO_WORLD_URL` or `http://127.0.0.1:8080`

## CLI → HTTP

| CLI | HTTP |
|-----|------|
| `aco status` | `GET /api/status` |
| `aco observe [--agent id]` | `GET /api/observe?agentId=` |
| `aco map` | `GET /api/map` |
| `aco events [--last N]` | `GET /api/events?last=N` |
| `aco focus <id>` | `POST /api/focus` `{"agentId"}` |
| `aco act move --dx --dy` | `POST /api/act` `{"type":"move","dx","dy"}` |
| `aco act harvest` | `POST /api/act` `{"type":"harvest"}` |
| `aco act idle` | `POST /api/act` `{"type":"idle"}` |
| `aco act say --text` | `POST /api/act` `{"type":"say","text"}` |

## Actions

| type | fields | notes |
|------|--------|-------|
| `move` | `dx`, `dy` ∈ {-1,0,1}, \|dx\|+\|dy\|=1 | 4-way only |
| `harvest` | — | same cell as resource, ore>0 |
| `idle` | — | no-op |
| `say` | `text` ≤ 200 | event only |

One mutating action per agent per tick. Actions are **queued** for `decisionTick` and apply on the next world tick (~500ms).

## Useful responses

**observe** includes `tick` (decision tick to echo), `self`, `visible`, `allowed_actions`, `focused`, `events`.

**act** success:

```json
{ "ok": true, "queued": true, "agentId": "agent-1", "tick": 12, "action": { "type": "move", "payload": { "dx": 1, "dy": 0 } } }
```

**act** stale tick:

```json
{ "ok": false, "error": "stale_or_future_tick: expected 13, got 12", "decisionTick": 13 }
```

## WebSocket (optional)

Still available for live UIs / push:

- `ws://host:8080/ws/frontend` — snapshots
- `ws://host:8080/ws/agent?agentId=agent-1` — push observations

CLI path does not require WebSocket.
