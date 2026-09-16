#!/usr/bin/env bash
# Download/copy portable Node 22 + install Pi into agent-runtime/vendor/{node,pi}.
# Not committed — packaging/CI only. Seeds OpenCode from env or local ~/.pi.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="${ROOT}/agent-runtime/vendor"
NODE_VERSION="${NODE_VERSION:-22.23.2}"
PI_VERSION="${PI_VERSION:-0.85.1}"
PI_PKG="@earendil-works/pi-coding-agent@${PI_VERSION}"
CACHE="${ROOT}/packaging/cache"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
case "$OS" in
  mingw*|msys*|cygwin*|windows_nt) OS=win ;;
esac
if [[ "$OS" == win || "${OS}" == mingw64_nt* ]]; then
  OS=win
fi
# GitHub windows bash reports MINGW64_NT-*
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
  OS=win
fi

if [[ "$OS" == "linux" ]]; then
  NODE_DIST="node-v${NODE_VERSION}-linux-x64"
  TARBALL="${NODE_DIST}.tar.xz"
  TARBALL_URL="https://nodejs.org/dist/v${NODE_VERSION}/${TARBALL}"
  EXTRACT=(tar -xJf)
elif [[ "$OS" == "darwin" ]]; then
  NODE_DIST="node-v${NODE_VERSION}-darwin-x64"
  TARBALL="${NODE_DIST}.tar.gz"
  TARBALL_URL="https://nodejs.org/dist/v${NODE_VERSION}/${TARBALL}"
  EXTRACT=(tar -xzf)
elif [[ "$OS" == "win" ]]; then
  NODE_DIST="node-v${NODE_VERSION}-win-x64"
  TARBALL="${NODE_DIST}.zip"
  TARBALL_URL="https://nodejs.org/dist/v${NODE_VERSION}/${TARBALL}"
  EXTRACT=(unzip -qo)
else
  echo "unsupported OS: $(uname -s)" >&2
  exit 1
fi

EXISTING_NODE="${HOME}/.local/share/pi-node/${NODE_DIST}"
mkdir -p "${VENDOR}/node/bin" "${VENDOR}/pi" "${CACHE}"

log() { printf 'vendor-runtime: %s\n' "$*"; }

NODE_EXE=node
if [[ "$OS" == "win" ]]; then
  NODE_EXE=node.exe
fi

copy_node_bin() {
  local src="$1"
  if [[ -f "$src" ]]; then
    cp -f "$src" "${VENDOR}/node/bin/${NODE_EXE}"
    chmod 755 "${VENDOR}/node/bin/${NODE_EXE}" 2>/dev/null || true
    # convenience symlink/copy as node for scripts
    if [[ "$OS" == "win" && "${NODE_EXE}" == "node.exe" ]]; then
      cp -f "$src" "${VENDOR}/node/bin/node" 2>/dev/null || true
    fi
    return 0
  fi
  return 1
}

if [[ -x "${VENDOR}/node/bin/${NODE_EXE}" ]] || [[ -f "${VENDOR}/node/bin/${NODE_EXE}" ]]; then
  if "${VENDOR}/node/bin/${NODE_EXE}" -e "process.exit(process.version.startsWith('v22')?0:1)" 2>/dev/null; then
    log "reusing ${VENDOR}/node/bin/${NODE_EXE} ($("${VENDOR}/node/bin/${NODE_EXE}" -v))"
    SKIP_NODE=1
  fi
fi

if [[ "${SKIP_NODE:-0}" != "1" ]]; then
  if copy_node_bin "${EXISTING_NODE}/bin/${NODE_EXE}" || copy_node_bin "${EXISTING_NODE}/node.exe"; then
    log "copied Node ${NODE_VERSION} from ${EXISTING_NODE}"
  else
    if [[ ! -f "${CACHE}/${TARBALL}" ]]; then
      log "downloading ${TARBALL_URL}"
      curl -fsSL "${TARBALL_URL}" -o "${CACHE}/${TARBALL}.partial"
      mv "${CACHE}/${TARBALL}.partial" "${CACHE}/${TARBALL}"
    fi
    log "extracting ${TARBALL}"
    rm -rf "${CACHE}/${NODE_DIST}"
    if [[ "$OS" == "win" ]]; then
      unzip -qo "${CACHE}/${TARBALL}" -d "${CACHE}"
    else
      "${EXTRACT[@]}" "${CACHE}/${TARBALL}" -C "${CACHE}"
    fi
    if [[ "$OS" == "win" ]]; then
      copy_node_bin "${CACHE}/${NODE_DIST}/${NODE_EXE}" || copy_node_bin "${CACHE}/${NODE_DIST}/node.exe"
    else
      copy_node_bin "${CACHE}/${NODE_DIST}/bin/node"
    fi
    log "installed Node $("${VENDOR}/node/bin/${NODE_EXE}" -v)"
  fi
