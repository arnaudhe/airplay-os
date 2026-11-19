# airplay-os

Distribution Linux minimale (Buildroot) qui transforme des Raspberry Pi en
récepteurs **AirPlay 2** regroupables en multiroom depuis un iPhone ou un Mac.

- Racine **squashfs en lecture seule**, `/boot` monté `ro` : aucune écriture
  sur la carte SD en fonctionnement, tolérance aux coupures de courant.
- **Image unique** 32 bits ARMv7 pour Raspberry Pi 2 B et Pi 3 A+.
- Configuration propre à chaque enceinte dans `airplay.conf` sur la partition
  boot.

Base : **Buildroot 2026.02.3** (LTS), shairport-sync 4.3.7 (AirPlay 2) et
nqptp 1.2.4 fournis par Buildroot, noyau Raspberry Pi 6.12.

## Arborescence

```
Dockerfile                  environnement de build (Debian bookworm, arm64 natif)
scripts/build.sh            build dans Docker + récupération de sdcard.img
br2-airplay/                arbre BR2_EXTERNAL
├── configs/airplay_rpi_armv7_defconfig
├── board/airplay/
│   ├── config.txt          firmware : overlay du HAT, pas de arm_64bit
│   ├── cmdline.txt         root=/dev/mmcblk0p2 rootfstype=squashfs ro
│   ├── airplay.conf        modèle de configuration par appareil
│   ├── linux.fragment      CONFIG_SQUASHFS=y (module par défaut)
│   ├── genimage.cfg        boot FAT (64 Mo) + rootfs.squashfs
│   ├── post-build.sh       liens /etc -> /run, /var/log en tmpfs
│   ├── post-image.sh       airplay.conf + genimage
│   └── rootfs_overlay/etc/
│       ├── fstab
│       ├── shairport-sync.conf.tpl
│       └── init.d/S01bootconf S40network S49nqptp S50shairport
└── package/                paquets additionnels (vide)
```

## Construire l'image (macOS + Docker)

Prérequis Docker Desktop : 8 Go de RAM, tous les cœurs, ~40 Go de disque.
Sur Apple Silicon, l'image `debian:bookworm` est tirée en arm64 natif.

```bash
./scripts/build.sh
```

Le script crée l'image `buildroot-env` et le volume `br-work`, clone Buildroot
2026.02.3 dans le volume, applique `airplay_rpi_armv7_defconfig`, compile et
copie `sdcard.img` à la racine du dépôt. Le premier build (toolchain interne,
noyau, ffmpeg) prend plusieurs heures. ccache est activé (cache dans le volume).

Autres usages :

```bash
./scripts/build.sh shell                  # shell dans /work/buildroot
```

```bash
./scripts/build.sh menuconfig             # puis : ./scripts/build.sh savedefconfig
```

```bash
./scripts/build.sh shairport-sync-rebuild all
```

Après une modification du defconfig, réappliquer la configuration avec
`./scripts/build.sh defconfig`. Les commandes manuelles équivalentes sont dans
la section 8 des spécifications.

## Flasher et configurer une enceinte

1. Flasher `sdcard.img` (balenaEtcher, ou `dd` vers `/dev/rdiskN`).
2. Sur la partition **BOOT** :
   - `config.txt` : garder **une seule** ligne `dtoverlay=` correspondant au HAT
     (par défaut `iqaudio-dacplus,unmute_amp` pour le DigiAMP+) ;
   - `airplay.conf` : `NAME`, Wi-Fi (`WIFI_SSID`, `WIFI_PSK`, `WIFI_COUNTRY`),
     `OUTPUT_DEVICE`, `MIXER` ;
   - facultatif : `authorized_keys` (clés publiques SSH pour `root`).
3. Éjecter proprement la carte (seul moment où la partition boot est écrite).

Le hostname est déduit de `NAME` (`Salon` → `salon.local`) ; le définir avec
`HOSTNAME=` si le nom contient des accents.

## Diagnostic

```bash
ssh root@salon.local
```

- Connexion par clé uniquement (`/boot/authorized_keys`). La console HDMI
  `tty1` accepte `root` sans mot de passe.
- Les clés d'hôte SSH sont régénérées à chaque démarrage. Pour une empreinte
  fixe, copier `/run/dropbear/dropbear_ed25519_host_key` (généré au premier
  boot) à la racine de la partition boot.
