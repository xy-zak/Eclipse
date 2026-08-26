#!/bin/sh
# Debian postinst and RPM %post. Files are already unpacked.
set -e
. /usr/lib/eclipse/host-setup.sh
eclipse_host_install
