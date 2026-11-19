#!/bin/sh
# post-image.sh — assemble la partition boot et l'image sdcard.img.
# config.txt et cmdline.txt sont déjà copiés dans rpi-firmware/ par le
# paquet rpi-firmware (BR2_PACKAGE_RPI_FIRMWARE_{CONFIG,CMDLINE}_FILE).
set -eu

BOARD_DIR="$(dirname "$0")"
GENIMAGE_CFG="${BOARD_DIR}/genimage.cfg"
GENIMAGE_TMP="${BUILD_DIR}/genimage.tmp"

# Configuration propre à chaque appareil (modèle par défaut)
install -m 0644 "${BOARD_DIR}/airplay.conf" "${BINARIES_DIR}/rpi-firmware/airplay.conf"

# genimage n'a pas besoin du rootfs (le squashfs est déjà construit)
ROOTPATH_TMP="$(mktemp -d)"
trap 'rm -rf "${ROOTPATH_TMP}"' EXIT

rm -rf "${GENIMAGE_TMP}"

genimage \
	--rootpath "${ROOTPATH_TMP}" \
	--tmppath "${GENIMAGE_TMP}" \
	--inputpath "${BINARIES_DIR}" \
	--outputpath "${BINARIES_DIR}" \
	--config "${GENIMAGE_CFG}"
