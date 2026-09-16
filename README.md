# Local Agent

Desktop host for [Pi coding agent](https://pi.dev) (`pi --mode rpc`). The GUI is **Tauri 2 + Svelte 5**. Rust is a thin JSONL multiplexer. LLM calls stay inside the Pi sidecar.

## Versions (pinned at scaffold)

| Piece | Version |
|---|---|
| Tauri / `@tauri-apps/cli` | 2.x (`@tauri-apps/cli` 2.11.4) |
| `@tauri-apps/plugin-dialog` / `notification` | 2.x |
| Svelte | 5.x (5.56.3) |
| Vite | 7.x |
| Node | 22.x LTS |
| `@earendil-works/pi-coding-agent` | **0.85.1** (tested 2026-09-16) |

## Node + Pi (bundled)

Linux amd64 AppImage ships a portable **Node 22 LTS** and `@earendil-works/pi-coding-agent` **0.85.1** under `agent-runtime/vendor/`. No `pi` / Node on the user PATH is required.

Resolution: `PI_BIN` → `agent-runtime/vendor/pi/bin/pi` (exec with vendored `node`) → `PATH` (dev fallback).

```bash
bash scripts/vendor-runtime.sh   # fills agent-runtime/vendor/ (gitignored)
pnpm tauri dev                   # PATH fallback if vendor/ is absent
```

`--ignore-scripts` is the official Pi install advice if you use a global `pi` for development.

## Develop

```bash
pnpm install
pnpm tauri dev
```

Window: **1100×720**, min **800×560**, light snow theme (`#F7F9FC`). Sidebar / keys / model / thinking / settings are hidden until the top-left hamburger (☰). Identifier: `local.agent.desktop`.

## Runtime paths (portable)

The host never bakes a checkout path into the binary. `agent-runtime` is resolved at process start:

1. `DESKPI_RUNTIME` (must be a valid runtime tree)
2. Tauri resource dir (`agent-runtime/` and `_up_/agent-runtime`)
3. `APPDIR` (AppImage)
4. Walk-up from the executable and the current working directory

Optional: `DESKPI_WORKSPACE` forces the initial workspace root. In a source checkout, if `tests/fixtures/Fred-Projet` exists, the default root is `tests/fixtures` (parent of the fixture) so `trouver-dossier` can discover it.

Spawn isolation: `--no-extensions`, `--no-skills`, `--no-prompt-templates`, then explicit `-e` / `--skill` / `--prompt-template` (`/recap`). `--approve` is not passed unless `expertApprove` is set.

## Tests

```bash
pnpm test
pnpm test:rust
pnpm golden:offline  # Phase 5 fixture procedure (no LLM)
pnpm golden          # Phase 5 live recap (needs pi + provider credits)
```

Do not call a live LLM in CI. Tests use a fake JSONL sidecar and the shipped policy modules. `pnpm golden` is the opt-in live path.

## Security checklist (Phase 4)

Verified 2026-09-16 against shipped `agent-runtime` extensions (workspace-roots → protected-paths → permission-gate) and `kill_group` (process group SIGTERM then SIGKILL). Re-run: `pnpm test -- tests/security.test.ts` and `cargo test --manifest-path src-tauri/Cargo.toml --test spawn_plan kill_group_reaps_bash_grandchild`.

- [x] `read /etc/passwd` → block
- [x] `read <root>/.env` → block
- [x] `write <root>/../escape.txt` → block
- [x] `write <root>/ok.md` en ask → modal → Allow → fichier créé
- [x] même write → Block → pas de fichier
- [x] `bash: rm -rf /tmp/x` → modal même en full
- [x] readonly + `write` → block sans modal
- [x] tuer l’app pendant un bash → pas de process orphelin

Security is enforced in Pi extensions (`workspace-roots`, `permission-gate`, `protected-paths`) plus the GUI approval bridge — not a parallel Rust permission engine. Product extensions load only via `-e`. Do not pass `--approve` unless `expertApprove` is set in `{app_data}/settings.json`.

API keys live in `{app_data}/secrets.json` (chmod 600), never in `settings.json`, never in the webview. Pi auth is isolated with official `PI_CODING_AGENT_DIR={app_data}/pi-home/.pi/agent` (HOME is not rewritten, so bash `~` stays the real user home). First launch copies gitignored `packaging/seed/opencode-auth.json` (or `vendor/seed/auth.json` baked at package time from the build machine) if that file does not exist yet. Settings UI remains the way to update the key. **Never commit API keys.**

## Golden path (Phase 5)

Fixture: `tests/fixtures/Fred-Projet/` (incomplete CLI, open TODOs: argparse + tests). Default workspace root in a checkout is `tests/fixtures` so `trouver-dossier` can discover `Fred-Projet`.

```bash
pnpm golden:offline   # procedure + fixture, no LLM
pnpm golden           # live `pi --mode rpc` (needs provider credits)
```

Offline runs the skill procedure: `find <root> -maxdepth 5 -iname '*fred*'`, sample README/todo/notes/src, write `RECAP.md` with Vue d’ensemble / Contenu / État / Manques / Actions, mentioning the open TODOs. Artifact: `tests/golden-path-output.md`.

Live spawn uses the product `-e` / `--skill` / `/recap` runtime, prompts **fais-moi un récap du dossier de fred**, auto-allows in-root writes, blocks dangerous bash. zai/opencode on this machine returned 429/401 (no credits) when last tried.

In the GUI: pick a folder that **contains** `Fred-Projet`, permission **ask**, then the same sentence.

## Stability (Phase 7)

Hardening after the golden path. Does **not** change AppImage packaging.

| Symptom | Cause | Fix |
|---|---|---|
| UI frozen while Pi is busy or hung | Sidecar mutex held across RPC wait (prompt ack has no timeout) | Clone the RPC handle, drop the mutex, then wait. `abort` / `restart` can run. Reader exit fails pending waiters. |
| Window gone from the desktop, `deskpi` still running | GTK unmap after folder dialog, tiny/off-screen geometry, or webview death without `CloseRequested` | Parent the folder dialog on `main`, restore/show/focus after pick, 2s window watchdog (`!visible && !minimized` only — user minimize is left alone), recreate `main` on unexpected `Destroyed`, `app.exit(0)` on user close so the host is not a zombie. |
| Sidecar dead, composer still “running” | Crash only noticed on the next `get_state` | 500ms `health_tick`: `try_wait` → `process: crashed` + stderr tail. Banner + **Restart sidecar**. Last prompt is **not** replayed. |
| LLM / sidecar silent mid-run | No events for 10 minutes while streaming | Watchdog event + banner + Restart. **No auto-kill** (a long `bash` is legitimate). |
| `host.log` / `pi.stderr.log` grow forever | Append-only | Rotate at 5 MiB, keep 3 backups (`host.log.1` …). |
| Transcript freeze on a long recap | Unbounded `{#each}` + huge tool bodies | Render the last 200 messages (“Load earlier”), cap tool-card body at 32 KiB, cap `get_messages` at 200 for the webview. |
| `path outside workspace roots` then success | Agent `find`/`grep` on `/` or `$HOME` | Expected block. Overlay + skill say to pass `path` = a listed root. `find`/`grep` with no path default to cwd. Workspace roots are canonicalized on spawn. |

Logs: `{app_data}/logs/host.log`, `{app_data}/logs/pi.stderr.log`.

Crash / silence: banner in the main column, button **Restart sidecar** (`agent_restart` = kill process group then spawn + health `get_state`). Compaction still uses the “Compacting context…” banner.

## Packaging (Phase 8)

Linux AppImage / .deb. Node 22 + Pi 0.85.1 are vendored into the bundle (`agent-runtime/vendor/`).

```bash
pnpm package:appimage
# → src-tauri/target/release/bundle/appimage/Local Agent_0.1.0_amd64.AppImage
```

`scripts/package-appimage.sh` runs `vendor-runtime.sh` first (Node tarball + `npm i --ignore-scripts @earendil-works/pi-coding-agent@0.85.1`), copies a local OpenCode seed from `/root/.pi/agent/auth.json` when present into gitignored `packaging/seed/` and `agent-runtime/vendor/seed/`, then `tauri build --bundles appimage`. Smoke: vendored `node` + `pi --version` / RPC `get_state` with `PATH=/usr/bin:/bin` (no global `pi`).

Produced on 2026-09-16: **134 MiB** AppImage. `agent-runtime` lands at `usr/lib/Local Agent/agent-runtime` (extensions, skills, `/recap`, `vendor/node`, `vendor/pi`). If resolution fails at launch, set `DESKPI_RUNTIME`.
