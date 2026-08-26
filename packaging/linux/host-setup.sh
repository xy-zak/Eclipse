#!/bin/sh
# Shared host setup for deb/rpm postinst and Arch eclipse.install.
# Installed at /usr/lib/eclipse/host-setup.sh. Idempotent. Do not use bash-only syntax.
set -e

CAPS="cap_sys_admin,cap_sys_nice+p"
CGNAT="100.64.0.0/10"
CONF_TEMPLATE="/usr/lib/eclipse/sunshine.conf"
SUNSHINE_DIR="/etc/eclipse/sunshine"

eclipse_host_setcap() {
  bin="$1"
  if [ -x "$bin" ] && command -v setcap >/dev/null 2>&1; then
    setcap "$CAPS" "$bin" || true
  fi
}

eclipse_host_udev() {
  if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --property-match=DEVNAME=/dev/uinput || true
    udevadm trigger --property-match=DEVNAME=/dev/uhid || true
  fi
  modprobe uinput 2>/dev/null || true
  modprobe uhid 2>/dev/null || true
}

eclipse_host_add_input_user() {
  u="$1"
  if [ -z "$u" ] || [ "$u" = "root" ]; then
    return 0
  fi
  if command -v usermod >/dev/null 2>&1; then
    usermod -aG input "$u" 2>/dev/null || true
  fi
}

eclipse_json_str() {
  # $1 file $2 key — Sunshine sunshine_state.json string fields.
  sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$1" | head -n 1
}

eclipse_sunshine_pw_hex() {
  password="$1"
  salt="$2"
  hex=$(printf '%s' "${password}${salt}" | openssl dgst -sha256 2>/dev/null | awk '{print toupper($NF)}')
  [ -n "$hex" ] || return 1
  awk -v h="$hex" 'BEGIN {
    out = ""
    for (i = length(h); i > 0; i -= 2) {
      out = out substr(h, i - 1, 2)
    }
    print out
  }'
}

eclipse_creds_present() {
  f="$1"
  [ -f "$f" ] || return 1
  u=$(eclipse_json_str "$f" username)
  p=$(eclipse_json_str "$f" password)
  s=$(eclipse_json_str "$f" salt)
  [ -n "$u" ] && [ -n "$p" ] && [ -n "$s" ]
}

eclipse_creds_are_legacy() {
  f="$1"
  [ -f "$f" ] || return 1
  u=$(eclipse_json_str "$f" username)
  p=$(eclipse_json_str "$f" password)
  s=$(eclipse_json_str "$f" salt)
  [ -n "$u" ] && [ -n "$p" ] && [ -n "$s" ] || return 1
  [ "$u" = "eclipse" ] || return 1
  command -v openssl >/dev/null 2>&1 || return 1
  expect=$(eclipse_sunshine_pw_hex eclipse "$s") || return 1
  [ "$(printf '%s' "$p" | tr '[:lower:]' '[:upper:]')" = "$expect" ]
}

eclipse_random_pass() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 18 2>/dev/null | tr -d '/+=\n' | cut -c1-20
    return 0
  fi
  dd if=/dev/urandom bs=20 count=1 2>/dev/null | od -An -tx1 | tr -d ' \n'
}

eclipse_upsert_conf_key() {
  conf="$1"
  key="$2"
  val="$3"
  tmp=$(mktemp)
  found=0
  if [ -f "$conf" ]; then
    while IFS= read -r line || [ -n "$line" ]; do
      case "$line" in
        ''|\#*)
          printf '%s\n' "$line"
          ;;
        *)
          k=$(printf '%s' "$line" | sed -n 's/^[[:space:]]*\([^=]*\)=.*/\1/p' | sed 's/[[:space:]]*$//')
          if [ "$k" = "$key" ]; then
            printf '%s = %s\n' "$key" "$val"
            found=1
          else
            printf '%s\n' "$line"
          fi
          ;;
      esac
    done < "$conf" > "$tmp"
  fi
  if [ "$found" -eq 0 ]; then
    printf '%s = %s\n' "$key" "$val" >> "$tmp"
  fi
  mv "$tmp" "$conf"
}

eclipse_host_merge_conf() {
  conf="$1"
  template="${2:-$CONF_TEMPLATE}"
  dir=$(dirname "$conf")
  mkdir -p "$dir"
  if [ ! -f "$conf" ]; then
    if [ -f "$template" ]; then
      cp "$template" "$conf"
    else
      printf '%s\n' \
        "lan_encryption_mode = 2" \
        "wan_encryption_mode = 2" \
        "origin_web_ui_allowed = lan" \
        "upnp = disabled" \
        "system_tray = disabled" > "$conf"
    fi
    return 0
  fi
  eclipse_upsert_conf_key "$conf" lan_encryption_mode 2
  eclipse_upsert_conf_key "$conf" wan_encryption_mode 2
  eclipse_upsert_conf_key "$conf" origin_web_ui_allowed lan
  eclipse_upsert_conf_key "$conf" upnp disabled
  eclipse_upsert_conf_key "$conf" system_tray disabled
}

