#!/usr/bin/env bash
# GameStream video smoke. Modes: host | client <ip> | loopback
# Pair/stream flags match src-tauri/src/session.rs. See docs/Video.md.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TRANSCRIPT="${TMPDIR:-/tmp}/eclipse-video-smoke.log"
UI_PORT=47990
RTSP_PORT=48010
# 0 = stream until the Moonlight window is closed.
STREAM_SECS="${ECLIPSE_VIDEO_SMOKE_SECS:-10}"
RESOLUTION="${ECLIPSE_SMOKE_RESOLUTION:-1920x1080}"
PAIR_GRACE=0.4
PIN_INTERVAL=0.3
PIN_TRIES=80
POST_PIN_SETTLE=3
HTTPS_TRIES=40
ENCODER_TRIES=60

: > "${TRANSCRIPT}"

host_triple() {
  rustc --print host-tuple 2>/dev/null || rustc -vV | awk '/^host:/{print $2}'
}

TRIPLE="$(host_triple)"
SUN_BIN="${ROOT}/src-tauri/binaries/sunshine-${TRIPLE}"
MOON_BIN="${ROOT}/src-tauri/binaries/moonlight-${TRIPLE}"
ASSETS="${ROOT}/src-tauri/resources/sunshine"
SUN_PID=""
SPAWNED=0

# --- logging ---

log() { printf '%s\n' "$*" | tee -a "${TRANSCRIPT}"; }

die() {
  log "FAIL: $*"
  log "Transcript: ${TRANSCRIPT}"
  exit 1
}

usage() {
  cat <<'EOF'
Usage:
  ./scripts/smoke-video.sh              # host (default)
  ./scripts/smoke-video.sh host
  ./scripts/smoke-video.sh client <ip>
  ./scripts/smoke-video.sh loopback

Env:
  ECLIPSE_VIDEO_SMOKE_SECS=N   stream for N seconds (default 10); 0 = until you
                               close the Moonlight window (Ctrl+Alt+Q)
  ECLIPSE_STREAM_UI=0|1        hide/show the in-stream Eclipse logo menu (default 1)
EOF
}

require_display() {
  if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
    die "No graphical session (WAYLAND_DISPLAY/DISPLAY unset). Run this on the desktop."
  fi
}

require_moonlight() {
  [[ -x "${MOON_BIN}" ]] || die "Missing ${MOON_BIN}. Run ./scripts/build-sidecars.sh"
}

stop_pid() {
  local pid="${1:-}"
  [[ -n "${pid}" ]] || return 0
  kill "${pid}" 2>/dev/null || true
  wait "${pid}" 2>/dev/null || true
}

# --- net ---

tcp_open() {
  (echo >/dev/tcp/"$1"/"$2") >/dev/null 2>&1
}

http_code() {
  local code
  code="$(curl -k -s -o /dev/null -w '%{http_code}' --max-time 3 \
    "$1" 2>/dev/null || true)"
  [[ "${code}" =~ ^[0-9]{3}$ ]] && printf '%s\n' "${code}" || printf '000\n'
}

https_ok() { [[ "$1" =~ ^[1-5][0-9]{2}$ ]]; }

listen_pid() {
  ss -tlnp 2>/dev/null | awk -v p=":$1" '
    $4 ~ p"$" { if (match($0, /pid=([0-9]+)/, a)) { print a[1]; exit } }'
}

# --- sunshine log ---

sunshine_log_path() {
  echo "${XDG_CONFIG_HOME:-${HOME}/.config}/sunshine/sunshine.log"
}

# Lines from the last "Sunshine version:" banner (the newest instance) to EOF.
slice_latest_instance() {
  awk '
    BEGIN { start = 1 }
    /Sunshine version:/ { start = NR }
    { lines[NR] = $0 }
    END { for (i = start; i <= NR; i++) print lines[i] }
  '
}

latest_slice() {
  local file="$1"
  [[ -f "${file}" ]] || return 0
  slice_latest_instance <"${file}"
}

# Attached to the packaged host: it logs to the journal, not ~/.config/sunshine.
using_service_log() {
  [[ "${SPAWNED}" -eq 0 ]] && systemctl is-active --quiet eclipse-host.service 2>/dev/null
}

host_log_name() {
  if using_service_log; then
    echo "journalctl -u eclipse-host.service"
  else
    sunshine_log_path
  fi
}

host_log_slice() {
  if using_service_log; then
    journalctl -u eclipse-host.service -b -o cat --no-pager 2>/dev/null | slice_latest_instance
  else
    latest_slice "$(sunshine_log_path)"
  fi
}

