# Vendored Node + Pi (Linux amd64)

Filled by `scripts/vendor-runtime.sh`. **Not committed.**

| Path | What |
|---|---|
| `node/bin/node` | Node 22 LTS portable |
| `pi/bin/pi` | `@earendil-works/pi-coding-agent` (see README pin, 0.85.1) |
| `seed/` | Optional OpenCode `auth.json` / `settings.json` copied at package time |

Host resolution: `PI_BIN` → `vendor/pi/bin/pi` (spawned with vendored Node) → `PATH`.

Pi auth is isolated with official `PI_CODING_AGENT_DIR={app_data}/pi-home/.pi/agent` (HOME is not rewritten). Seed files here are gitignored; never put API keys in this README or the repo.
