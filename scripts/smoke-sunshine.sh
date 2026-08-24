#!/usr/bin/env bash
# Spawn the built Sunshine sidecar and require HTTPS on 127.0.0.1:47990.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRIPLE="$(rustc --print host-tuple 2>/dev/null || rustc -vV | awk '/^host:/{print $2}')"
BIN="${ROOT}/src-tauri/binaries/sunshine-${TRIPLE}"
ASSETS="${ROOT}/src-tauri/resources/sunshine"
URL="https://127.0.0.1:47990"
PORT=47990

if [[ ! -x "${BIN}" ]]; then
  echo "Missing ${BIN}. Run ./scripts/build-sidecars.sh first." >&2
  exit 1
fi

port_open() {
  (echo >/dev/tcp/127.0.0.1/${PORT}) >/dev/null 2>&1
}

http_code() {
  curl -k -s -o /dev/null -w '%{http_code}' --max-time 3 "${URL}" 2>/dev/null || echo 000
}

if port_open; then
  code="$(http_code)"
  if [[ "${code}" != "000" && -n "${code}" ]]; then
    echo "Sunshine already answering on ${URL} (HTTP ${code}). Stop Eclipse if you need a clean spawn."
    exit 0
  fi
  echo "Port ${PORT} is open but HTTPS ${URL} did not respond." >&2
  exit 1
fi

cleanup() {
  if [[ -n "${SUN_PID:-}" ]]; then
    kill "${SUN_PID}" 2>/dev/null || true
    wait "${SUN_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

cmd=("${BIN}")
if [[ -d "${ASSETS}" ]]; then
  export SUNSHINE_ASSETS_DIR="${ASSETS}"
  cd "$(dirname "${BIN}")"
fi
"${cmd[@]}" >/tmp/eclipse-sunshine-smoke.log 2>&1 &
SUN_PID=$!

for _ in $(seq 1 40); do
  code="$(http_code)"
  if [[ "${code}" != "000" && -n "${code}" ]]; then
    echo "OK Sunshine HTTPS ${URL} -> HTTP ${code}"
    exit 0
  fi
  if ! kill -0 "${SUN_PID}" 2>/dev/null; then
    echo "Sunshine exited before opening ${URL}. Log:" >&2
    cat /tmp/eclipse-sunshine-smoke.log >&2 || true
    exit 1
  fi
  sleep 0.5
done

echo "Sunshine did not answer ${URL} within 20s. Log:" >&2
cat /tmp/eclipse-sunshine-smoke.log >&2 || true
exit 1