host_video_issue() {
  local text="$1"
  if grep -q "config: 'capture' = kms" <<<"${text}" && grep -q "Failed to gain CAP_SYS_ADMIN" <<<"${text}"; then
    echo "KMS capture lacks CAP_SYS_ADMIN. Run ./scripts/linux-host-perms.sh or --reset-capture."
    return 0
  fi
  if grep -qE "Unable to initialize capture method|Platform failed to initialize|Unable to find display or encoder|Video failed to find working encoder" <<<"${text}"; then
    echo "Capture/encoder did not initialize. See docs/Video.md."
    return 0
  fi
  return 1
}

host_audio_issue() {
  grep -qE "Unable to initialize audio capture|Couldn't connect to pulseaudio|pa_simple_new\(\) failed" <<<"$1"
}

# Packaged hosts log to the journal (eclipse-host.service), not ~/.config/sunshine.
service_log_since() {
  systemctl is-active --quiet eclipse-host.service 2>/dev/null || return 0
  journalctl -u eclipse-host.service --since "@$1" -o cat --no-pager 2>/dev/null || true
}

print_host_summary() {
  log "--- Sunshine log (latest instance) ---"
  grep -E "Found H\.264 encoder|Found HEVC|Found AV1|wlgrab|kmsgrab|portalgrab|Screencasting|CLIENT CONNECTED|Couldn't import RGB|Could not convert image|Failed to gain CAP_SYS_ADMIN|Unable to initialize capture|Creating encoder \[h264" <<<"$1" \
    | tail -n 40 | tee -a "${TRANSCRIPT}" || true
}

# --- host ---

cleanup() {
  if [[ "${SPAWNED}" -eq 1 ]]; then
    stop_pid "${SUN_PID}"
  fi
}
trap cleanup EXIT

ensure_host_up() {
  local url="https://127.0.0.1:${UI_PORT}" code i
  if tcp_open 127.0.0.1 "${UI_PORT}"; then
    code="$(http_code "${url}")"
    https_ok "${code}" || die "Port ${UI_PORT} is open but HTTPS ${url} did not respond (HTTP ${code})."
    log "Attaching to Sunshine already on ${url} (HTTP ${code})"
    return 0
  fi
  [[ -x "${SUN_BIN}" ]] || die "Missing ${SUN_BIN}. Run ./scripts/build-sidecars.sh"
  log "Spawning ${SUN_BIN}"
  [[ -d "${ASSETS}" ]] && export SUNSHINE_ASSETS_DIR="${ASSETS}"
  (
    cd "$(dirname "${SUN_BIN}")"
    "${SUN_BIN}"
  ) >>"${TRANSCRIPT}" 2>&1 &
  SUN_PID=$!
  SPAWNED=1
  for i in $(seq 1 "${HTTPS_TRIES}"); do
    code="$(http_code "${url}")"
    if https_ok "${code}"; then
      log "OK Sunshine HTTPS ${url} -> HTTP ${code}"
      return 0
    fi
    kill -0 "${SUN_PID}" 2>/dev/null || die "Sunshine exited before opening ${url}. See ${TRANSCRIPT}"
    sleep 0.5
  done
  die "Sunshine did not answer ${url} within 20s. See ${TRANSCRIPT}"
}

wait_for_encoder() {
  local slice issue i
  for i in $(seq 1 "${ENCODER_TRIES}"); do
    slice="$(host_log_slice)"
    if [[ -n "${slice}" ]]; then
      if issue="$(host_video_issue "${slice}")"; then
        print_host_summary "${slice}"
        die "${issue}"
      fi
      grep -q "Found H.264 encoder" <<<"${slice}" && return 0
    fi
    if [[ "${SPAWNED}" -eq 1 ]] && ! kill -0 "${SUN_PID}" 2>/dev/null; then
      print_host_summary "${slice:-}"
      die "Sunshine exited before finding an encoder. See ${TRANSCRIPT}"
    fi
    sleep 0.5
  done
  print_host_summary "$(host_log_slice)"
  die "No H.264 encoder line in $(host_log_name) after 30s. See docs/Video.md."
}

check_rtsp_ports() {
  tcp_open 127.0.0.1 "${UI_PORT}" || die "TCP ${UI_PORT} not listening"
  tcp_open 127.0.0.1 "${RTSP_PORT}" || die "TCP ${RTSP_PORT} (RTSP) not listening"
  local ui_pid rtsp_pid
  ui_pid="$(listen_pid "${UI_PORT}")"
  rtsp_pid="$(listen_pid "${RTSP_PORT}")"
  if [[ -n "${ui_pid}" && -n "${rtsp_pid}" && "${ui_pid}" != "${rtsp_pid}" ]]; then
    die "Web UI :${UI_PORT} is pid ${ui_pid} but RTSP :${RTSP_PORT} is pid ${rtsp_pid}. Kill extra Sunshine, restart Eclipse."
  fi
}

