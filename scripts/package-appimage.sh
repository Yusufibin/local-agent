#!/usr/bin/env bash
# Phase 8: Linux AppImage. The GUI binary is bundled; Node + `pi` remain a
# runtime dependency on PATH (plan D8 / Phase 8: self-contained Node sidecar
# is optional and not shipped in this pass).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="${HOME}/.cargo/bin:${PATH}"

if ! command -v pnpm >/dev/null; then
  echo "pnpm is required" >&2
  exit 1
fi

echo "building AppImage (Tauri 2, identifier local.agent.desktop)"
pnpm tauri build --bundles appimage

bundle="${ROOT}/src-tauri/target/release/bundle/appimage"
if ! ls "${bundle}"/*.AppImage >/dev/null 2>&1; then
  echo "AppImage not produced under ${bundle}" >&2
  echo "Need linuxdeploy + webkit2gtk-4.1. Node + pi stay on PATH at runtime." >&2
  exit 1
fi
ls -lh "${bundle}"/*.AppImage
echo "Runtime deps (not bundled): Node 22.x and pi (npm i -g --ignore-scripts @earendil-works/pi-coding-agent)"
echo "Override product runtime with DESKPI_RUNTIME if agent-runtime is not next to the binary."