- Logs : `/var/log/messages` (tmpfs, perdu au redémarrage).
- Configs générées : `/run/airplay.env`, `/run/shairport-sync.conf`,
  `/run/wpa_supplicant.conf`.

## Séquence de démarrage

| Script | Rôle |
|---|---|
| `S01bootconf` | lit `/boot/airplay.conf`, règle le hostname, génère les configs dans `/run`, copie les clés SSH |
| `S10udevd` | eudev : charge les modules du HAT et du Wi-Fi |
| `S30dbus` | |
| `S40network` | eth0 en DHCP ; wlan0 via wpa_supplicant + DHCP, `power_save off` |
| `S49nqptp` | horloge PTP (UDP 319/320, root) |
| `S50avahi-daemon` | annonce mDNS |
| `S50dropbear` | SSH |
| `S50shairport` | attend la carte son, applique `AMIXER_CMDS`, lance shairport-sync |

## Écarts et précisions par rapport aux spécifications

- **Réglages `amixer`** : appliqués par `S50shairport` et non par
  `S01bootconf`, car la carte son n'existe qu'une fois les modules chargés par
  udev (S10).
- **Réseau** : script `S40network` dédié, qui remplace ifupdown (une seule
  image doit gérer Ethernet seul, Wi-Fi seul, ou les deux).
- **fstab** : en plus des lignes prévues, `proc`, `sysfs`, `devpts` et
  `/dev/shm` (mémoire partagée indispensable entre nqptp et shairport-sync).
  `/var/log`, qui est un lien vers `/tmp` dans le squelette Buildroot, devient
  un vrai répertoire pour y monter son tmpfs.
- **Init des paquets** : `S90nqptp` et `S99shairport-sync` installés par
  Buildroot sont supprimés au post-build au profit de `S49nqptp` et
  `S50shairport`.
- **Bluetooth** coupé sur Pi 3 (`dtoverlay=disable-bt`) : il partage l'antenne
  du CYW43455.
- `config.txt` et `cmdline.txt` sont installés par le paquet `rpi-firmware`
  (`BR2_PACKAGE_RPI_FIRMWARE_{CONFIG,CMDLINE}_FILE`) ; `post-image.sh` ajoute
  `airplay.conf` et appelle genimage. Le noyau est copié en `kernel7.img`.

## Points vérifiés (Buildroot 2026.02.3)

- Option AirPlay 2 : `BR2_PACKAGE_SHAIRPORT_SYNC_AIRPLAY2` (sélectionne
  `nqptp`, ffmpeg, libplist, libsodium, libgcrypt). Aucun paquet externe requis.
- Firmware Wi-Fi : `BR2_PACKAGE_BRCMFMAC_SDIO_FIRMWARE_RPI_WIFI`.
- DTB : il n'existe pas de `bcm2710-rpi-3-a-plus.dtb` ; le firmware du 3 A+
  charge `bcm2710-rpi-3-b-plus.dtb`, qui est inclus.
- Overlays `iqaudio-dacplus`, `hifiberry-dacplusadc` et
  `hifiberry-dacplusadcpro` présents.
- `bcm2709_defconfig` a `CONFIG_SQUASHFS=m` : le fragment `linux.fragment` le
  passe en `y`, sinon la racine ne peut pas être montée.

Restent à vérifier sur le matériel : noms des cartes et mixers ALSA
(`aplay -l`, `amixer scontrols`) et version exacte de la HiFiBerry.

## Plan de réalisation

Les étapes 1 à 6 du plan (section 9 des spécifications) sont regroupées dans
le defconfig ; la validation (section 10) se fait sur le matériel.

- [ ] `mount` : `/` en `squashfs (ro)`, `/boot` en `vfat (ro)`
- [ ] Aucune écriture sur `mmcblk0` (`cat /proc/diskstats`)
- [ ] 10 coupures de courant pendant la lecture, 10 redémarrages corrects
- [ ] Même image sur Pi 2 B et Pi 3 A+
- [ ] Son et volume matériel sur DigiAMP+ et DAC+ ADC
- [ ] Enceintes visibles et sélectionnables ensemble sur iOS
- [ ] Synchro correcte entre deux enceintes de la même pièce
- [ ] Wi-Fi stable sur le 3 A+ pendant une longue écoute
