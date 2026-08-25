#!/bin/sh
# Debian postrm and RPM %postun. Safe on upgrade.
set -e

if command -v udevadm >/dev/null 2>&1; then
  udevadm control --reload-rules || true
fi

# Full uninstall only: Debian remove/purge, RPM remaining count 0.
if [ "$1" = "remove" ] || [ "$1" = "purge" ] || [ "$1" = "0" ]; then
  if [ -d /run/systemd/system ]; then
    systemctl disable --now eclipse-host.service 2>/dev/null || true
    systemctl daemon-reload || true
  fi
elif [ -d /run/systemd/system ]; then
  systemctl daemon-reload || true
fi
