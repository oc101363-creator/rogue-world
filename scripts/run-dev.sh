#!/bin/zsh
# Dev server with crash forensics:
#   - pinned ASK_SECRET → dev token survives restarts
#   - world autosave (--save) + restore on restart (--load)
#   - watchdog loop: auto-restarts and LOGS THE EXIT CODE so a silent
#     death leaves evidence (137=SIGKILL/OOM, 134=SIGABRT, 139=SIGSEGV)
cd "$(dirname "$0")/.."
mkdir -p saves
export ASK_SECRET="${ASK_SECRET:-agentgame-dev-pinned}"
export RUST_BACKTRACE=1
PORT="${PORT:-8080}"
TICK_MS="${TICK_MS:-500}"
SAVE=saves/ask-dev.json

while true; do
  LOAD_ARG=()
  [[ -f "$SAVE" ]] && LOAD_ARG=(--load "$SAVE")
  ./target/debug/ask-kernel --serve --port "$PORT" --tick-ms "$TICK_MS" --save "$SAVE" "${LOAD_ARG[@]}" >> /tmp/ask8080.log 2>&1
  code=$?
  echo "[watchdog] exited code=$code at $(date '+%F %T') (137=SIGKILL/OOM 134=SIGABRT 139=SIGSEGV) — restarting in 2s" >> /tmp/ask8080.log
  sleep 2
done
