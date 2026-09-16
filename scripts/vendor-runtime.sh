#!/usr/bin/env bash
# Download/copy portable Node 22 (linux amd64) and install Pi into
# agent-runtime/vendor/{node,pi}. Not committed — packaging/dev only.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="${ROOT}/agent-runtime/vendor"
NODE_VERSION="${NODE_VERSION:-22.23.2}"
PI_VERSION="${PI_VERSION:-0.85.1}"
PI_PKG="@earendil-works/pi-coding-agent@${PI_VERSION}"
CACHE="${ROOT}/packaging/cache"
NODE_DIST="node-v${NODE_VERSION}-linux-x64"
TARBALL="${NODE_DIST}.tar.xz"
TARBALL_URL="https://nodejs.org/dist/v${NODE_VERSION}/${TARBALL}"
EXISTING_NODE="${HOME}/.local/share/pi-node/${NODE_DIST}"

mkdir -p "${VENDOR}/node/bin" "${VENDOR}/pi" "${CACHE}"

log() { printf 'vendor-runtime: %s\n' "$*"; }

copy_node_bin() {
  local src="$1"
  if [[ -x "$src" ]]; then
    cp -f "$src" "${VENDOR}/node/bin/node"
    chmod 755 "${VENDOR}/node/bin/node"
    return 0
  fi
  return 1
}

if [[ -x "${VENDOR}/node/bin/node" ]] && "${VENDOR}/node/bin/node" -e "process.exit(process.version.startsWith('v22')?0:1)"; then
  log "reusing ${VENDOR}/node/bin/node ($("${VENDOR}/node/bin/node" -v))"
else
  if copy_node_bin "${EXISTING_NODE}/bin/node"; then
    log "copied Node ${NODE_VERSION} from ${EXISTING_NODE}"
  else
    if [[ ! -f "${CACHE}/${TARBALL}" ]]; then
      log "downloading ${TARBALL_URL}"
      curl -fsSL "${TARBALL_URL}" -o "${CACHE}/${TARBALL}.partial"
      mv "${CACHE}/${TARBALL}.partial" "${CACHE}/${TARBALL}"
    fi
    log "extracting ${TARBALL}"
    rm -rf "${CACHE}/${NODE_DIST}"
    tar -xJf "${CACHE}/${TARBALL}" -C "${CACHE}"
    copy_node_bin "${CACHE}/${NODE_DIST}/bin/node"
    log "installed Node $("${VENDOR}/node/bin/node" -v)"
  fi
fi

NODE_BIN="${VENDOR}/node/bin/node"
if [[ -x "${EXISTING_NODE}/bin/npm" ]]; then
  NPM=( "${EXISTING_NODE}/bin/npm" )
elif [[ -x "${CACHE}/${NODE_DIST}/bin/npm" ]]; then
  NPM=( "${CACHE}/${NODE_DIST}/bin/npm" )
else
  # bootstrap npm via the official tarball (needed once)
  if [[ ! -f "${CACHE}/${TARBALL}" ]]; then
    log "downloading ${TARBALL_URL} (npm bootstrap)"
    curl -fsSL "${TARBALL_URL}" -o "${CACHE}/${TARBALL}.partial"
    mv "${CACHE}/${TARBALL}.partial" "${CACHE}/${TARBALL}"
  fi
  rm -rf "${CACHE}/${NODE_DIST}"
  tar -xJf "${CACHE}/${TARBALL}" -C "${CACHE}"
  NPM=( "${CACHE}/${NODE_DIST}/bin/npm" )
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
  # Remove any symlink first — `cat > bin/pi` would otherwise clobber cli.js.
  rm -f "${VENDOR}/pi/bin/pi"
  cat > "${VENDOR}/pi/bin/pi" <<EOF
#!/bin/sh
here=\$(CDPATH= cd -- "\$(dirname "\$0")" && pwd)
cli="\$here/${cli}"
node="\$here/../../node/bin/node"
if [ ! -x "\$node" ]; then
  node=\$(command -v node) || exit 127
fi
exec "\$node" "\$cli" "\$@"
EOF
  chmod 755 "${VENDOR}/pi/bin/pi"
}

NEED_PI=1
if pkg="$(pi_pkg_json)"; then
  have="$("$NODE_BIN" -e "console.log(require('${pkg}').version)" 2>/dev/null || true)"
  if [[ "$have" == "$PI_VERSION" ]]; then
    NEED_PI=0
    log "reusing Pi ${have}"
  fi
fi

if [[ "$NEED_PI" -eq 1 ]]; then
  log "npm install --ignore-scripts ${PI_PKG}"
  rm -rf "${VENDOR}/pi"
  mkdir -p "${VENDOR}/pi"
  PATH="$(dirname "${NPM[0]}"):${PATH}" \
    "${NPM[0]}" install --prefix "${VENDOR}/pi" --ignore-scripts --omit=dev "${PI_PKG}"
  find "${VENDOR}/pi" -type f -name '*.map' -delete
  pkg="$(pi_pkg_json)"
  log "installed Pi $("$NODE_BIN" -e "console.log(require('${pkg}').version)")"
fi

ensure_pi_bin
if [[ ! -e "${VENDOR}/pi/bin/pi" ]]; then
  log "ERROR: vendor/pi/bin/pi missing"
  exit 1
fi

# Local-machine OpenCode seed (gitignored). Never write secrets into the repo.
SEED_DIR="${VENDOR}/seed"
PACK_SEED="${ROOT}/packaging/seed"
mkdir -p "$SEED_DIR" "$PACK_SEED"
if [[ -f /root/.pi/agent/auth.json ]]; then
  cp -f /root/.pi/agent/auth.json "${PACK_SEED}/opencode-auth.json"
  chmod 600 "${PACK_SEED}/opencode-auth.json"
  log "seeded packaging/seed/opencode-auth.json from /root/.pi/agent/auth.json"
fi
if [[ -f "${PACK_SEED}/opencode-auth.json" ]]; then
  cp -f "${PACK_SEED}/opencode-auth.json" "${SEED_DIR}/auth.json"
  chmod 600 "${SEED_DIR}/auth.json"
fi
# Product default: OpenCode + muse-spark (do not copy user plugin lists).
cat > "${SEED_DIR}/settings.json" <<'JSON'
{
  "defaultProvider": "opencode",
  "defaultModel": "muse-spark"
}
JSON
cp -f "${SEED_DIR}/settings.json" "${PACK_SEED}/settings.json"

log "node=$("$NODE_BIN" -v) pi=${VENDOR}/pi/bin/pi -> $(readlink -f "${VENDOR}/pi/bin/pi" 2>/dev/null || true)"
log "done"