fi

NODE_BIN="${VENDOR}/node/bin/${NODE_EXE}"

if [[ "$OS" == "win" ]]; then
  if [[ -x "${EXISTING_NODE}/npm.cmd" ]]; then
    NPM=( "${EXISTING_NODE}/npm.cmd" )
  elif [[ -f "${CACHE}/${NODE_DIST}/npm.cmd" ]]; then
    NPM=( "${CACHE}/${NODE_DIST}/npm.cmd" )
  else
    NPM=( npm )
  fi
else
  if [[ -x "${EXISTING_NODE}/bin/npm" ]]; then
    NPM=( "${EXISTING_NODE}/bin/npm" )
  elif [[ -x "${CACHE}/${NODE_DIST}/bin/npm" ]]; then
    NPM=( "${CACHE}/${NODE_DIST}/bin/npm" )
  else
    if [[ ! -f "${CACHE}/${TARBALL}" ]]; then
      log "downloading ${TARBALL_URL} (npm bootstrap)"
      curl -fsSL "${TARBALL_URL}" -o "${CACHE}/${TARBALL}.partial"
      mv "${CACHE}/${TARBALL}.partial" "${CACHE}/${TARBALL}"
    fi
    rm -rf "${CACHE}/${NODE_DIST}"
    "${EXTRACT[@]}" "${CACHE}/${TARBALL}" -C "${CACHE}"
    NPM=( "${CACHE}/${NODE_DIST}/bin/npm" )
  fi
fi

pi_pkg_json() {
  for p in \
    "${VENDOR}/pi/lib/node_modules/@earendil-works/pi-coding-agent/package.json" \
    "${VENDOR}/pi/node_modules/@earendil-works/pi-coding-agent/package.json"; do
    if [[ -f "$p" ]]; then
      printf '%s\n' "$p"
      return 0
    fi
  done
  return 1
}

ensure_pi_bin() {
  mkdir -p "${VENDOR}/pi/bin"
  local cli=""
  if [[ -f "${VENDOR}/pi/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js" ]]; then
    cli="../node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
  elif [[ -f "${VENDOR}/pi/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js" ]]; then
    cli="../lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
  else
    return 1
  fi
  rm -f "${VENDOR}/pi/bin/pi" "${VENDOR}/pi/bin/pi.cmd"
  cat > "${VENDOR}/pi/bin/pi" <<EOF
#!/bin/sh
here=\$(CDPATH= cd -- "\$(dirname "\$0")" && pwd)
cli="\$here/${cli}"
node="\$here/../../node/bin/node"
if [ -x "\$here/../../node/bin/node.exe" ]; then
  node="\$here/../../node/bin/node.exe"
fi
if [ ! -x "\$node" ] && [ ! -f "\$node" ]; then
  node=\$(command -v node) || exit 127
fi
exec "\$node" "\$cli" "\$@"
EOF
  chmod 755 "${VENDOR}/pi/bin/pi" 2>/dev/null || true
  if [[ "$OS" == "win" ]]; then
    cat > "${VENDOR}/pi/bin/pi.cmd" <<EOF
@echo off
set HERE=%~dp0
set CLI=%HERE%${cli}
set NODE=%HERE%..\..\node\bin\node.exe
if not exist "%NODE%" set NODE=node
"%NODE%" "%CLI%" %*
EOF
  fi
}