check_listening_host_matches_log() {
  local pids n_pids ui_pid proc_epoch log_line ts log_epoch
  # systemd owns the service process; its journal is always that instance.
  using_service_log && return 0
  pids="$(pgrep -x sunshine 2>/dev/null || true)"
  n_pids="$(printf '%s\n' "${pids}" | grep -c '[0-9]' || true)"
  if [[ "${n_pids}" -gt 1 ]]; then
    die "Multiple Sunshine processes (${pids}). Kill leftovers (killall sunshine), restart Eclipse, then retry."
  fi
  ui_pid="$(listen_pid "${UI_PORT}")"
  [[ -n "${ui_pid}" ]] || return 0
  proc_epoch="$(date -d "$(ps -o lstart= -p "${ui_pid}" 2>/dev/null)" +%s 2>/dev/null || true)"
  [[ -n "${proc_epoch}" ]] || return 0
  log_line="$(grep 'Sunshine version:' "$(sunshine_log_path)" 2>/dev/null | tail -n 1 || true)"
  ts="$(grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2}' <<<"${log_line}" | head -n 1 || true)"
  [[ -n "${ts}" ]] || return 0
  log_epoch="$(date -d "${ts}" +%s 2>/dev/null || true)"
  [[ -n "${log_epoch}" ]] || return 0
  if [[ "${proc_epoch}" -lt $((log_epoch - 5)) ]]; then
    die "Sunshine pid ${ui_pid} started before the latest log instance (${ts}). Leftover host owns GameStream ports. Kill ${ui_pid}, restart Eclipse, then retry."
  fi
}

check_host() {
  require_display
  ensure_host_up
  wait_for_encoder
  local slice
  slice="$(host_log_slice)"
  print_host_summary "${slice}"
  check_rtsp_ports
  check_listening_host_matches_log
  log "OK host: encoder present, TCP ${UI_PORT} and ${RTSP_PORT} open (same process)"
}

# --- moonlight ---

moonlight_cmd() {
  if command -v stdbuf >/dev/null 2>&1; then
    stdbuf -oL -eL "${MOON_BIN}" "$@"
  else
    "${MOON_BIN}" "$@"
  fi
}

list_has_desktop() {
  local out
  out="$("${MOON_BIN}" list "$1" 2>&1 || true)"
  printf '%s\n' "${out}" >>"${TRANSCRIPT}"
  grep -qiE '^[[:space:]]*Desktop[[:space:]]*$' <<<"${out}"
}

submit_pin() {
  local code
  code="$(curl -k -s -o /tmp/eclipse-pin-smoke.json -w '%{http_code}' --max-time 5 \
    -H 'Content-Type: application/json' \
    -d "{\"pin\":\"$2\",\"name\":\"Eclipse\"}" \
    "https://$1:${UI_PORT}/api/pin" 2>>"${TRANSCRIPT}" || true)"
  [[ "${code}" =~ ^[0-9]{3}$ ]] && printf '%s\n' "${code}" || printf '000\n'
}

try_pair() {
  local ip="$1" pin="$2" pair_log="$3" pair_pid i code
  : > "${pair_log}"
  log "Pairing ${ip} with PIN ${pin}"
  moonlight_cmd pair "${ip}" --pin "${pin}" >>"${pair_log}" 2>&1 &
  pair_pid=$!
  sleep "${PAIR_GRACE}"
  if ! kill -0 "${pair_pid}" 2>/dev/null; then
    cat "${pair_log}" >>"${TRANSCRIPT}"
    log "Moonlight pair exited before PIN POST"
    return 1
  fi
  for i in $(seq 1 "${PIN_TRIES}"); do
    code="$(submit_pin "${ip}" "${pin}")"
    if [[ "${code}" == "401" ]]; then
      stop_pid "${pair_pid}"
      die "PIN POST 401: host sidecar is missing Eclipse's CGNAT pairing patch. Rebuild sidecars on the host."
    fi
    kill -0 "${pair_pid}" 2>/dev/null || break
    sleep "${PIN_INTERVAL}"
  done
  sleep "${POST_PIN_SETTLE}"
  stop_pid "${pair_pid}"
  cat "${pair_log}" >>"${TRANSCRIPT}"
  if list_has_desktop "${ip}"; then
    return 0
  fi
  return 1
}

pair_if_needed() {
  local ip="$1" pin pair_log attempt
  require_moonlight
  if list_has_desktop "${ip}"; then
    log "Already paired with ${ip} (Desktop listed)"
    return 0
  fi
  pair_log="${TMPDIR:-/tmp}/eclipse-moonlight-pair.log"
  for attempt in 1 2; do
    pin="$(printf '%04d' $((RANDOM % 10000)))"
    if try_pair "${ip}" "${pin}" "${pair_log}"; then
      log "Paired: Desktop listed"
      return 0
    fi
    log "Pair attempt ${attempt} did not list Desktop"
  done
  die "Moonlight never listed Desktop after pairing. See ${TRANSCRIPT}"
}

