#!/usr/bin/env bash
# Install Eclipse / Tauri / Sunshine / Moonlight build dependencies.
set -euo pipefail

need() {
  command -v "$1" >/dev/null 2>&1
}

export_cuda_path() {
  local d
  for d in /usr/local/cuda/bin /opt/cuda/bin /usr/lib/cuda/bin; do
    if [[ -d "$d" ]]; then
      case ":${PATH}:" in
        *":${d}:"*) ;;
        *) export PATH="${d}:${PATH}" ;;
      esac
    fi
  done
}

export_local_path() {
  local d="${HOME}/.local/bin"
  if [[ -d "$d" ]]; then
    case ":${PATH}:" in
      *":${d}:"*) ;;
      *) export PATH="${d}:${PATH}" ;;
    esac
  fi
}

has_nvidia_gpu() {
  if grep -q 0x10de /sys/bus/pci/devices/*/vendor 2>/dev/null; then
    return 0
  fi
  need nvidia-smi
}

want_cuda() {
  local mode="${ECLIPSE_CUDA:-auto}"
  case "${mode}" in
    off|OFF) return 1 ;;
    on|ON) return 0 ;;
    *) has_nvidia_gpu ;;
  esac
}

install_rustup() {
  if need rustup; then
    rustup default stable
    return 0
  fi
  if need rustc; then
    return 0
  fi
  echo "Installing rustup (stable)..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
  rustup default stable
}

node_major() {
  if ! need node; then
    echo 0
    return
  fi
  node -p "parseInt(process.versions.node, 10)" 2>/dev/null || echo 0
}

install_node22() {
  local ver="v22.19.0"
  local arch
  case "$(uname -m)" in
    x86_64) arch=x64 ;;
    aarch64|arm64) arch=arm64 ;;
    *)
      echo "Cannot install Node ${ver}: unsupported arch $(uname -m)." >&2
      exit 1
      ;;
  esac
  local url="https://nodejs.org/dist/${ver}/node-${ver}-linux-${arch}.tar.xz"
  echo "Installing Node ${ver} into ${HOME}/.local ..."
  mkdir -p "${HOME}/.local"
  curl -fsSL "${url}" | tar -xJ -C "${HOME}/.local" --strip-components=1
  export_local_path
}

ensure_node() {
  export_local_path
  local maj
  maj="$(node_major)"
  if [[ "${maj}" -ge 20 ]]; then
    return 0
  fi
  if [[ "${maj}" -gt 0 ]]; then
    echo "Node $(node --version) is older than 20; installing Node 22 into ~/.local."
  else
    echo "Node.js not found; installing Node 22 into ~/.local."
  fi
  install_node22
}

install_cuda_arch() {
  local -a extra=(cuda)
  if ! need nvidia-smi; then
    extra+=(nvidia-utils)
  fi
  if need omarchy; then
    omarchy pkg add "${extra[@]}"
  else
    sudo pacman -S --needed --noconfirm "${extra[@]}"
  fi
}

install_cuda_debian() {
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y nvidia-cuda-toolkit
}

install_cuda_fedora() {
  if sudo dnf install -y cuda-nvcc; then
    return 0
  fi
  if sudo dnf install -y nvidia-cuda-toolkit; then
    return 0
  fi
  echo "CUDA: no cuda-nvcc or nvidia-cuda-toolkit package in Fedora repos; skipping."
}

maybe_install_cuda() {
  export_cuda_path
  if ! want_cuda; then
    echo "CUDA: skipped (no NVIDIA GPU, or ECLIPSE_CUDA=off)."
    return 0
  fi
  if need nvcc; then
    echo "CUDA: nvcc already present."
    return 0
  fi
  if has_nvidia_gpu && ! need nvidia-smi; then
    echo "NVIDIA GPU detected but nvidia-smi is missing. Install the vendor driver for runtime NVENC; installing the CUDA toolkit for compile."
  fi
  echo "Installing CUDA toolkit (nvcc). This can take a while and several GB..."
  case "${ECLIPSE_CUDA_DISTRO}" in
    arch) install_cuda_arch ;;
    debian) install_cuda_debian ;;
    fedora) install_cuda_fedora ;;
  esac
  export_cuda_path
  if need nvcc; then
    echo "CUDA: $(nvcc --version | tail -1)"
  else
    echo "CUDA: nvcc still not on PATH after install. Sidecar build will use ECLIPSE_CUDA=auto (off) unless you add the toolkit bin directory to PATH."
  fi
}

pkg_arch() {
  local -a pkgs=(
    base-devel git curl wget file cmake ninja pkgconf nodejs npm rustup
    webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg openssl appmenu-gtk-module
    avahi libcap libdrm libevdev libmfx libnotify libpipewire libpulse libva
    libx11 libxcb libxfixes libxrandr libxtst miniupnpc numactl opus
    qt6-base qt6-svg qt6-declarative qt6-wayland vulkan-icd-loader wayland
    appstream appstream-glib desktop-file-utils python-jinja shaderc
    intel-media-driver libva-utils vulkan-intel
    mesa vulkan-radeon
    ffmpeg sdl2_ttf sdl2-compat libvdpau libplacebo vulkan-headers
  )
  if need omarchy; then
    omarchy pkg add "${pkgs[@]}"
  else
    sudo pacman -S --needed --noconfirm "${pkgs[@]}"
  fi
}

pkg_debian() {
  sudo apt-get update
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential git curl wget file cmake ninja-build pkg-config \
    nodejs npm \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev libssl-dev \
    libavahi-client-dev libcap-dev libdrm-dev libevdev-dev libnotify-dev \
    libpipewire-0.3-dev libpulse-dev libva-dev libvdpau-dev \
    libx11-dev libxcb1-dev libxfixes-dev libxrandr-dev libxtst-dev \
    libminiupnpc-dev libnuma-dev libopus-dev \
    qt6-base-dev qt6-declarative-dev libqt6svg6-dev qt6-wayland \
    qml6-module-qtquick-controls qml6-module-qtquick-templates \
    qml6-module-qtquick-layouts qml6-module-qtqml-workerscript \
    qml6-module-qtquick-window qml6-module-qtquick \
    libegl1-mesa-dev libgl1-mesa-dev libsdl2-dev libsdl2-ttf-dev \
    libavcodec-dev libavformat-dev libswscale-dev libxkbcommon-dev \
    wayland-protocols libplacebo-dev libvulkan-dev \
    appstream-util desktop-file-utils python3-jinja2 glslang-tools \
    vainfo intel-media-va-driver-non-free mesa-va-drivers
}

pkg_fedora() {
  sudo dnf install -y \
    gcc gcc-c++ git curl wget file cmake ninja-build pkgconf-pkg-config \
    nodejs npm rustup \
    webkit2gtk4.1-devel openssl-devel gtk3-devel libappindicator-gtk3-devel \
    librsvg2-devel \
    avahi-devel libcap-devel libdrm-devel libevdev-devel libnotify-devel \
    pipewire-devel pulseaudio-libs-devel libva-devel libvdpau-devel \
    libX11-devel libxcb-devel libXfixes-devel libXrandr-devel libXtst-devel \
    miniupnpc-devel numactl-devel opus-devel \
    qt6-qtbase-devel qt6-qtdeclarative-devel qt6-qtsvg-devel qt6-qtwayland \
    SDL2-devel SDL2_ttf-devel ffmpeg-devel libplacebo-devel vulkan-headers \
    libva-utils mesa-dri-drivers intel-media-driver
}

echo "Eclipse bootstrap"
echo "First run may take a while (packages; CUDA toolkit on NVIDIA machines is several GB)."
echo "Set ECLIPSE_CUDA=off to skip CUDA. This script does not install the NVIDIA kernel driver."

if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  source /etc/os-release
else
  echo "Cannot detect distro (missing /etc/os-release)." >&2
  exit 1
fi

ECLIPSE_CUDA_DISTRO=""
id_like="${ID_LIKE:-}"
case "${ID}" in
  arch|omarchy|manjaro|endeavouros)
    pkg_arch
    ECLIPSE_CUDA_DISTRO=arch
    ;;
  debian|ubuntu|linuxmint|pop)
    pkg_debian
    ECLIPSE_CUDA_DISTRO=debian
    ;;
  fedora|rhel|centos|rocky|almalinux)
    pkg_fedora
    ECLIPSE_CUDA_DISTRO=fedora
    ;;
  *)
    case "${id_like}" in
      *arch*)
        pkg_arch
        ECLIPSE_CUDA_DISTRO=arch
        ;;
      *debian*)
        pkg_debian
        ECLIPSE_CUDA_DISTRO=debian
        ;;
      *fedora*|*rhel*)
        pkg_fedora
        ECLIPSE_CUDA_DISTRO=fedora
        ;;
      *)
        echo "Unsupported distro: ${ID}. Install Tauri, Qt6, CMake, Ninja, and VA-API deps manually." >&2
        exit 1
        ;;
    esac
    ;;
esac

# rustup / cargo env for this process (new installs land in ~/.cargo)
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi

install_rustup
ensure_node
maybe_install_cuda

if ! need rustc; then
  echo "rustc not found after bootstrap." >&2
  exit 1
fi

echo
echo "Toolchain:"
need rustc && rustc --version
need cargo && cargo --version
need node && node --version
need npm && npm --version
need cmake && cmake --version | head -1
need ninja && ninja --version
need qmake6 && qmake6 -v | head -2
if need nvcc; then
  echo "CUDA: $(nvcc --version | tail -1) (Sunshine CUDA will be ON unless ECLIPSE_CUDA=off)"
else
  echo "CUDA: nvcc not found (Sunshine CUDA will be OFF unless ECLIPSE_CUDA=on)"
fi
if need vainfo; then
  vainfo 2>/dev/null | head -20 || true
fi
echo "Bootstrap finished."
