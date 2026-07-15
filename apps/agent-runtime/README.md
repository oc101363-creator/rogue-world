# ACO Agent Runtime

Python Agent Runtime for Agent Civilization OS V1. Connects to the World Server over WebSocket, receives observations, and returns action batches using a mock rule policy or an optional LLM policy.

## Requirements

- Python 3.11+
- World Server reachable at the configured WebSocket URL (default `ws://127.0.0.1:8080/ws/agent?agentId=agent-1`)

## Install

```bash
cd apps/agent-runtime
python -m pip install -r requirements.txt
```

## Run

Mock policy (always available, no API key):

```bash
python -m aco_runtime --url ws://127.0.0.1:8080/ws/agent?agentId=agent-1 --mode mock
```

LLM policy (requires API key):

```bash
export ACO_LLM_API_KEY=sk-...
export ACO_LLM_BASE_URL=https://api.openai.com/v1   # optional
export ACO_LLM_MODEL=gpt-4o-mini                    # optional
python -m aco_runtime --mode llm
```

CLI flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--url` | `ws://127.0.0.1:8080/ws/agent?agentId=agent-1` | World agent WebSocket URL |
| `--mode` | `mock` (or `ACO_RUNTIME_MODE`) | `mock` or `llm` |

Environment:

| Variable | Purpose |
|----------|---------|
| `ACO_RUNTIME_MODE` | Default mode when `--mode` omitted |
| `ACO_LLM_API_KEY` | Required for `llm` mode |
| `ACO_LLM_BASE_URL` | Chat completions base URL (default `https://api.openai.com/v1`) |
| `ACO_LLM_MODEL` | Model name (default `gpt-4o-mini`) |

## Protocol

1. Connect WebSocket
2. On `hello` → reply `hello_ack` with `protocolVersion: "1.0"` and `runtime: "mock"|"llm"`
3. On `observation` → decide actions → send `action_batch` for the same `agentId` and `tick`
4. Handle `ping`/`pong` and `error`

## Mock policy

- Standing on a resource with `ore > 0` → `harvest`
- Else step one 4-way move toward nearest visible resource with ore (Manhattan; prefer larger axis delta, tie-break prefer `dx`)
- Else `idle`

## Tests

```bash
cd apps/agent-runtime
python -m unittest discover -s tests -v
# or, if pytest is installed:
python -m pytest tests/ -q
```
