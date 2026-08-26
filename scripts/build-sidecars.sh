#!/usr/bin/env bash
# Build Sunshine and Moonlight and copy Tauri sidecars for this host triple.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# CUDA toolkit bins (bootstrap may have installed nvcc here).
for _cuda_bin in /usr/local/cuda/bin /opt/cuda/bin /usr/lib/cuda/bin; do
  if [[ -d "${_cuda_bin}" ]]; then
    case ":${PATH}:" in
      *":${_cuda_bin}:"*) ;;
      *) export PATH="${_cuda_bin}:${PATH}" ;;
    esac
  fi
done
unset _cuda_bin
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi
if [[ -d "${HOME}/.local/bin" ]]; then
  case ":${PATH}:" in
    *":${HOME}/.local/bin:"*) ;;
    *) export PATH="${HOME}/.local/bin:${PATH}" ;;
  esac
fi

SUNSHINE_SRC="${ROOT}/vendor/sunshine"
MOONLIGHT_SRC="${ROOT}/vendor/moonlight-qt"
BUILD_SUN="${ROOT}/build/sunshine"
BUILD_MOON="${ROOT}/build/moonlight"
BIN_DIR="${ROOT}/src-tauri/binaries"
ASSET_DIR="${ROOT}/src-tauri/resources/sunshine"
PREFIX="${ROOT}/src-tauri/resources/sunshine-prefix"

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required tool: $1" >&2
    echo "Run scripts/bootstrap.sh first." >&2
    exit 1
  fi
}

need rustc
need cmake
need ninja
need qmake6
need git

if [[ ! -f "${SUNSHINE_SRC}/CMakeLists.txt" || ! -f "${MOONLIGHT_SRC}/moonlight-qt.pro" ]]; then
  echo "Vendored engines missing. Clone with: git submodule update --init --recursive" >&2
  exit 1
fi

TRIPLE="$(rustc --print host-tuple 2>/dev/null || true)"
if [[ -z "${TRIPLE}" ]]; then
  TRIPLE="$(rustc -vV | awk '/^host:/{print $2}')"
fi
echo "Host triple: ${TRIPLE}"

CUDA_MODE="${ECLIPSE_CUDA:-auto}"
USE_CUDA=OFF
if [[ "${CUDA_MODE}" == "on" ]]; then
  USE_CUDA=ON
elif [[ "${CUDA_MODE}" == "auto" ]] && command -v nvcc >/dev/null 2>&1; then
  USE_CUDA=ON
fi
echo "Sunshine CUDA: ${USE_CUDA} (override with ECLIPSE_CUDA=on|off)"

if [[ ! -f "${SUNSHINE_SRC}/.git" && ! -d "${SUNSHINE_SRC}/.git" ]]; then
  git -C "${ROOT}" submodule update --init --recursive
fi

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
  # Keep gitignored prebuilts (moonlight libs/).
  git -C "${repo}" clean -fd
}

apply_one_patch() {
  local repo="$1"
  local patch="$2"
  local name
  name="$(basename "${patch}")"
  echo "==> Applying ${name}"
  if ! git -C "${repo}" apply --whitespace=nowarn "${patch}"; then
    echo "git apply failed for ${name}. Vendor tree must be the clean pin." >&2
    return 1
  fi
}

apply_sunshine_patches() {
  local patch
  shopt -s nullglob
  local found=0
  reset_vendor_to_pin "${SUNSHINE_SRC}" "vendor/sunshine"
  for patch in "${ROOT}/patches/sunshine-"*.patch; do
    found=1
    apply_one_patch "${SUNSHINE_SRC}" "${patch}"
  done
  if [[ "${found}" -eq 0 ]]; then
    echo "No patches/sunshine-*.patch files found" >&2
    exit 1
  fi
}

apply_moonlight_patches() {
  local patch
  shopt -s nullglob
  local found=0
  reset_vendor_to_pin "${MOONLIGHT_SRC}" "vendor/moonlight-qt"
  for patch in "${ROOT}/patches/moonlight-"*.patch; do
    found=1
    apply_one_patch "${MOONLIGHT_SRC}" "${patch}"
  done
  if [[ "${found}" -eq 0 ]]; then
    echo "No patches/moonlight-*.patch files found" >&2
    exit 1
  fi
  # Guard against a silent partial UI apply (Material colors without dialog chrome).
  if ! grep -q 'exclamation\.svg' "${MOONLIGHT_SRC}/app/gui/NavigableMessageDialog.qml" \
    || ! grep -q 'frameColor' "${MOONLIGHT_SRC}/app/gui/NavigableMessageDialog.qml"; then
    echo "moonlight-02-ui.patch incomplete: NavigableMessageDialog.qml missing Eclipse dialog chrome" >&2
    exit 1
  fi
  if ! grep -q 'JetBrains Mono' "${MOONLIGHT_SRC}/app/main.cpp"; then
    echo "moonlight-01-behavior.patch incomplete: main.cpp missing JetBrains Mono app font" >&2
    exit 1
  fi
}

