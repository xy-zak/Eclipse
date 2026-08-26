#!/usr/bin/env bash
# Create empty Tauri externalBin placeholders so cargo/tauri can build without
# compiling Sunshine/Moonlight. Real sidecars come from scripts/build-sidecars.sh.
# Used by CI (and locally when you only need adapter unit tests).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
triple="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "${triple}" ]]; then
  echo "Could not detect rustc host triple" >&2
  exit 1
fi

mkdir -p "${ROOT}/src-tauri/binaries"
for engine in sunshine moonlight; do
  path="${ROOT}/src-tauri/binaries/${engine}-${triple}"
  if [[ -e "${path}" ]]; then
    echo "keep existing ${path}"
  else
    : > "${path}"
    chmod +x "${path}"
    echo "stubbed ${path}"
  fi
done
