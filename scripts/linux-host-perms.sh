#!/usr/bin/env bash
# Linux host privileges for Eclipse Sunshine: file caps (KMS), udev, input group.
# Tauri copies externalBin to target/debug/sunshine and strips capabilities, so
# this script setcaps every Sunshine binary it finds. Re-run after sidecar
# rebuilds and after the first `npm run tauri -- dev` copy.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RULES_SRC="${ROOT}/vendor/sunshine/src_assets/linux/misc/60-sunshine.rules"
RULES_DST="/etc/udev/rules.d/60-eclipse-sunshine.rules"
CAPS="cap_sys_admin,cap_sys_nice+p"

STATUS=0
RESET_CAPTURE=0

usage() {
  cat <<'EOF'
Usage: ./scripts/linux-host-perms.sh [--status] [--reset-capture]

  (default)  sudo: setcap on Sunshine binaries, install udev rules, add user
             to the input group.
  --status   Print getcap, input group, and sunshine.conf capture line. No sudo.
  --reset-capture
             Remove capture = kms from the user's sunshine.conf so auto/wlr
             is used. Safe with or without sudo.

Re-run after ./scripts/build-sidecars.sh and after the first tauri dev copy.
A new desktop session is required before the input group applies.
EOF
}

for arg in "$@"; do
  case "${arg}" in
    --status) STATUS=1 ;;
    --reset-capture) RESET_CAPTURE=1 ;;
    -h|--help) usage; exit 0 ;;
    *)
      echo "Unknown argument: ${arg}" >&2
      usage >&2
      exit 1
      ;;
  esac
done

real_user() {
  if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    echo "${SUDO_USER}"
  else
    echo "${USER}"
  fi
}

real_home() {
  local u
  u="$(real_user)"
  getent passwd "${u}" | cut -d: -f6
}