fail_if_moon_log() {
  local moon_log="$1"
  if grep -qiE 'has not been paired|Incorrect PIN' "${moon_log}"; then
    cat "${moon_log}" >>"${TRANSCRIPT}"
    die "Moonlight is not paired. Pairing must list Desktop before streaming."
  fi
  if grep -qiE 'No video received|Failed to initialise video capture|error 503' "${moon_log}"; then
    cat "${moon_log}" >>"${TRANSCRIPT}"
    die "Moonlight reported no video. See ${moon_log} and ${TRANSCRIPT}"
  fi
}

run_client() {
  local ip="$1" slog mark moon_log moon_pid t new="" started
  require_display
  require_moonlight
  [[ -n "${ip}" ]] || die "client mode needs an IP"
  pair_if_needed "${ip}"
  slog="$(sunshine_log_path)"
  mark="$(wc -l < "${slog}" 2>/dev/null || echo 0)"
  moon_log="${TMPDIR:-/tmp}/eclipse-moonlight-smoke.log"
  : > "${moon_log}"
  started="$(date +%s)"
  if [[ "${STREAM_SECS}" -eq 0 ]]; then
    log "Streaming to ${ip} until you close the Moonlight window (windowed ${RESOLUTION} H.264)"
  else
    log "Streaming to ${ip} for ${STREAM_SECS}s (windowed ${RESOLUTION} H.264)"
  fi
  # Show the Eclipse logo menu like Eclipse does (ECLIPSE_STREAM_UI=0 to hide).
  ECLIPSE_STREAM_UI="${ECLIPSE_STREAM_UI:-1}" moonlight_cmd stream \
    --resolution "${RESOLUTION}" --video-codec H.264 --no-hdr --packet-size 1024 \
    --display-mode windowed \
    "${ip}" Desktop >>"${moon_log}" 2>&1 &
  moon_pid=$!
  t=0
  while [[ "${STREAM_SECS}" -eq 0 || "${t}" -lt "${STREAM_SECS}" ]]; do
    sleep 1
    t=$((t + 1))
    # Window closed (Ctrl+Alt+Q / Esc chord): stop waiting.
    kill -0 "${moon_pid}" 2>/dev/null || break
    if grep -qiE 'has not been paired|Incorrect PIN|No video received|Failed to initialise video capture|error 503' "${moon_log}" 2>/dev/null; then
      stop_pid "${moon_pid}"
      fail_if_moon_log "${moon_log}"
    fi
  done
  stop_pid "${moon_pid}"
  cat "${moon_log}" >>"${TRANSCRIPT}"
  fail_if_moon_log "${moon_log}"
  if [[ -f "${slog}" ]]; then
    new="$(tail -n +"$((mark + 1))" "${slog}" 2>/dev/null || true)"
    grep -q "Couldn't import RGB Image" <<<"${new}" && \
      die "Sunshine could not import captured frames (EGL 00003009). Rebuild sidecars so the EGL fallback patch is applied."
    grep -q "Could not convert image" <<<"${new}" && \
      die "Sunshine could not convert captured frames. See docs/Video.md."
    grep -q "CLIENT CONNECTED" <<<"${new}" && log "Sunshine: CLIENT CONNECTED"
  fi
  case "${ip}" in
    127.0.0.1|localhost|::1)
      new+=$'\n'"$(service_log_since "${started}")"
      grep -q "CLIENT CONNECTED" <<<"${new}" || \
        die "Loopback stream ran ${t}s but Sunshine never logged CLIENT CONNECTED."
      if host_audio_issue "${new}"; then
        grep -E "Unable to initialize audio capture|Couldn't connect to pulseaudio|pa_simple_new|Eclipse: " <<<"${new}" \
          | tail -n 10 | tee -a "${TRANSCRIPT}" || true
        die "Host streamed without audio. Packaged Linux hosts need patches/sunshine-session-audio.patch and a desktop user logged in on seat0."
      fi
      log "OK host audio: capture initialized"
      ;;
  esac
  log "OK client: Moonlight ran ${t}s without a no-video error"
}

# --- main ---

MODE="${1:-host}"
case "${MODE}" in
  -h|--help) usage; exit 0 ;;
  host) check_host ;;
  client) run_client "${2:-}" ;;
  loopback)
    check_host
    run_client "127.0.0.1"
    ;;
  *) usage >&2; exit 1 ;;
esac

log "Transcript: ${TRANSCRIPT}"
