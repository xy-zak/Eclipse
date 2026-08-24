#!/usr/bin/env bash
# One-command setup: toolchain, sidecars, npm install. Does not launch the GUI.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

if [[ ! -f vendor/sunshine/CMakeLists.txt || ! -f vendor/moonlight-qt/moonlight-qt.pro ]]; then
  echo "Initializing git submodules..."
  git submodule update --init --recursive
fi

"${ROOT}/scripts/bootstrap.sh"

if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi
export PATH="${HOME}/.local/bin:/usr/local/cuda/bin:/opt/cuda/bin:${PATH}"

"${ROOT}/scripts/build-sidecars.sh"
npm install

echo
echo "Setup complete. Start the app with:"
echo "  npm run tauri -- dev"
