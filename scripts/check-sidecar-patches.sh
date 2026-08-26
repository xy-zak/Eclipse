#!/usr/bin/env bash
# Apply sidecar patches to a clean vendor tree (CI). Idempotent if already applied,
# including when a later patch overlaps an earlier one (same rule as build-sidecars).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

apply_one_patch() {
  local repo="$1"
  local patch="$2"
  local name
  name="$(basename "${patch}")"
  if git -C "${repo}" apply --reverse --check "${patch}" >/dev/null 2>&1; then
    echo "Already applied ${name}"
    return 0
  fi
  echo "Applying ${name}"
  if git -C "${repo}" apply --whitespace=nowarn "${patch}" >/dev/null 2>&1; then
    return 0
  fi
  if ! git -C "${repo}" apply --check --whitespace=nowarn "${patch}" >/dev/null 2>&1; then
    echo "Already applied ${name} (later patches overlap)"
    return 0
  fi
  echo "git apply failed for ${name}" >&2
  return 1
}

apply_series() {
  local repo="$1"
  shift
  local patch
  local found=0
  if [[ "$#" -eq 0 ]]; then
    echo "No patches given for ${repo}" >&2
    exit 1
  fi
  for patch in "$@"; do
    found=1
    apply_one_patch "${repo}" "${patch}"
  done
  if [[ "${found}" -eq 0 ]]; then
    echo "No patches for ${repo}" >&2
    exit 1
  fi
}

bash "${ROOT}/scripts/check-pins.sh"

shopt -s nullglob
sun_patches=( "${ROOT}/patches/sunshine-"*.patch )
moon_patches=( "${ROOT}/patches/moonlight-"*.patch )
shopt -u nullglob

apply_series "${ROOT}/vendor/sunshine" "${sun_patches[@]}"
apply_series "${ROOT}/vendor/moonlight-qt" "${moon_patches[@]}"

echo "Sidecar patches apply cleanly."
