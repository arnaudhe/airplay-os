#!/bin/sh
# post-build.sh — adapte le rootfs à un fonctionnement en lecture seule.
# Appelé par Buildroot avec $1 = TARGET_DIR.
set -eu

TARGET_DIR="$1"

# Point de montage de la partition boot
mkdir -p "${TARGET_DIR}/boot"

# Configurations générées au démarrage par S01bootconf dans /run
ln -sfn /run/shairport-sync.conf "${TARGET_DIR}/etc/shairport-sync.conf"
ln -sfn /run/wpa_supplicant.conf "${TARGET_DIR}/etc/wpa_supplicant.conf"

# Clés SSH autorisées, copiées depuis /boot/authorized_keys au démarrage
mkdir -p "${TARGET_DIR}/root/.ssh"
chmod 0700 "${TARGET_DIR}/root/.ssh"
ln -sfn /run/authorized_keys "${TARGET_DIR}/root/.ssh/authorized_keys"

# /var/log est un lien vers /tmp dans le squelette Buildroot : on en fait un
# vrai répertoire pour y monter un tmpfs dédié (voir /etc/fstab).
if [ -L "${TARGET_DIR}/var/log" ]; then
	rm -f "${TARGET_DIR}/var/log"
	mkdir -p "${TARGET_DIR}/var/log"
fi

# Les paquets installent leurs propres scripts d'init : nqptp et
# shairport-sync sont lancés et supervisés par init (inittab ci-dessous).
rm -f "${TARGET_DIR}/etc/init.d/S90nqptp" \
      "${TARGET_DIR}/etc/init.d/S99shairport-sync"

# Services supervisés : BusyBox init les lance une fois rcS terminé (réseau,
# dbus, avahi prêts) et les relance s'ils s'arrêtent.
INITTAB="${TARGET_DIR}/etc/inittab"
sed -i '/# airplay-respawn$/d' "${INITTAB}"
cat >> "${INITTAB}" <<'INITTAB_EOF'
::respawn:/usr/libexec/airplay/run-nqptp # airplay-respawn
::respawn:/usr/libexec/airplay/run-shairport # airplay-respawn
INITTAB_EOF
if [ -x "${TARGET_DIR}/usr/libexec/airplay/run-airplay-web" ]; then
	echo '::respawn:/usr/libexec/airplay/run-airplay-web # airplay-respawn' >> "${INITTAB}"
fi
