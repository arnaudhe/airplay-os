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
docs/                       captures d'écran
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
│   └── rootfs_overlay/
│       ├── etc/fstab
│       ├── etc/shairport-sync.conf.tpl
│       ├── etc/init.d/S01bootconf S40network
│       └── usr/libexec/airplay/run-nqptp run-shairport respawn-guard
├── package/airplay-web/    paquet Buildroot de l'interface web
└── src/airplay-web/        sources Rust de l'interface web
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

## Construire l'image sur Linux (sans Docker)

Sur une machine Linux x86_64 ou arm64, Buildroot se lance directement. Prévoir
~40 Go de disque libre et un utilisateur **non-root**.

### Dépendances

Debian / Ubuntu :

```bash
sudo apt-get install -y build-essential gcc g++ make patch perl python3 file wget ca-certificates cpio unzip rsync bc git libncurses-dev bzip2 xz-utils gzip diffutils findutils sed bash
```

Fedora :

```bash
sudo dnf install -y gcc gcc-c++ make patch perl python3 file wget which cpio unzip rsync bc git ncurses-devel bzip2 xz gzip diffutils findutils sed bash perl-FindBin perl-IPC-Cmd perl-English perl-Thread-Queue
```

### Build

Depuis la racine de ce dépôt, Buildroot étant cloné à côté :

```bash
git clone https://gitlab.com/buildroot.org/buildroot.git ../buildroot
```

```bash
git -C ../buildroot checkout 2026.02.3
```

```bash
make -C ../buildroot BR2_EXTERNAL="$PWD/br2-airplay" airplay_rpi_armv7_defconfig
```

```bash
make -C ../buildroot -j"$(nproc)"
```

L'image est produite dans `../buildroot/output/images/sdcard.img`.
`BR2_EXTERNAL` est mémorisé dans `output/` : les `make` suivants (par exemple
`make -C ../buildroot menuconfig`, puis `savedefconfig` pour réécrire le
defconfig du dépôt) n'en ont plus besoin. Le cache ccache est placé à côté
de Buildroot, dans `../.buildroot-ccache`, et les sources téléchargées dans
`../buildroot/dl/`.

Remarques :
- la variable `PATH` ne doit contenir ni espace ni répertoire relatif (`.`) ;
  Buildroot refuse de démarrer sinon ;
- la variable `LD_LIBRARY_PATH` doit être vide (`unset LD_LIBRARY_PATH`) ;
- ne pas lancer le build en root.

### Flasher depuis Linux

Identifier la carte SD avec `lsblk` (ici `/dev/sdX`, à adapter : **toutes les
données du périphérique sont effacées**) :

```bash
sudo dd if=../buildroot/output/images/sdcard.img of=/dev/sdX bs=4M conv=fsync status=progress
```

## Flasher et configurer une enceinte

1. Flasher `sdcard.img` (balenaEtcher, ou `dd` vers `/dev/rdiskN`).
2. Sur la partition **BOOT** :
   - `config.txt` : garder **une seule** ligne `dtoverlay=` correspondant au HAT
     (par défaut `iqaudio-dacplus,unmute_amp` pour le DigiAMP+) ;
   - `airplay.conf` : `NAME`, Wi-Fi (`WIFI_SSID`, `WIFI_PSK`, `WIFI_COUNTRY`),
     `OUTPUT_DEVICE`, `MIXER`, `AUDIO_MODE` ;
   - facultatif : `authorized_keys` (clés publiques SSH pour `root`).
3. Éjecter proprement la carte (seul moment où la partition boot est écrite).

### Enceinte à un seul haut-parleur

`AUDIO_MODE` dans `airplay.conf` choisit le mixage des canaux, appliqué par
shairport-sync (`playback_mode`) avant le volume matériel :

| `AUDIO_MODE` | Sortie gauche | Sortie droite | Usage |
|---|---|---|---|
| `stereo` (défaut) | G | D | deux haut-parleurs |
| `mono` | (G+D)/2 | (G+D)/2 | un seul HP, branché d'un côté ou de l'autre |
| `left` | G | G | ne diffuser que le canal gauche |
| `right` | D | D | ne diffuser que le canal droit |
| `reverse` | D | G | haut-parleurs câblés à l'envers |

Une valeur inconnue retombe sur `stereo` (shairport-sync refuserait sinon de
démarrer).

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
- Relancer shairport-sync (après avoir modifié `/run/shairport-sync.conf` pour
  un essai, par exemple) : `killall shairport-sync` ; init le redémarre.

## Interface web (lecture seule)

Chaque enceinte affiche son état sur **`http://<hostname>.local`** (par
exemple `http://salon.local`). Le service est annoncé en mDNS (`_http._tcp`)
et la page se met à jour toutes les 3 s. Elle fonctionne en thème clair ou
sombre et s'adapte au téléphone.

