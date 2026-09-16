#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRATCH="${1:-/tmp/grok-goal-26b12f88ca7c/implementer}"
mkdir -p "$SCRATCH"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$PATH"

echo "== lib unit (runtime paths, spawn args) =="
cargo test --manifest-path src-tauri/Cargo.toml --lib -- --nocapture | tee "$SCRATCH/lib-test.log"

echo "== jsonl =="
cargo test --manifest-path src-tauri/Cargo.toml --test jsonl_framing -- --nocapture | tee "$SCRATCH/jsonl-test.log"

echo "== rpc =="
cargo test --manifest-path src-tauri/Cargo.toml --test rpc_client -- --nocapture | tee "$SCRATCH/rpc-test.log"

echo "== spawn =="
cargo test --manifest-path src-tauri/Cargo.toml --test spawn_plan -- --nocapture | tee "$SCRATCH/spawn-test.log"

echo "== runtime paths =="
cargo test --manifest-path src-tauri/Cargo.toml --test runtime_paths -- --nocapture | tee "$SCRATCH/runtime-paths-test.log"

echo "== ui-bridge =="
cargo test --manifest-path src-tauri/Cargo.toml --test ui_bridge -- --nocapture | tee "$SCRATCH/ui-bridge-test.log"

echo "== vitest security/ui/skills/golden =="
pnpm test -- tests/security.test.ts | tee "$SCRATCH/security-test.log"
pnpm test -- tests/ui-store.test.ts | tee "$SCRATCH/ui-store-test.log"
pnpm test -- tests/skills-check.test.ts | tee "$SCRATCH/skills-check.log"
pnpm test -- tests/golden-path-offline.test.ts | tee "$SCRATCH/golden-offline.log"

echo "== frontend build =="
pnpm build | tee "$SCRATCH/frontend-build.log"
pnpm test -- tests/frontend-entry.test.ts | tee -a "$SCRATCH/frontend-build.log"
node "$ROOT/scripts/frontend-build-check.mjs" | tee -a "$SCRATCH/frontend-build.log"