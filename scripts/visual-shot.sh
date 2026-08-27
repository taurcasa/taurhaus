#!/usr/bin/env bash
#
# Shoot one visual-host fixture at a real window size with Windows Edge headless.
#
# The browser-mode vitest lane renders a component into a 960x640 test page; a
# popup that positions itself against the viewport cannot be judged there. This
# starts the visual host, points Edge at `?component=&scenario=&viewport=&
# theme=&chrome=0`, and writes a PNG the size of the viewport preset.
#
# Edge runs on the Windows side, so the screenshot path must be a Windows path
# and the URL must be one Windows can reach — localhost forwarding gives us
# that for a server bound inside WSL.
#
# The server is only ever started when nothing is already listening on the port,
# and `--stop` kills only a pid this script wrote down and verified. Somebody
# else's `bun run dev:visual` is left alone.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT="${VISUAL_SHOT_PORT:-5211}"
RUN_DIR=".visual-shot"
PID_FILE="$RUN_DIR/server.pid"
LOG_FILE="$RUN_DIR/server.log"
EDGE="${VISUAL_SHOT_EDGE:-/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe}"
WIN_SHOT_DIR="${VISUAL_SHOT_WINDOWS_DIR:-C:\\taurhaus_build\\shots}"
WSL_SHOT_DIR="${VISUAL_SHOT_WSL_DIR:-/mnt/c/taurhaus_build/shots}"

port_busy() {
  # `ss` is the only listener probe present on every dev host here.
  ss -ltn 2>/dev/null | grep -q ":${PORT} "
}

stop_server() {
  if [[ ! -f "$PID_FILE" ]]; then
    echo "No visual-shot server recorded; nothing to stop."
    return 0
  fi
  local pid
  pid="$(cat "$PID_FILE")"
  rm -f "$PID_FILE"
  if [[ -z "$pid" ]] || ! kill -0 "$pid" 2>/dev/null; then
    echo "Recorded visual-shot server ($pid) is already gone."
    return 0
  fi
  # Never kill a process this script did not start: the recorded pid has to
  # still be the vite server we launched.
  local cmd
  cmd="$(ps -o cmd= -p "$pid" 2>/dev/null || true)"
  if [[ "$cmd" != *"vite.visual.config.js"* ]]; then
    echo "Refusing to kill pid $pid — it is no longer the visual host ($cmd)." >&2
    return 1
  fi
  kill "$pid"
  echo "Stopped the visual-shot server (pid $pid)."
}

if [[ "${1:-}" == "--stop" ]]; then
  stop_server
  exit 0
fi

COMPONENT="${1:?component id is required (e.g. shell-popups)}"
SCENARIO="${2:?scenario name is required}"
VIEWPORT="${3:-laptop}"
THEME="${4:-light}"
OUT="${5:-}"

case "$VIEWPORT" in
  desktop) WIDTH=1920; HEIGHT=1080 ;;
  laptop)  WIDTH=1366; HEIGHT=768 ;;
  narrow)  WIDTH=1024; HEIGHT=768 ;;
  *) echo "Unknown viewport '$VIEWPORT' (desktop|laptop|narrow)." >&2; exit 2 ;;
esac

if [[ ! -x "$EDGE" ]]; then
  echo "Edge not found at: $EDGE" >&2
  echo "Set VISUAL_SHOT_EDGE to the msedge.exe path." >&2
  exit 3
fi

mkdir -p "$RUN_DIR" "$WSL_SHOT_DIR"

if port_busy; then
  echo "Reusing the server already listening on port $PORT."
else
  echo "Starting the visual host on port $PORT..."
  # `--strictPort` so a busy port fails loudly instead of silently moving.
  nohup bunx vite --config vite.visual.config.js \
    --port "$PORT" --strictPort --host 127.0.0.1 >"$LOG_FILE" 2>&1 &
  echo $! >"$PID_FILE"
  for _ in $(seq 1 60); do
    port_busy && break
    sleep 0.5
  done
  if ! port_busy; then
    echo "The visual host did not come up on port $PORT; see $LOG_FILE" >&2
    exit 4
  fi
fi

NAME="${OUT:-${COMPONENT}-${SCENARIO}-${VIEWPORT}-${THEME}}"
NAME="${NAME%.png}"
WIN_PATH="${WIN_SHOT_DIR}\\${NAME}.png"
WSL_PATH="${WSL_SHOT_DIR}/${NAME}.png"
URL="http://localhost:${PORT}/?component=${COMPONENT}&scenario=${SCENARIO}&viewport=${VIEWPORT}&theme=${THEME}&chrome=0"

rm -f "$WSL_PATH"
# `--virtual-time-budget` is what makes the shot wait for the app: the host is
# a Svelte SPA behind a dev server, and load fires long before it paints.
"$EDGE" \
  --headless=new \
  --disable-gpu \
  --hide-scrollbars \
  --no-first-run \
  --virtual-time-budget="${VISUAL_SHOT_BUDGET_MS:-6000}" \
  --window-size="${WIDTH},${HEIGHT}" \
  --screenshot="$WIN_PATH" \
  "$URL" >/dev/null 2>&1 || true

if [[ ! -s "$WSL_PATH" ]]; then
  echo "Edge produced no screenshot at $WSL_PATH" >&2
  exit 5
fi

echo "$WSL_PATH"
