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
#
# A screenshot is evidence, so every way of producing an irrelevant one is a
# failure here rather than a PNG: the listener on the port has to identify
# itself as the visual host, the page has to report the fixture that was asked
# for (`--dump-dom` comes back from the same run as the shot), the PNG has to be
# a PNG the size of the viewport preset, Edge's exit status counts, and the
# browser runs under a wall clock.

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

# The visual host says who it is in its own `<head>`; anything else on the port
# is somebody else's server and must not be photographed.
is_visual_host() {
  curl -fsS --max-time 5 "http://localhost:${PORT}/" 2>/dev/null \
    | grep -q 'name="taurhaus-visual-host"'
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

# The host falls back to the scenario's own theme for a theme it does not know,
# so `theme=drak` would render light, file itself under `drak`, and succeed.
case "$THEME" in
  light|dark) ;;
  *) echo "Unknown theme '$THEME' (light|dark)." >&2; exit 2 ;;
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

if ! is_visual_host; then
  echo "The server on port $PORT is not the visual host." >&2
  echo "Stop it, or point VISUAL_SHOT_PORT at a free port." >&2
  exit 6
fi

NAME="${OUT:-${COMPONENT}-${SCENARIO}-${VIEWPORT}-${THEME}}"
NAME="${NAME%.png}"
WIN_PATH="${WIN_SHOT_DIR}\\${NAME}.png"
WSL_PATH="${WSL_SHOT_DIR}/${NAME}.png"
URL="http://localhost:${PORT}/?component=${COMPONENT}&scenario=${SCENARIO}&viewport=${VIEWPORT}&theme=${THEME}&chrome=0"

rm -f "$WSL_PATH"
DOM_FILE="$(mktemp)"
trap 'rm -f "$DOM_FILE"' EXIT

# `--virtual-time-budget` is what makes the shot wait for the app: the host is
# a Svelte SPA behind a dev server, and load fires long before it paints.
# `--dump-dom` returns the page this same run photographed, and the wall clock
# bounds the process itself — the budget only bounds the page's own time.
status=0
TIMEOUT_S="${VISUAL_SHOT_TIMEOUT_S:-90}"
# TERM asks; a hung renderer does not have to answer. `--kill-after` is what
# turns the wall clock into one: KILL follows, and the lane returns either way.
timeout --kill-after="${VISUAL_SHOT_KILL_AFTER_S:-5}s" "$TIMEOUT_S" "$EDGE" \
  --headless=new \
  --disable-gpu \
  --hide-scrollbars \
  --no-first-run \
  --virtual-time-budget="${VISUAL_SHOT_BUDGET_MS:-6000}" \
  --window-size="${WIDTH},${HEIGHT}" \
  --force-device-scale-factor=1 \
  --dump-dom \
  --screenshot="$WIN_PATH" \
  "$URL" >"$DOM_FILE" 2>/dev/null || status=$?

# 124: `timeout` gave up on it. 137: it had to be killed after ignoring TERM.
if [[ "$status" -eq 124 || "$status" -eq 137 ]]; then
  echo "Edge timed out after ${TIMEOUT_S}s on $URL" >&2
  exit 9
fi
if [[ "$status" -ne 0 ]]; then
  echo "Edge failed (exit $status) on $URL" >&2
  exit 8
fi

# The page names the state it rendered, all four parts of it: a shot is
# evidence about one component, in one scenario, at one size, in one theme.
# Anything else means the host fell back and the PNG shows something else.
#
# `-F` because a component or scenario is a caller's string: read as a regular
# expression, a name carrying `.` or `*` matches the fixture the host fell back
# to, and the fallback gets filed under the requested name.
WANTED="${COMPONENT}/${SCENARIO}/${VIEWPORT}/${THEME}"
if ! grep -Fq -- "data-visual-host-fixture=\"${WANTED}\"" "$DOM_FILE"; then
  RENDERED="$(grep -o 'data-visual-host-fixture="[^"]*"' "$DOM_FILE" | head -1 | cut -d'"' -f2)"
  echo "Asked for '${WANTED}' but the page rendered '${RENDERED:-nothing}'." >&2
  echo "Check the component id and scenario name in the visual registry." >&2
  exit 7
fi

if [[ ! -s "$WSL_PATH" ]]; then
  echo "Edge produced no screenshot at $WSL_PATH" >&2
  exit 5
fi

# A PNG says its own size in the IHDR chunk, right after the signature: bytes
# 0-7 are the signature, 12-15 spell IHDR, and 16-23 are width and height,
# big-endian.
png_size() {
  local bytes
  bytes="$(od -An -v -tx1 -N24 "$1" | tr -d ' \n')"
  [[ "${bytes:0:16}" == "89504e470d0a1a0a" && "${bytes:24:8}" == "49484452" ]] || return 1
  echo "$((16#${bytes:32:8}))x$((16#${bytes:40:8}))"
}

# The window size was asked for; the pixels are what the shot is. A browser
# rendering at another device scale, or ignoring the window size, produces a
# file that says a viewport it never showed.
if ! SHOT_SIZE="$(png_size "$WSL_PATH")"; then
  echo "The file at $WSL_PATH is not a PNG." >&2
  exit 10
fi
if [[ "$SHOT_SIZE" != "${WIDTH}x${HEIGHT}" ]]; then
  echo "Asked for a ${WIDTH}x${HEIGHT} shot but Edge wrote ${SHOT_SIZE} to $WSL_PATH." >&2
  echo "Check the window size and the device scale factor." >&2
  exit 10
fi

echo "$WSL_PATH"
