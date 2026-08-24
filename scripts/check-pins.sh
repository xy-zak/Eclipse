#!/usr/bin/env bash
# Fail if vendored engine SHAs do not match vendor/PINS.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PINS="${ROOT}/vendor/PINS"
fail=0

while read -r path sha label; do
  [[ -z "${path}" || "${path}" == \#* ]] && continue
  actual="$(git -C "${ROOT}/${path}" rev-parse HEAD)"
  if [[ "${actual}" != "${sha}" ]]; then
    echo "PIN MISMATCH ${path}: expected ${sha} (${label}), got ${actual}" >&2
    fail=1
  else
    echo "OK ${path} ${sha} (${label})"
  fi
done < <(awk 'NF && $1 !~ /^#/' "${PINS}")

if [[ "${fail}" -ne 0 ]]; then
  echo "Fix: checkout the pinned commit, or update vendor/PINS with the new SHA." >&2
  echo "Do not reset Moonlight to tag v6.1.0 (FFmpeg 8)." >&2
  exit 1
fi
