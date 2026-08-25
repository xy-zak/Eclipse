#!/bin/sh
# Debian postinst and RPM %post. Idempotent. Do not use bash-only syntax.
set -e

CAPS="cap_sys_admin,cap_sys_nice+p"

setcap_sunshine() {
  bin="$1"
  if [ -x "$bin" ] && command -v setcap >/dev/null 2>&1; then
    setcap "$CAPS" "$bin" || true
  fi
}

setcap_sunshine /usr/bin/sunshine

if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules || true
  udevadm trigger --property-match=DEVNAME=/dev/uinput || true
  udevadm trigger --property-match=DEVNAME=/dev/uhid || true
fi

modprobe uinput 2>/dev/null || true
modprobe uhid 2>/dev/null || true

if command -v groupadd >/dev/null 2>&1; then
  groupadd -f input 2>/dev/null || true
fi

add_input_user() {
  u="$1"
  if [ -z "$u" ] || [ "$u" = "root" ]; then
    return 0
  fi
  if command -v usermod >/dev/null 2>&1; then
    usermod -aG input "$u" 2>/dev/null || true
  fi
}

if [ -n "${SUDO_USER:-}" ]; then
  add_input_user "$SUDO_USER"
elif [ -n "${PKEXEC_UID:-}" ]; then
  add_input_user "$(getent passwd "$PKEXEC_UID" | cut -d: -f1)"
fi

if command -v loginctl >/dev/null 2>&1; then
  loginctl list-users --no-legend 2>/dev/null | awk '{print $2}' | while read -r u; do
    add_input_user "$u"
  done
fi

echo "Eclipse: log out of the desktop session once so the input group applies."
echo "NVIDIA encode uses the vendor GPU driver; the CUDA toolkit is not required at runtime."
echo "Eclipse tray starts at login. Close hides to the tray; Quit Eclipse leaves the host service running."

enable_eclipse_host() {
  if [ ! -d /run/systemd/system ]; then
    return 0
  fi
  mkdir -p /etc/eclipse/sunshine
  if [ -x /usr/bin/sunshine ]; then
    if [ -d /usr/lib/Eclipse ]; then
      (cd /usr/lib/Eclipse && CONFIGURATION_DIRECTORY=/etc/eclipse /usr/bin/sunshine --creds eclipse eclipse) || true
    else
      CONFIGURATION_DIRECTORY=/etc/eclipse /usr/bin/sunshine --creds eclipse eclipse || true
    fi
  fi
  systemctl daemon-reload || true
  systemctl enable --now eclipse-host.service || true
}

chmod 755 /usr/lib/eclipse/eclipse-hostctl 2>/dev/null || true

enable_eclipse_host

open_gamestream_firewall() {
  if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q "Status: active"; then
    ufw allow 47984:47990/tcp >/dev/null 2>&1 || true
    ufw allow 48010/tcp >/dev/null 2>&1 || true
    ufw allow 47998:48010/udp >/dev/null 2>&1 || true
  fi
  if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state 2>/dev/null | grep -q running; then
    firewall-cmd --permanent --add-port=47984-47990/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-port=48010/tcp >/dev/null 2>&1 || true
    firewall-cmd --permanent --add-port=47998-48010/udp >/dev/null 2>&1 || true
    firewall-cmd --reload >/dev/null 2>&1 || true
  fi
}

open_gamestream_firewall