sunshine_conf() {
  local home
  home="$(real_home)"
  local xdg="${home}/.config"
  if [[ -n "${SUDO_USER:-}" ]]; then
    :
  elif [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
    xdg="${XDG_CONFIG_HOME}"
  fi
  echo "${xdg}/sunshine/sunshine.conf"
}

list_sunshine_bins() {
  shopt -s nullglob
  local f
  for f in \
    "${ROOT}/src-tauri/binaries/sunshine-"* \
    "${ROOT}/src-tauri/target/debug/sunshine" \
    "${ROOT}/src-tauri/target/release/sunshine" \
    /usr/bin/sunshine
  do
    [[ -f "${f}" && -x "${f}" ]] || continue
    [[ "${f}" == *.exe ]] && continue
    printf '%s\n' "${f}"
  done
}

print_status() {
  echo "== Eclipse Linux host permissions"
  echo "User: $(real_user)"
  local groups
  groups="$(id -nG "$(real_user)" 2>/dev/null || true)"
  if [[ " ${groups} " == *" input "* ]]; then
    echo "input group: yes"
  else
    echo "input group: no (log out after this script adds it)"
  fi
  local conf
  conf="$(sunshine_conf)"
  echo "sunshine.conf: ${conf}"
  if [[ -f "${conf}" ]]; then
    grep -E '^[[:space:]]*capture[[:space:]]*=' "${conf}" || echo "capture: (unset, auto)"
  else
    echo "capture: (no conf file, auto)"
  fi
  echo "udev: ${RULES_DST}"
  if [[ -f "${RULES_DST}" ]]; then
    echo "udev rules: installed"
  else
    echo "udev rules: missing"
  fi
  local found=0
  local bin
  while IFS= read -r bin; do
    found=1
    if command -v getcap >/dev/null 2>&1; then
      local caps
      caps="$(getcap "${bin}" 2>/dev/null || true)"
      echo "getcap ${bin}: ${caps:-(none)}"
    else
      echo "getcap missing; ${bin}"
    fi
  done < <(list_sunshine_bins)
  if [[ "${found}" -eq 0 ]]; then
    echo "No Sunshine sidecar binaries found. Run ./scripts/build-sidecars.sh"
  fi
}

reset_capture() {
  local conf
  conf="$(sunshine_conf)"
  if [[ ! -f "${conf}" ]]; then
    echo "No ${conf}; capture is already auto."
    return 0
  fi
  if ! grep -qE '^[[:space:]]*capture[[:space:]]*=' "${conf}"; then
    echo "${conf} has no capture= line (auto)."
    return 0
  fi
  local tmp
  tmp="$(mktemp)"
  grep -vE '^[[:space:]]*capture[[:space:]]*=' "${conf}" > "${tmp}" || true
  if [[ ! -s "${tmp}" ]]; then
    printf '%s\n' "# capture unset so Sunshine uses auto (wlr/portal/kms)." > "${tmp}"
  fi
  if [[ -n "${SUDO_USER:-}" ]]; then
    chown "${SUDO_USER}:${SUDO_USER}" "${tmp}" || true
  fi
  mv "${tmp}" "${conf}"
  echo "Removed capture= from ${conf}."
  if pgrep -x sunshine >/dev/null 2>&1 || pgrep -f '[t]arget/.*/sunshine$' >/dev/null 2>&1; then
    echo "Sunshine is still running with the old setting. Quit Eclipse completely, then start it again."
  else
    echo "Restart Eclipse so Sunshine uses auto/wlr."
  fi
}

if [[ "${STATUS}" -eq 1 ]]; then
  print_status
  exit 0
fi

if [[ "${RESET_CAPTURE}" -eq 1 && "${EUID}" -ne 0 ]]; then
  reset_capture
  if [[ "${#}" -eq 1 ]]; then
    exit 0
  fi
fi

if [[ "${EUID}" -ne 0 ]]; then
  echo "Re-running with sudo for setcap, udev, and input group..."
  exec sudo --preserve-env=XDG_CONFIG_HOME -- "$0" "$@"
fi

if [[ "${RESET_CAPTURE}" -eq 1 ]]; then
  reset_capture
fi

if ! command -v setcap >/dev/null 2>&1; then
  echo "setcap not found. Install libcap (Arch: libcap)." >&2
  exit 1
fi

found=0
while IFS= read -r bin; do
  found=1
  setcap "${CAPS}" "${bin}"
  echo "setcap ${CAPS} ${bin}"
  getcap "${bin}" || true
done < <(list_sunshine_bins)
if [[ "${found}" -eq 0 ]]; then
  echo "No Sunshine binaries to setcap. Run ./scripts/build-sidecars.sh first." >&2
  exit 1
fi

if [[ -f "${RULES_SRC}" ]]; then
  install -m 644 "${RULES_SRC}" "${RULES_DST}"
  echo "Installed ${RULES_DST}"
  if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules
    udevadm trigger --property-match=DEVNAME=/dev/uinput || true
    udevadm trigger --property-match=DEVNAME=/dev/uhid || true
    echo "udev rules reloaded."
  fi
else
  echo "Missing ${RULES_SRC}; skip udev (init submodules)." >&2
fi

target="$(real_user)"
groupadd -f input
usermod -aG input "${target}"
echo "User ${target} is in group input (new desktop session required)."
echo

HOST_SETUP="${ROOT}/packaging/linux/host-setup.sh"
if [[ -f "${HOST_SETUP}" ]]; then
  # shellcheck disable=SC1091
  . "${HOST_SETUP}"
  eclipse_host_open_firewall
  user_conf="$(sunshine_conf)"
  user_dir="$(dirname "${user_conf}")"
  mkdir -p "${user_dir}"
  eclipse_host_merge_conf "${user_conf}" "${ROOT}/packaging/sunshine.conf"
  if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
    chown -R "${SUDO_USER}:${SUDO_USER}" "${user_dir}" 2>/dev/null || true
  fi
  echo "Merged host encryption defaults into ${user_conf}."
  echo "GameStream inbound (if ufw/firewalld is active) is limited to 100.64.0.0/10."
fi

desktop_src="${ROOT}/packaging/linux/com.eclipse.moonlight.desktop"
icon_src="${ROOT}/src-tauri/icons/moonlight.svg"
if [[ -f "${desktop_src}" && -f "${icon_src}" ]]; then
  user_home="$(getent passwd "${target}" | cut -d: -f6)"
  if [[ -n "${user_home}" ]]; then
    data_home="${user_home}/.local/share"
    install -Dm644 "${desktop_src}" \
      "${data_home}/applications/com.eclipse.moonlight.desktop"
    install -Dm644 "${icon_src}" \
      "${data_home}/icons/hicolor/scalable/apps/com.eclipse.moonlight.svg"
    if [[ -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
      chown "${SUDO_USER}:${SUDO_USER}" \
        "${data_home}/applications/com.eclipse.moonlight.desktop" \
        "${data_home}/icons/hicolor/scalable/apps/com.eclipse.moonlight.svg" \
        2>/dev/null || true
    else
      chown "${target}:${target}" \
        "${data_home}/applications/com.eclipse.moonlight.desktop" \
        "${data_home}/icons/hicolor/scalable/apps/com.eclipse.moonlight.svg" \
        2>/dev/null || true
    fi
    echo "Installed Moonlight stream window icon for ${target} (com.eclipse.moonlight)."
  fi
fi

echo "KMS capture still needs these caps. Prefer auto/wlr after rebuilding sidecars"
echo "with the EGL fallback; use --reset-capture if sunshine.conf forces kms."
echo "Re-run this script after a sidecar rebuild or a fresh tauri copy of sunshine."
echo
print_status
