#!/bin/sh
# post-image.sh — assembles the boot partition and the sdcard.img image.
# config.txt and cmdline.txt are already copied into rpi-firmware/ by the
# rpi-firmware package (BR2_PACKAGE_RPI_FIRMWARE_{CONFIG,CMDLINE}_FILE).
set -eu

BOARD_DIR="$(dirname "$0")"
GENIMAGE_CFG="${BOARD_DIR}/genimage.cfg"
GENIMAGE_TMP="${BUILD_DIR}/genimage.tmp"

# Per-device configuration (default template)
install -m 0644 "${BOARD_DIR}/airplay.conf" "${BINARIES_DIR}/rpi-firmware/airplay.conf"

# genimage does not need the rootfs (the squashfs is already built)
ROOTPATH_TMP="$(mktemp -d)"
trap 'rm -rf "${ROOTPATH_TMP}"' EXIT

rm -rf "${GENIMAGE_TMP}"

genimage \
	--rootpath "${ROOTPATH_TMP}" \
	--tmppath "${GENIMAGE_TMP}" \
	--inputpath "${BINARIES_DIR}" \
	--outputpath "${BINARIES_DIR}" \
	--config "${GENIMAGE_CFG}"