# Sunshine FindUdev/FindSystemd write absolute /usr destinations; keep them in PREFIX.
keep_sunshine_install_in_prefix() {
  local inst="${BUILD_SUN}/cmake_install.cmake"
  [[ -f "${inst}" ]] || return 0
  mkdir -p \
    "${PREFIX}/lib/udev/rules.d" \
    "${PREFIX}/lib/systemd/user" \
    "${PREFIX}/lib/modules-load.d"
  if grep -qE '/usr/lib/(udev/rules\.d|systemd/user|modules-load\.d)' "${inst}"; then
    echo "==> Redirecting Sunshine udev/systemd install into ${PREFIX}"
    sed -i \
      -e "s#/usr/lib/udev/rules.d#${PREFIX}/lib/udev/rules.d#g" \
      -e "s#/usr/lib/systemd/user#${PREFIX}/lib/systemd/user#g" \
      -e "s#/usr/lib/modules-load.d#${PREFIX}/lib/modules-load.d#g" \
      "${inst}"
  fi
}

apply_sunshine_patches
apply_moonlight_patches

mkdir -p "${BIN_DIR}" "${ASSET_DIR}" "${BUILD_SUN}" "${BUILD_MOON}" "${PREFIX}"

echo "==> Configuring Sunshine"
cmake -S "${SUNSHINE_SRC}" -B "${BUILD_SUN}" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="${PREFIX}" \
  -DSUNSHINE_ASSETS_DIR=share/sunshine \
  -DSUNSHINE_ASSETS_DIR_DEF=share/sunshine \
  -DBUILD_DOCS=OFF \
  -DBUILD_TESTS=OFF \
  -DBUILD_WERROR=OFF \
  -DSUNSHINE_ENABLE_TRAY=OFF \
  -DSUNSHINE_ENABLE_CUDA="${USE_CUDA}" \
  -DCUDA_FAIL_ON_MISSING=OFF \
  -DUDEV_RULES_INSTALL_DIR="${PREFIX}/lib/udev/rules.d" \
  -DSYSTEMD_USER_UNIT_INSTALL_DIR="${PREFIX}/lib/systemd/user" \
  -DSYSTEMD_MODULES_LOAD_DIR="${PREFIX}/lib/modules-load.d" \
  -DSUNSHINE_PUBLISHER_NAME=Eclipse \
  -DSUNSHINE_PUBLISHER_ISSUE_URL=https://github.com

echo "==> Building Sunshine"
cmake --build "${BUILD_SUN}"
# Sunshine also tries to install udev/systemd units; keep those inside PREFIX.
keep_sunshine_install_in_prefix
cmake --install "${BUILD_SUN}"

SUN_BIN="$(find "${PREFIX}" "${BUILD_SUN}" -type f -name sunshine -perm /111 | head -n 1)"
if [[ -z "${SUN_BIN}" ]]; then
  echo "Sunshine binary not found under ${PREFIX} or ${BUILD_SUN}" >&2
  exit 1
fi
install -m 755 "${SUN_BIN}" "${BIN_DIR}/sunshine-${TRIPLE}"

if [[ -d "${PREFIX}/share/sunshine" ]]; then
  rsync -a --delete "${PREFIX}/share/sunshine/" "${ASSET_DIR}/"
fi
if [[ -f "${ROOT}/src-tauri/icons/sunshine.svg" ]]; then
  for dest in "${ASSET_DIR}" "${PREFIX}/share/sunshine"; do
    if [[ -d "${dest}/web/images" ]]; then
      cp "${ROOT}/src-tauri/icons/sunshine.svg" "${dest}/web/images/logo-sunshine.svg"
      if [[ -f "${ROOT}/src-tauri/icons/sunshine.ico" ]]; then
        cp "${ROOT}/src-tauri/icons/sunshine.ico" "${dest}/web/images/sunshine.ico"
      fi
    fi
  done
fi
echo "Installed ${BIN_DIR}/sunshine-${TRIPLE}"

echo "==> Building Moonlight"
mkdir -p "${MOONLIGHT_SRC}/app/res/eclipse"
if [[ ! -f "${ROOT}/assets/fonts/Remboy-Regular.ttf" || ! -f "${ROOT}/assets/fonts/JetBrainsMono-Regular.ttf" ]]; then
  echo "Missing Eclipse fonts in assets/fonts (Remboy-Regular.ttf, JetBrainsMono-Regular.ttf)" >&2
  exit 1
fi
cp "${ROOT}/assets/fonts/Remboy-Regular.ttf" "${ROOT}/assets/fonts/JetBrainsMono-Regular.ttf" \
  "${MOONLIGHT_SRC}/app/res/eclipse/"
if [[ -f "${ROOT}/src-tauri/icons/moonlight.ico" ]]; then
  cp "${ROOT}/src-tauri/icons/moonlight.ico" "${MOONLIGHT_SRC}/app/moonlight.ico"
fi
pushd "${BUILD_MOON}" >/dev/null
qmake6 "${MOONLIGHT_SRC}/moonlight-qt.pro" CONFIG+=release
make -j"$(nproc)"
popd >/dev/null

MOON_BIN="$(find "${BUILD_MOON}" "${MOONLIGHT_SRC}" -type f -name moonlight -perm /111 | head -n 1)"
if [[ -z "${MOON_BIN}" ]]; then
  echo "Moonlight binary not found. Looked in ${BUILD_MOON} and ${MOONLIGHT_SRC}" >&2
  exit 1
fi
install -m 755 "${MOON_BIN}" "${BIN_DIR}/moonlight-${TRIPLE}"
echo "Installed ${BIN_DIR}/moonlight-${TRIPLE}"

echo
echo "Sidecars:"
ls -lh "${BIN_DIR}/sunshine-${TRIPLE}" "${BIN_DIR}/moonlight-${TRIPLE}"
"${BIN_DIR}/sunshine-${TRIPLE}" --version || true
echo "Done."
