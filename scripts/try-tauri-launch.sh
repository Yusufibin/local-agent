#!/usr/bin/env bash
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRATCH="${1:-/tmp/grok-goal-26b12f88ca7c/implementer}"
mkdir -p "$SCRATCH"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$ROOT"

if ! command -v xvfb-run >/dev/null 2>&1; then
  echo "xvfb-run missing" | tee "$SCRATCH/tauri-launch-unavailable.log"
  exit 0
fi
if ! pkg-config --exists webkit2gtk-4.1; then
  echo "webkit2gtk-4.1 missing" | tee "$SCRATCH/tauri-launch-unavailable.log"
  exit 0
fi

python3 - <<'PY' | tee -a "$SCRATCH/tauri-launch-1.log"
import json
p=json.load(open("src-tauri/tauri.conf.json"))
w=p["app"]["windows"][0]
assert p["identifier"]=="local.agent.desktop"
assert w["width"]==1100 and w["height"]==720
assert w["minWidth"]==800 and w["minHeight"]==560
print("window config ok", w["width"], w["height"], "min", w["minWidth"], w["minHeight"])
print("identifier", p["identifier"])
print("csp", p["app"]["security"]["csp"])
PY

launch_once() {
  local log="$1"
  echo "=== launch $(date -Iseconds) ===" | tee "$log"
  timeout 75 xvfb-run -a -s "-screen 0 1280x800x24" pnpm tauri dev >"$log.raw" 2>&1 &
  local pid=$!
  local ok=0
  for i in $(seq 1 50); do
    sleep 1
    if grep -E "Local Agent|identifier|Finished|Running|error while running|Failed to create|webkit|WebKit" "$log.raw" >/dev/null 2>&1; then
      ok=1
      break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      break
    fi
  done
  sleep 3
  if command -v xwininfo >/dev/null 2>&1; then
    DISPLAY="${DISPLAY:-:99}" xwininfo -root -tree 2>/dev/null | tee -a "$log" || true
  fi
  cat "$log.raw" >> "$log"
  kill "$pid" 2>/dev/null || true
  pkill -P "$pid" 2>/dev/null || true
  sleep 2
  kill -9 "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

launch_once "$SCRATCH/tauri-launch-1.log"
sleep 2
pgrep -a pi > "$SCRATCH/pgrep-after-exit.txt" || echo "(no pi processes)" > "$SCRATCH/pgrep-after-exit.txt"
launch_once "$SCRATCH/tauri-launch-2.log"
sleep 2
pgrep -a pi >> "$SCRATCH/pgrep-after-exit.txt" || true

# If tauri never got past compile/display, copy evidence
if grep -qiE "cannot open display|WebKitGTK|Failed to create webview|error while running tauri" "$SCRATCH/tauri-launch-1.log"; then
  grep -iE "cannot open display|WebKitGTK|Failed to create webview|error while running tauri|missing" "$SCRATCH/tauri-launch-1.log" \
    | head -40 | tee "$SCRATCH/tauri-launch-unavailable.log" || true
fi

echo "launch logs written"