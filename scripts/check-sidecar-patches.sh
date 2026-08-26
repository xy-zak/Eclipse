#!/usr/bin/env bash
# Apply sidecar patches to a clean vendor pin (CI). Always reset to vendor/PINS
# first so a partially patched tree cannot false-pass.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

pin_sha() {
  local vendor_rel="$1"
  awk -v p="${vendor_rel}" 'NF && $1 !~ /^#/ && $1 == p { print $2; exit }' "${ROOT}/vendor/PINS"
}

reset_vendor_to_pin() {
  local repo="$1"
  local vendor_rel="$2"
  local sha
  sha="$(pin_sha "${vendor_rel}")"
  if [[ -z "${sha}" ]]; then
    echo "No pin for ${vendor_rel} in vendor/PINS" >&2
    exit 1
  fi
  echo "==> Resetting ${vendor_rel} to pin ${sha}"
  git -C "${repo}" reset --hard "${sha}"
  git -C "${repo}" clean -fd
}

apply_one_patch() {
  local repo="$1"
  local patch="$2"
  local name
  name="$(basename "${patch}")"
  echo "Applying ${name}"
  if ! git -C "${repo}" apply --whitespace=nowarn "${patch}"; then
    echo "git apply failed for ${name}" >&2
    return 1
  fi
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

reset_vendor_to_pin "${ROOT}/vendor/sunshine" "vendor/sunshine"
apply_series "${ROOT}/vendor/sunshine" "${sun_patches[@]}"

reset_vendor_to_pin "${ROOT}/vendor/moonlight-qt" "vendor/moonlight-qt"
apply_series "${ROOT}/vendor/moonlight-qt" "${moon_patches[@]}"

if ! grep -q 'exclamation\.svg' "${ROOT}/vendor/moonlight-qt/app/gui/NavigableMessageDialog.qml" \
  || ! grep -q 'frameColor' "${ROOT}/vendor/moonlight-qt/app/gui/NavigableMessageDialog.qml"; then
  echo "moonlight-02-ui.patch incomplete: NavigableMessageDialog.qml missing Eclipse dialog chrome" >&2
  exit 1
fi
if ! grep -q 'JetBrains Mono' "${ROOT}/vendor/moonlight-qt/app/main.cpp"; then
  echo "moonlight-01-behavior.patch incomplete: main.cpp missing JetBrains Mono app font" >&2
  exit 1
fi

echo "Sidecar patches apply cleanly."
