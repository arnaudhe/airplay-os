#!/bin/sh
# post-build.sh — adapts the rootfs to run read-only.
# Called by Buildroot with $1 = TARGET_DIR.
set -eu

TARGET_DIR="$1"

# Mount point of the boot partition
mkdir -p "${TARGET_DIR}/boot"

# Configuration files generated at boot by S01bootconf in /run
ln -sfn /run/shairport-sync.conf "${TARGET_DIR}/etc/shairport-sync.conf"
ln -sfn /run/wpa_supplicant.conf "${TARGET_DIR}/etc/wpa_supplicant.conf"

# Authorized SSH keys, copied from /boot/authorized_keys at boot
mkdir -p "${TARGET_DIR}/root/.ssh"
chmod 0700 "${TARGET_DIR}/root/.ssh"
ln -sfn /run/authorized_keys "${TARGET_DIR}/root/.ssh/authorized_keys"

# /var/log is a symlink to /tmp in the Buildroot skeleton: turn it into a
# real directory so a dedicated tmpfs can be mounted on it (see /etc/fstab).
if [ -L "${TARGET_DIR}/var/log" ]; then
	rm -f "${TARGET_DIR}/var/log"
	mkdir -p "${TARGET_DIR}/var/log"
fi

# Packages install their own init scripts: nqptp and shairport-sync are
# started and supervised by init instead (inittab below).
rm -f "${TARGET_DIR}/etc/init.d/S90nqptp" \
      "${TARGET_DIR}/etc/init.d/S99shairport-sync"

# Former overlay scripts (before supervision by init). Buildroot never
# removes anything from output/target: without this cleanup an incremental
# build would keep them and start nqptp and shairport-sync twice.
rm -f "${TARGET_DIR}/etc/init.d/S49nqptp" \
      "${TARGET_DIR}/etc/init.d/S50shairport"

# Supervised services: BusyBox init starts them once rcS is done (network,
# dbus and avahi ready) and restarts them if they exit.
INITTAB="${TARGET_DIR}/etc/inittab"
sed -i '/# airplay-respawn$/d' "${INITTAB}"
cat >> "${INITTAB}" <<'INITTAB_EOF'
::respawn:/usr/libexec/airplay/run-nqptp # airplay-respawn
::respawn:/usr/libexec/airplay/run-shairport # airplay-respawn
INITTAB_EOF
if [ -x "${TARGET_DIR}/usr/libexec/airplay/run-airplay-web" ]; then
	echo '::respawn:/usr/libexec/airplay/run-airplay-web # airplay-respawn' >> "${INITTAB}"
fi
