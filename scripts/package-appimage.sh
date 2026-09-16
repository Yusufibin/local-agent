#!/usr/bin/env bash
# Phase 8: Linux AppImage with vendored Node 22 + Pi 0.85.1.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:${PATH}"

if ! command -v pnpm >/dev/null; then
  echo "pnpm is required" >&2
  exit 1
fi

echo "vendoring Node 22 + Pi ${PI_VERSION:-0.85.1}"
bash "${ROOT}/scripts/vendor-runtime.sh"

if [[ ! -x "${ROOT}/agent-runtime/vendor/node/bin/node" ]]; then
  echo "vendor node missing after vendor-runtime.sh" >&2
  exit 1
fi
if [[ ! -e "${ROOT}/agent-runtime/vendor/pi/bin/pi" ]]; then
  echo "vendor pi missing after vendor-runtime.sh" >&2
  exit 1
fi

echo "building AppImage (Tauri 2, identifier local.agent.desktop, vendor in agent-runtime)"
pnpm tauri build --bundles appimage

bundle="${ROOT}/src-tauri/target/release/bundle/appimage"
if ! ls "${bundle}"/*.AppImage >/dev/null 2>&1; then
  echo "AppImage not produced under ${bundle}" >&2
  echo "Need linuxdeploy + webkit2gtk-4.1." >&2
  exit 1
fi
ls -lh "${bundle}"/*.AppImage

appdir="${bundle}/Local Agent.AppDir"
vendor_lib="${appdir}/usr/lib/Local Agent/agent-runtime/vendor"
node_bin="${vendor_lib}/node/bin/node"
pi_bin="${vendor_lib}/pi/bin/pi"
pi_cli="${vendor_lib}/pi/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
if [[ ! -f "$pi_cli" ]]; then
  pi_cli="${vendor_lib}/pi/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
fi
if [[ ! -d "$appdir" ]]; then
  echo "AppDir not found at ${appdir}" >&2
  exit 1
fi
if [[ ! -e "$node_bin" || ! -e "$pi_cli" ]]; then
  echo "vendored Node/Pi missing from AppDir (${vendor_lib})" >&2
  find "$appdir" \( -name node -o -name cli.js -o -name pi \) 2>/dev/null | head
  exit 1
fi
chmod 755 "$node_bin" "$pi_bin" 2>/dev/null || true

echo "smoke: vendored pi --version with PATH=/usr/bin:/bin (no global pi)"
smoke_home="$(mktemp -d /tmp/local-agent-pi-smoke.XXXXXX)"
# Strip /usr/local and the machine pi-node prefix.
if ! env -i \
  PATH="/usr/bin:/bin" \
  HOME="$smoke_home" \
  USER="${USER:-root}" \
  LANG=C \
  PI_CODING_AGENT_DIR="${smoke_home}/.pi/agent" \
  "$node_bin" "$pi_cli" --version; then
  echo "smoke failed: vendored pi --version" >&2
  exit 1
fi
if command -v timeout >/dev/null; then
  # JSONL get_state — no live LLM.
  if ! printf '%s\n' '{"id":"smoke","type":"get_state"}' \
    | env -i \
      PATH="/usr/bin:/bin" \
      HOME="$smoke_home" \
      USER="${USER:-root}" \
      LANG=C \
      TERM=dumb \
      NO_COLOR=1 \
      PI_CODING_AGENT_DIR="${smoke_home}/.pi/agent" \
      timeout 12 "$node_bin" "$pi_cli" --mode rpc --no-session \
    | head -5; then
    echo "warning: rpc get_state smoke timed out or failed (non-fatal if --no-session unsupported)" >&2
  fi
fi
rm -rf "$smoke_home"

echo "AppImage includes vendored Node + Pi. Global pi on PATH is not required."
echo "OpenCode seed (if present) is in agent-runtime/vendor/seed/ — gitignored."
