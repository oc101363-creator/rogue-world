# Agent Civilization OS (ACO)

Agent-native civilization simulation demo.

> World is authoritative. Agents connect over WebSocket, receive observations, submit actions. Frontend observes and selects agents.

## Spec & Plan

- Design: [`docs/superpowers/specs/2026-07-15-aco-v1-demo-design.md`](docs/superpowers/specs/2026-07-15-aco-v1-demo-design.md)
- Plan: [`docs/superpowers/plans/2026-07-15-aco-v1-demo.md`](docs/superpowers/plans/2026-07-15-aco-v1-demo.md)

## Quick start

```bash
pnpm install
pnpm --filter @aco/protocol build

# terminal 1
pnpm dev:world

# terminal 2
cd apps/agent-runtime && python -m aco_runtime.main --mode mock

# terminal 3
pnpm dev:web
```

Open the Vite URL, watch the agent move/harvest, click the agent to select.