eclipse_host_seed_creds() {
  mkdir -p "$SUNSHINE_DIR"
  state="$SUNSHINE_DIR/sunshine_state.json"
  passfile="$SUNSHINE_DIR/web-ui-password"
  if eclipse_creds_present "$state" && ! eclipse_creds_are_legacy "$state"; then
    return 0
  fi
  pass=$(eclipse_random_pass)
  [ -n "$pass" ] || return 0
  if [ -x /usr/bin/sunshine ]; then
    if [ -d /usr/lib/Eclipse ]; then
      (cd /usr/lib/Eclipse && CONFIGURATION_DIRECTORY=/etc/eclipse /usr/bin/sunshine --creds eclipse "$pass") || true
    else
      CONFIGURATION_DIRECTORY=/etc/eclipse /usr/bin/sunshine --creds eclipse "$pass" || true
    fi
  fi
  printf '%s\n' "$pass" > "$passfile"
  chmod 600 "$passfile" 2>/dev/null || true
  echo "Eclipse: seeded Sunshine Web UI credentials (username eclipse). Password: $passfile"
}

eclipse_host_open_firewall() {
  opened=0
  if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw --force delete allow 47984:47990/tcp >/dev/null 2>&1 || true
    ufw --force delete allow 48010/tcp >/dev/null 2>&1 || true
    ufw --force delete allow 47998:48010/udp >/dev/null 2>&1 || true
    ufw allow from "$CGNAT" to any port 47984:47990 proto tcp >/dev/null 2>&1 || true
    ufw allow from "$CGNAT" to any port 48010 proto tcp >/dev/null 2>&1 || true
    ufw allow from "$CGNAT" to any port 47998:48010 proto udp >/dev/null 2>&1 || true
    opened=1
  fi
  if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state 2>/dev/null | grep -q running; then
    firewall-cmd --permanent --remove-port=47984-47990/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-port=48010/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-port=47998-48010/udp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47984-47990 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=48010 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47998-48010 protocol=udp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47984-47990 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-rich-rule="rule family=ipv4 source address=${CGNAT} port port=48010 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47998-48010 protocol=udp accept" >/dev/null 2>&1 || true
    firewall-cmd --reload >/dev/null 2>&1 || true
    opened=1
  fi
  if [ "$opened" -eq 0 ]; then
    echo "Eclipse: no active ufw/firewalld; GameStream is not restricted to Tailscale (${CGNAT})."
  fi
}

eclipse_host_close_firewall() {
  if command -v ufw >/dev/null 2>&1; then
    ufw --force delete allow from "$CGNAT" to any port 47984:47990 proto tcp >/dev/null 2>&1 || true
    ufw --force delete allow from "$CGNAT" to any port 48010 proto tcp >/dev/null 2>&1 || true
    ufw --force delete allow from "$CGNAT" to any port 47998:48010 proto udp >/dev/null 2>&1 || true
    ufw --force delete allow 47984:47990/tcp >/dev/null 2>&1 || true
    ufw --force delete allow 48010/tcp >/dev/null 2>&1 || true
    ufw --force delete allow 47998:48010/udp >/dev/null 2>&1 || true
  fi
  if command -v firewall-cmd >/dev/null 2>&1; then
    firewall-cmd --permanent --remove-port=47984-47990/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-port=48010/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-port=47998-48010/udp >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47984-47990 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=48010 protocol=tcp accept" >/dev/null 2>&1 || true
    firewall-cmd --permanent --remove-rich-rule="rule family=ipv4 source address=${CGNAT} port port=47998-48010 protocol=udp accept" >/dev/null 2>&1 || true
    firewall-cmd --reload >/dev/null 2>&1 || true
  fi
}

eclipse_host_enable() {
  if [ ! -d /run/systemd/system ]; then
    return 0
  fi
  mkdir -p "$SUNSHINE_DIR"
  eclipse_host_merge_conf "$SUNSHINE_DIR/sunshine.conf"
  eclipse_host_seed_creds
  systemctl daemon-reload || true
  systemctl enable eclipse-host.service || true
  if systemctl is-active --quiet eclipse-host.service; then
    systemctl restart eclipse-host.service || true
  else
    systemctl start eclipse-host.service || true
  fi
}

eclipse_host_install() {
  eclipse_host_setcap /usr/bin/sunshine
  eclipse_host_udev

  if command -v groupadd >/dev/null 2>&1; then
    groupadd -f input 2>/dev/null || true
  fi

  if [ -n "${SUDO_USER:-}" ]; then
    eclipse_host_add_input_user "$SUDO_USER"
  elif [ -n "${PKEXEC_UID:-}" ]; then
    eclipse_host_add_input_user "$(getent passwd "$PKEXEC_UID" | cut -d: -f1)"
  fi

  if command -v loginctl >/dev/null 2>&1; then
    loginctl list-users --no-legend 2>/dev/null | awk '{print $2}' | while read -r u; do
      eclipse_host_add_input_user "$u"
    done
  fi

  chmod 755 /usr/lib/eclipse/eclipse-hostctl 2>/dev/null || true
  chmod 755 /usr/lib/eclipse/host-setup.sh 2>/dev/null || true
  chmod 755 /usr/lib/systemd/system-sleep/eclipse-host 2>/dev/null || true

  eclipse_host_enable
  eclipse_host_open_firewall

  echo "Eclipse: log out of the desktop session once so the input group applies."
  echo "NVIDIA encode uses the vendor GPU driver; the CUDA toolkit is not required at runtime."
  echo "Eclipse tray starts at login. Close hides to the tray; Quit Eclipse leaves the host service running."
  echo "Host streaming blocks idle sleep; lid-close and the power button still sleep the PC."
  echo "GameStream inbound is limited to Tailscale CGNAT ${CGNAT}."
}

eclipse_host_remove() {
  if [ -d /run/systemd/system ]; then
    systemctl disable --now eclipse-host.service 2>/dev/null || true
    systemctl daemon-reload || true
  fi
  eclipse_host_close_firewall
  eclipse_host_udev
}