NEED_PI=1
if pkg="$(pi_pkg_json)"; then
  have="$("$NODE_BIN" -e "console.log(require('${pkg//\'/}').version)" 2>/dev/null || true)"
  # fallback without broken escaping
  have="$("$NODE_BIN" -e "console.log(require(process.argv[1]).version)" "$pkg" 2>/dev/null || true)"
  if [[ "$have" == "$PI_VERSION" ]]; then
    NEED_PI=0
    log "reusing Pi ${have}"
  fi
fi

if [[ "$NEED_PI" -eq 1 ]]; then
  log "npm install --ignore-scripts ${PI_PKG}"
  rm -rf "${VENDOR}/pi"
  mkdir -p "${VENDOR}/pi"
  if [[ "$OS" == "win" ]]; then
    "${NPM[@]}" install --prefix "${VENDOR}/pi" --ignore-scripts --omit=dev "${PI_PKG}"
  else
    PATH="$(dirname "${NPM[0]}"):${PATH}" \
      "${NPM[0]}" install --prefix "${VENDOR}/pi" --ignore-scripts --omit=dev "${PI_PKG}"
  fi
  find "${VENDOR}/pi" -type f -name '*.map' -delete || true
  pkg="$(pi_pkg_json)"
  log "installed Pi $("$NODE_BIN" -e "console.log(require(process.argv[1]).version)" "$pkg")"
fi

ensure_pi_bin
if [[ ! -e "${VENDOR}/pi/bin/pi" ]]; then
  log "ERROR: vendor/pi/bin/pi missing"
  exit 1
fi

# OpenCode seed — never commit real keys. Prefer CI env, then packaging/seed, then ~/.pi
SEED_DIR="${VENDOR}/seed"
PACK_SEED="${ROOT}/packaging/seed"
mkdir -p "$SEED_DIR" "$PACK_SEED"

if [[ -n "${OPENCODE_AUTH_JSON:-}" ]]; then
  printf '%s\n' "$OPENCODE_AUTH_JSON" > "${PACK_SEED}/opencode-auth.json"
  chmod 600 "${PACK_SEED}/opencode-auth.json" 2>/dev/null || true
  log "seeded packaging/seed/opencode-auth.json from OPENCODE_AUTH_JSON"
elif [[ -n "${OPENCODE_API_KEY:-}" ]]; then
  python - "$PACK_SEED" <<'PY'
import json, os, sys
pack = sys.argv[1]
key = os.environ["OPENCODE_API_KEY"]
doc = {
  "opencode": {"type": "api_key", "key": key},
  "opencode-go": {"type": "api_key", "key": key},
}
path = os.path.join(pack, "opencode-auth.json")
open(path, "w").write(json.dumps(doc, indent=2) + "\n")
PY
  chmod 600 "${PACK_SEED}/opencode-auth.json" 2>/dev/null || true
  log "seeded packaging/seed/opencode-auth.json from OPENCODE_API_KEY"
elif [[ -f "${HOME}/.pi/agent/auth.json" ]]; then
  cp -f "${HOME}/.pi/agent/auth.json" "${PACK_SEED}/opencode-auth.json"
  chmod 600 "${PACK_SEED}/opencode-auth.json" 2>/dev/null || true
  log "seeded packaging/seed/opencode-auth.json from ~/.pi/agent/auth.json"
elif [[ -f /root/.pi/agent/auth.json ]]; then
  cp -f /root/.pi/agent/auth.json "${PACK_SEED}/opencode-auth.json"
  chmod 600 "${PACK_SEED}/opencode-auth.json" 2>/dev/null || true
  log "seeded packaging/seed/opencode-auth.json from /root/.pi/agent/auth.json"
fi

if [[ -f "${PACK_SEED}/opencode-auth.json" ]]; then
  cp -f "${PACK_SEED}/opencode-auth.json" "${SEED_DIR}/auth.json"
  chmod 600 "${SEED_DIR}/auth.json" 2>/dev/null || true
fi
cat > "${SEED_DIR}/settings.json" <<'JSON'
{
  "defaultProvider": "opencode",
  "defaultModel": "muse-spark"
}
JSON
cp -f "${SEED_DIR}/settings.json" "${PACK_SEED}/settings.json"

log "node=$("$NODE_BIN" -v) pi=${VENDOR}/pi/bin/pi"
log "done"
