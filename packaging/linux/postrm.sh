#!/bin/sh
# Debian postrm and RPM %postun. Safe on upgrade.
set -e

CGNAT="100.64.0.0/10"

eclipse_postrm_close_firewall() {
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

if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules || true
fi

# Full uninstall only: Debian remove/purge, RPM remaining count 0.
if [ "$1" = "remove" ] || [ "$1" = "purge" ] || [ "$1" = "0" ]; then
  if [ -d /run/systemd/system ]; then
    systemctl disable --now eclipse-host.service 2>/dev/null || true
    systemctl daemon-reload || true
  fi
  eclipse_postrm_close_firewall
elif [ -d /run/systemd/system ]; then
  systemctl daemon-reload || true
fi
