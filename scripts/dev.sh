#!/usr/bin/env bash
# Start World + mock Agent Runtime for local demo.
# Frontend: run `pnpm dev:web` in another terminal (or set START_WEB=1).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

mkdir -p "$ROOT/data" "$ROOT/apps/world/data" /tmp

echo "[dev] building protocol…"
pnpm --filter @aco/protocol build

if ! python3 -c "import websockets" 2>/dev/null; then
  echo "[dev] installing agent-runtime deps…"
  python3 -m pip install -r apps/agent-runtime/requirements.txt -q
fi

WORLD_LOG="${ACO_WORLD_LOG:-/tmp/aco-world.log}"
AGENT_LOG="${ACO_AGENT_LOG:-/tmp/aco-agent.log}"

cleanup() {
  echo "[dev] shutting down…"
  [[ -n "${WORLD_PID:-}" ]] && kill "$WORLD_PID" 2>/dev/null || true
  [[ -n "${AGENT_PID:-}" ]] && kill "$AGENT_PID" 2>/dev/null || true
  [[ -n "${WEB_PID:-}" ]] && kill "$WEB_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "[dev] starting World Server…"
pnpm --filter @aco/world dev >"$WORLD_LOG" 2>&1 &
WORLD_PID=$!

# Wait for port 8080
for i in $(seq 1 40); do
  if curl -sf -o /dev/null --http0.9 "http://127.0.0.1:8080/" 2>/dev/null \
    || nc -z 127.0.0.1 8080 2>/dev/null; then
    break
  fi
  sleep 0.25
done

echo "[dev] starting Agent Runtime (mock)…"
(
  cd "$ROOT/apps/agent-runtime"
  python3 -m aco_runtime --mode mock --url 'ws://127.0.0.1:8080/ws/agent?agentId=agent-1'
) >"$AGENT_LOG" 2>&1 &
AGENT_PID=$!

if [[ "${START_WEB:-0}" == "1" ]]; then
  echo "[dev] starting Web…"
  pnpm --filter @aco/web dev >"${ACO_WEB_LOG:-/tmp/aco-web.log}" 2>&1 &
  WEB_PID=$!
fi

echo ""
echo "  World   ws://127.0.0.1:8080/ws/frontend"
echo "  Agent   mock → agent-1  (log: $AGENT_LOG)"
echo "  Web     pnpm dev:web  → http://localhost:5173"
echo "  World log: $WORLD_LOG"
echo ""
echo "[dev] running (Ctrl+C to stop)…"

wait
