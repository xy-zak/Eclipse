#!/usr/bin/env bash
# Emit Eclipse sidecar patches from a dirty vendor tree at the pinned SHA.
# Fonts (*.ttf) are never written into patches; build-sidecars copies them.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUNSHINE_SRC="${ROOT}/vendor/sunshine"
MOONLIGHT_SRC="${ROOT}/vendor/moonlight-qt"
PATCH_DIR="${ROOT}/patches"

MOONLIGHT_BEHAVIOR=(
  app/backend/nvhttp.cpp
  app/backend/nvhttp.h
  app/backend/computermanager.cpp
  app/backend/computerseeker.cpp
  app/cli/commandlineparser.cpp
  app/cli/pair.cpp
  app/cli/startstream.cpp
  app/cli/startstream.h
  app/main.cpp
  app/settings/streamingpreferences.h
  app/streaming/input/keyboard.cpp
  app/streaming/session.cpp
  app/streaming/streamutils.cpp
)

MOONLIGHT_UI=(
  app/app.pro
  app/gui/CliQuitStreamSegue.qml
  app/gui/CliStartStreamSegue.qml
  app/gui/EclipseSplash.qml
  app/gui/EclipseToast.qml
  app/gui/NavigableDialog.qml
  app/gui/NavigableMessageDialog.qml
  app/gui/QuitSegue.qml
  app/gui/StreamSegue.qml
  app/gui/main.qml
  app/qml.qrc
  app/resources.qrc
  app/res/eclipse-logo.svg
  app/res/eclipse/spinner.svg
  app/res/eclipse/check-circle.svg
  app/res/eclipse/moon-solid.svg
  app/res/eclipse/exclamation.svg
)

SUNSHINE_EGL=(src/platform/linux/graphics.cpp)
SUNSHINE_UDEV=(cmake/FindSystemd.cmake cmake/FindUdev.cmake)
SUNSHINE_IDLE=(tools/sunshinesvc.cpp tools/CMakeLists.txt)

need_repo() {
  local repo="$1"
  if [[ ! -e "${repo}/.git" && ! -d "${repo}/.git" ]]; then
    echo "Missing git repo: ${repo}" >&2
    exit 1
  fi
}

intent_add() {
  local repo="$1"
  shift
  local f
  for f in "$@"; do
    local path="${repo}/${f}"
    if [[ -f "${path}" ]]; then
      git -C "${repo}" add -N -- "${f}"
    fi
  done
}

write_patch() {
  local repo="$1"
  local dest="$2"
  local subject="$3"
  local why="$4"
  local depends="$5"
  shift 5
  local tmp
  tmp="$(mktemp)"
  if ! git -C "${repo}" diff --src-prefix=a/ --dst-prefix=b/ -- "$@" >"${tmp}"; then
    rm -f "${tmp}"
    echo "git diff failed for ${dest}" >&2
    exit 1
  fi
  if [[ ! -s "${tmp}" ]]; then
    echo "No changes for ${dest}; leaving existing file" >&2
    rm -f "${tmp}"
    return 0
  fi
  {
    echo "Subject: ${subject}"
    echo "Why: ${why}"
    echo "Depends-on: ${depends}"
    echo
    cat "${tmp}"
  } >"${dest}"
  rm -f "${tmp}"
  echo "Wrote ${dest}"
}

need_repo "${SUNSHINE_SRC}"
need_repo "${MOONLIGHT_SRC}"

echo "==> Moonlight"
intent_add "${MOONLIGHT_SRC}" "${MOONLIGHT_BEHAVIOR[@]}" "${MOONLIGHT_UI[@]}"
write_patch "${MOONLIGHT_SRC}" "${PATCH_DIR}/moonlight-01-behavior.patch" \
  "moonlight-01-behavior" \
  "Host-display CLI, Tailscale QHostAddress, headless pair, ECLIPSE_CONTROL hotkeys, integer-scale fullscreen, splash hold." \
  "vendor/PINS moonlight-qt; apply before moonlight-02-ui.patch" \
  "${MOONLIGHT_BEHAVIOR[@]}"
write_patch "${MOONLIGHT_SRC}" "${PATCH_DIR}/moonlight-02-ui.patch" \
  "moonlight-02-ui" \
  "Eclipse connecting splash, dialogs/toasts, moon icon, Remboy/JetBrains, qrc assets. Fonts and ICO are copied at sidecar build, not in this patch." \
  "vendor/PINS moonlight-qt; moonlight-01-behavior.patch" \
  "${MOONLIGHT_UI[@]}"

echo "==> Sunshine (non-overlapping files only)"
intent_add "${SUNSHINE_SRC}" "${SUNSHINE_EGL[@]}" "${SUNSHINE_UDEV[@]}" "${SUNSHINE_IDLE[@]}"
write_patch "${SUNSHINE_SRC}" "${PATCH_DIR}/sunshine-egl-bad-match.patch" \
  "sunshine-egl-bad-match" \
  "Retry DMA-BUF EGL import without modifier on EGL_BAD_MATCH (00003009)." \
  "vendor/PINS sunshine" \
  "${SUNSHINE_EGL[@]}"
write_patch "${SUNSHINE_SRC}" "${PATCH_DIR}/sunshine-udev-prefix.patch" \
  "sunshine-udev-prefix" \
  "Honor CMake UDEV/systemd install dirs so sidecar prefix is not /usr." \
  "vendor/PINS sunshine" \
  "${SUNSHINE_UDEV[@]}"
write_patch "${SUNSHINE_SRC}" "${PATCH_DIR}/sunshine-idle-awake.patch" \
  "sunshine-idle-awake" \
  "Windows sunshinesvc: PowerRequestSystemRequired while Sunshine.exe runs; restart on resume. Lid-close still sleeps." \
  "vendor/PINS sunshine" \
  "${SUNSHINE_IDLE[@]}"

echo
echo "Not regenerated (overlapping hunks; edit by hand, keep headers):"
echo "  patches/sunshine-displays.patch"
echo "  patches/sunshine-sessions.patch"
echo "  patches/sunshine-cgnat-pair.patch"
echo
echo "TTF fonts are not in patches. Rebuild sidecars after refresh."
