#!/usr/bin/env bash
# Build native Eclipse packages on this machine. Detects Debian, Fedora, or Arch.
# Does not publish or run GitHub Actions. Sidecars are compiled first (CUDA kernels
# if nvcc is on PATH). End users of the package do not need the CUDA toolkit.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi
export PATH="${HOME}/.local/bin:/usr/local/cuda/bin:/opt/cuda/bin:/usr/lib/cuda/bin:${PATH}"

usage() {
  cat <<'EOF'
Usage: ./scripts/package.sh [--skip-sidecars]

  Arch:     sidecars + tauri --no-bundle + makepkg → packaging/arch/*.pkg.tar.zst
  Debian:   sidecars + tauri --bundles deb
  Fedora:   sidecars + tauri --bundles rpm

Packages must be built on the distro they target (Qt/FFmpeg sonames).
EOF
}

SKIP_SIDECARS=0
for arg in "$@"; do
  case "${arg}" in
    -h|--help)
      usage
      exit 0
      ;;
    --skip-sidecars) SKIP_SIDECARS=1 ;;
    *)
      echo "Unknown argument: ${arg}" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ ! -f /etc/os-release ]]; then
  echo "Cannot detect distro (missing /etc/os-release). On Windows use scripts/package.ps1." >&2
  exit 1
fi
# shellcheck disable=SC1091
source /etc/os-release

family=""
id_like="${ID_LIKE:-}"
case "${ID}" in
  arch|omarchy|manjaro|endeavouros) family=arch ;;
  debian|ubuntu|linuxmint|pop) family=debian ;;
  fedora|rhel|centos|rocky|almalinux) family=fedora ;;
  *)
    case "${id_like}" in
      *arch*) family=arch ;;
      *debian*) family=debian ;;
      *fedora*|*rhel*) family=fedora ;;
    esac
    ;;
esac
if [[ -z "${family}" ]]; then
  echo "Unsupported distro: ${ID}. Build on Debian/Ubuntu 24.04, Fedora, or Arch." >&2
  exit 1
fi

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing $1. Run ./scripts/bootstrap.sh first." >&2
    exit 1
  fi
}

need rustc
need cargo
need npm
need cmake

if [[ "${SKIP_SIDECARS}" -eq 0 ]]; then
  echo "==> Building sidecars"
  "${ROOT}/scripts/build-sidecars.sh"
else
  echo "==> Skipping sidecar rebuild"
fi

TRIPLE="$(rustc --print host-tuple 2>/dev/null || true)"
if [[ -z "${TRIPLE}" ]]; then
  TRIPLE="$(rustc -vV | awk '/^host:/{print $2}')"
fi
if [[ ! -x "${ROOT}/src-tauri/binaries/sunshine-${TRIPLE}" ]]; then
  echo "Sidecar binaries missing for ${TRIPLE}. Run without --skip-sidecars." >&2
  exit 1
fi

if [[ ! -d "${ROOT}/node_modules" ]]; then
  npm install
fi

# Cursor (and some CI) set CARGO_TARGET_DIR to a temp cache. Force the
# documented output so packages land in src-tauri/target/release/bundle.
export CARGO_TARGET_DIR="${ROOT}/src-tauri/target"
echo "CARGO_TARGET_DIR=${CARGO_TARGET_DIR}"

# PKGBUILD installs from src-tauri/target/release.
stage_release_binaries() {
  local dest="${ROOT}/src-tauri/target/release"
  mkdir -p "${dest}"
  local b
  for b in eclipse sunshine moonlight; do
    if [[ -x "${dest}/${b}" ]]; then
      continue
    fi
    echo "Expected ${dest}/${b} after tauri build." >&2
    return 1
  done
}

echo "==> Packaging for ${family} (${ID})"
case "${family}" in
  debian)
    npm run tauri -- build --bundles deb
    echo
    echo "Debian package:"
    ls -lh src-tauri/target/release/bundle/deb/
    echo "Install with: sudo apt install ./src-tauri/target/release/bundle/deb/*.deb"
    ;;
  fedora)
    npm run tauri -- build --bundles rpm
    echo
    echo "Fedora package:"
    ls -lh src-tauri/target/release/bundle/rpm/
    echo "Install with: sudo dnf install ./src-tauri/target/release/bundle/rpm/*.rpm"
    ;;
  arch)
    need makepkg
    npm run tauri -- build --no-bundle
    stage_release_binaries
    if [[ ! -x src-tauri/target/release/sunshine ]]; then
      # tauri --no-bundle still copies externalBin next to the app binary.
      echo "Expected src-tauri/target/release/sunshine after tauri --no-bundle." >&2
      exit 1
    fi
    (cd "${ROOT}/packaging/arch" && makepkg -f --noconfirm)
    echo
    echo "Arch package:"
    ls -lh "${ROOT}/packaging/arch/"*.pkg.tar.* || ls -lh "${ROOT}/packaging/arch/"
    echo "Install with: sudo pacman -U packaging/arch/eclipse-*.pkg.tar.zst"
    echo "Log out once after install so the input group applies."
    ;;
esac
