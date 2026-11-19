#!/bin/sh
# build.sh — construit sdcard.img dans Docker (à lancer depuis la racine du dépôt).
#
#   ./scripts/build.sh            build complet (ou incrémental), copie sdcard.img ici
#   ./scripts/build.sh shell      shell dans le conteneur, dans /work/buildroot
#   ./scripts/build.sh <cible>    ex. : menuconfig, linux-menuconfig, savedefconfig,
#                                       shairport-sync-rebuild, clean
#
# Le build vit dans le volume Docker "br-work" (système de fichiers sensible à
# la casse) ; seul l'arbre BR2_EXTERNAL du dépôt est monté depuis le Mac.
set -eu

BUILDROOT_VERSION="${BUILDROOT_VERSION:-2026.02.3}"
IMAGE=buildroot-env
VOLUME=br-work
DEFCONFIG=airplay_rpi_armv7_defconfig
REPO="$(cd "$(dirname "$0")/.." && pwd)"

docker build -t "$IMAGE" "$REPO"
if ! docker volume inspect "$VOLUME" >/dev/null 2>&1; then
	docker volume create "$VOLUME"
	docker run --rm -u root -v "$VOLUME":/work "$IMAGE" chown br /work
fi

run() {
	docker run --rm $DOCKER_TTY -v "$VOLUME":/work -v "$REPO/br2-airplay":/external \
		"$IMAGE" bash -c "$1"
}
[ -t 0 ] && DOCKER_TTY=-it || DOCKER_TTY=-i

PREPARE="set -e
if [ ! -d /work/buildroot ]; then
	git clone https://gitlab.com/buildroot.org/buildroot.git /work/buildroot
fi
cd /work/buildroot
if [ \"\$(git describe --tags --exact-match 2>/dev/null)\" != \"$BUILDROOT_VERSION\" ]; then
	git fetch --tags origin
	git checkout -q $BUILDROOT_VERSION
fi
[ -f .config ] || make BR2_EXTERNAL=/external $DEFCONFIG"

case "${1:-all}" in
	all)
		run "$PREPARE
make -j\$(nproc)"
		docker run --rm -v "$VOLUME":/work -v "$REPO":/out "$IMAGE" \
			cp /work/buildroot/output/images/sdcard.img /out/
		echo "Image : $REPO/sdcard.img"
		;;
	defconfig)
		run "$PREPARE
make BR2_EXTERNAL=/external $DEFCONFIG"
		;;
	shell)
		run "$PREPARE
exec bash"
		;;
	*)
		run "$PREPARE
make $*"
		;;
esac