![Interface web](docs/airplay-web.png)

- **Audio** : volume du mixer matériel, carte son, mixage (`AUDIO_MODE`), flux
  en cours (fréquence, format), indicateur « En lecture ».
- **Réseau** : IP, et pour le Wi-Fi le SSID, le signal, la bande et le canal
  ainsi que le débit.
- **Système** : uptime, charge, fréquence CPU, température, mémoire, alarme de
  **sous-tension** (pilote `raspberrypi-hwmon`).
- **Services** : processus attendus et depuis quand ils tournent (une relance
  récente se voit immédiatement).
- **Stockage** : `/` et `/boot` en lecture seule, nombre d'écritures sur la
  carte SD depuis le démarrage (critère de validation de la section 10).
- **Configuration** : valeurs de `/run/airplay.env` et lignes actives de
  `config.txt`.
- **Journal** : fin de `/var/log/messages`.

Rien n'est jamais écrit (seules les requêtes `GET`/`HEAD` sont acceptées) et le
mot de passe Wi-Fi n'est pas exposé. Il n'y a pas d'authentification : la page
est visible par tout le réseau local.

Le serveur (`src/airplay-web`) est écrit en Rust **sans aucune dépendance
externe** : le paquet se compile hors ligne avec le Rust précompilé de
Buildroot, et le binaire fait environ 500 Ko. HTML, CSS et JS sont intégrés au
binaire, sans police ni ressource externe. Il est lancé et supervisé par init
comme nqptp et shairport-sync. Pour le retirer de l'image :
`# BR2_PACKAGE_AIRPLAY_WEB is not set`.

Développement sur un poste Linux, sans Pi :

```bash
cd br2-airplay/src/airplay-web && cargo test
```

```bash
AIRPLAY_WEB_LISTEN=127.0.0.1:8080 cargo run
```

`AIRPLAY_WEB_ROOT=/chemin/vers/un/faux/arbre` permet de lire `/proc`, `/sys`,
`/run` et `/boot` depuis un autre dossier, pour simuler une enceinte.

## Séquence de démarrage

| Script | Rôle |
|---|---|
| `S01bootconf` | lit `/boot/airplay.conf`, règle le hostname, génère les configs dans `/run`, copie les clés SSH |
| `S10udevd` | eudev : charge les modules du HAT et du Wi-Fi |
| `S30dbus` | |
| `S40network` | eth0 en DHCP ; wlan0 via wpa_supplicant + DHCP, `power_save off` |
| `S50avahi-daemon` | annonce mDNS |
| `S50dropbear` | SSH |

Une fois `rcS` terminé, BusyBox init lance les services supervisés déclarés
dans `/etc/inittab` (action `respawn`, ajoutée par `post-build.sh`) et les
relance s'ils s'arrêtent :

| Service | Script | Rôle |
|---|---|---|
| nqptp | `run-nqptp` | horloge PTP (UDP 319/320, root) |
| shairport-sync | `run-shairport` | attend la carte son, applique `AMIXER_CMDS`, lance shairport-sync au premier plan |
| airplay-web | `run-airplay-web` | interface web en lecture seule, port 80 |

Pour ne pas boucler sur un service qui plante dès son lancement,
`respawn-guard` attend avant chaque relance : 2 s, puis 4, 8… jusqu'à 60 s. Le
délai revient à 2 s quand le service a tenu plus de 5 minutes. Les relances
sont tracées dans `/var/log/messages`.

## Écarts et précisions par rapport aux spécifications

- **Réglages `amixer`** : appliqués par `run-shairport` et non par
  `S01bootconf`, car la carte son n'existe qu'une fois les modules chargés par
  udev (S10).
- **Réseau** : script `S40network` dédié, qui remplace ifupdown (une seule
  image doit gérer Ethernet seul, Wi-Fi seul, ou les deux).
- **fstab** : en plus des lignes prévues, `proc`, `sysfs`, `devpts` et
  `/dev/shm` (mémoire partagée indispensable entre nqptp et shairport-sync).
  `/var/log`, qui est un lien vers `/tmp` dans le squelette Buildroot, devient
  un vrai répertoire pour y monter son tmpfs.
- **Supervision** : nqptp et shairport-sync ne sont pas lancés par des
  scripts `S49nqptp`/`S50shairport` mais par init (`inittab`, `respawn`), qui
  les relance en cas d'arrêt. `S90nqptp` et `S99shairport-sync` installés par
  Buildroot sont supprimés au post-build.
- **Interface web** : la section 2 l'excluait. Celle-ci est en lecture seule et
  peut être retirée de l'image d'une ligne dans le defconfig.
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
