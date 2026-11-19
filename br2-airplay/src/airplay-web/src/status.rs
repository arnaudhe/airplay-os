//! Collecte de l'état du système, uniquement en lecture (/proc, /sys, /run,
//! /boot et quelques commandes : ip, iw, amixer).

use crate::json::Json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Services surveillés : (nom du processus, libellé, indispensable)
const SERVICES: &[(&str, &str, bool)] = &[
    ("shairport-sync", "AirPlay (shairport-sync)", true),
    ("nqptp", "Horloge PTP (nqptp)", true),
    ("avahi-daemon", "Annonce mDNS (avahi)", true),
    ("dbus-daemon", "D-Bus", true),
    ("wpa_supplicant", "Wi-Fi (wpa_supplicant)", false),
    ("udhcpc", "Client DHCP", false),
    ("dropbear", "SSH (dropbear)", false),
    ("udevd", "udev", false),
    ("syslogd", "syslog", false),
];

/// Racine des fichiers lus : "/" sur le Pi, un faux arbre en développement.
pub struct Sys {
    root: PathBuf,
}

impl Sys {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Sys { root: root.into() }
    }

    fn path(&self, p: &str) -> PathBuf {
        self.root.join(p.trim_start_matches('/'))
    }

    fn read(&self, p: &str) -> Option<String> {
        fs::read_to_string(self.path(p)).ok()
    }

    fn read_trim(&self, p: &str) -> Option<String> {
        self.read(p)
            .map(|s| s.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string())
            .filter(|s| !s.is_empty())
    }

    fn list_dir(&self, p: &str) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.path(p))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Dernières lignes du journal système.
    pub fn logs(&self, max_lines: usize) -> String {
        let text = self.read("/var/log/messages").unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..].join("\n")
    }

    pub fn status(&self) -> Json {
        let env = parse_env(&self.read("/run/airplay.env").unwrap_or_default());
        let get = |k: &str| {
            env.iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let uptime = self
            .read("/proc/uptime")
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok());

        Json::Obj(vec![
            ("name", Json::str(get("NAME"))),
            ("hostname", Json::opt(self.read_trim("/proc/sys/kernel/hostname"), Json::Str)),
            ("system", self.system(uptime)),
            ("audio", self.audio(&get("OUTPUT_DEVICE"), &get("MIXER"), &get("AUDIO_MODE"))),
            ("network", self.network()),
            ("services", self.services(uptime)),
            ("storage", self.storage()),
            (
                "config",
                Json::Arr(
                    env.into_iter()
                        .map(|(k, v)| Json::Arr(vec![Json::Str(k), Json::Str(v)]))
                        .collect(),
                ),
            ),
            ("boot_config", self.boot_config()),
        ])
    }

    fn system(&self, uptime: Option<f64>) -> Json {
        let load: Vec<Json> = self
            .read("/proc/loadavg")
            .map(|s| {
                s.split_whitespace()
                    .take(3)
                    .filter_map(|v| v.parse().ok())
                    .map(Json::Num)
                    .collect()
            })
            .unwrap_or_default();

        let meminfo = self.read("/proc/meminfo").unwrap_or_default();
        let mem = |key: &str| {
            meminfo
                .lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
        };

        let temp = self
            .read_trim("/sys/class/thermal/thermal_zone0/temp")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|t| t / 1000.0);
        let freq = self
            .read_trim("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|f| f / 1000.0);

        Json::Obj(vec![
            ("model", Json::opt(self.read_trim("/proc/device-tree/model"), Json::Str)),
            ("kernel", Json::opt(self.read_trim("/proc/sys/kernel/osrelease"), Json::Str)),
            ("uptime_s", Json::opt(uptime, Json::Num)),
            ("load", Json::Arr(load)),
            ("mem_total_kb", Json::opt(mem("MemTotal:"), Json::Num)),
            ("mem_available_kb", Json::opt(mem("MemAvailable:"), Json::Num)),
            ("temp_c", Json::opt(temp, Json::Num)),
            ("cpu_mhz", Json::opt(freq, Json::Num)),
            ("undervoltage", Json::opt(self.undervoltage(), Json::Bool)),
        ])
    }

    /// Alarme de sous-tension du pilote raspberrypi-hwmon ("rpi_volt").
    fn undervoltage(&self) -> Option<bool> {
        self.list_dir("/sys/class/hwmon").into_iter().find_map(|h| {
            let dir = format!("/sys/class/hwmon/{h}");
            if self.read_trim(&format!("{dir}/name"))? != "rpi_volt" {
                return None;
            }
            Some(self.read_trim(&format!("{dir}/in0_lcrit_alarm"))? != "0")
        })
    }

    fn audio(&self, output_device: &str, mixer: &str, mode: &str) -> Json {
        let cards = parse_cards(&self.read("/proc/asound/cards").unwrap_or_default());

        // Premier flux de lecture ouvert, toutes cartes confondues
        let mut stream = Json::Null;
        let mut playing = false;
        'cards: for (index, _, _) in &cards {
            let card_dir = format!("/proc/asound/card{index}");
            for pcm in self.list_dir(&card_dir).into_iter().filter(|p| p.ends_with('p')) {
                for sub in self
                    .list_dir(&format!("{card_dir}/{pcm}"))
                    .into_iter()
                    .filter(|s| s.starts_with("sub"))
                {
                    let base = format!("{card_dir}/{pcm}/{sub}");
                    let status = self.read(&format!("{base}/status")).unwrap_or_default();
                    let state = field(&status, "state").unwrap_or_default();
                    if state.is_empty() {
                        continue; // "closed"
                    }
                    let params = self.read(&format!("{base}/hw_params")).unwrap_or_default();
                    playing = state == "RUNNING";
                    stream = Json::Obj(vec![
                        ("state", Json::str(state)),
                        ("format", Json::opt(field(&params, "format"), Json::Str)),
                        (
                            "rate",
                            Json::opt(
                                field(&params, "rate")
                                    .and_then(|r| r.split_whitespace().next()?.parse().ok()),
                                Json::Num,
                            ),
                        ),
                        (
                            "channels",
                            Json::opt(field(&params, "channels").and_then(|c| c.parse().ok()), Json::Num),
                        ),
                    ]);
                    break 'cards;
                }
            }
        }

        let volume = if mixer.is_empty() || cards.is_empty() {
            Json::Null
        } else {
            let card = alsa_card(output_device);
            run("amixer", &["-c", &card, "sget", mixer])
                .map(|out| {
                    let (percent, db) = parse_amixer(&out);
                    Json::Obj(vec![
                        ("percent", Json::opt(percent, Json::Num)),
                        ("db", Json::opt(db, Json::Num)),
                    ])
                })
                .unwrap_or(Json::Null)
        };

        Json::Obj(vec![
            ("playing", Json::Bool(playing)),
            (
                "cards",
                Json::Arr(
                    cards
                        .into_iter()
                        .map(|(i, id, name)| {
                            Json::Obj(vec![
                                ("index", Json::Num(i as f64)),
                                ("id", Json::Str(id)),
                                ("name", Json::Str(name)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("stream", stream),
            ("output_device", Json::str(output_device)),
            ("mixer", Json::str(mixer)),
            ("mode", Json::str(if mode.is_empty() { "stereo" } else { mode })),
            ("volume", volume),
        ])
    }

    fn network(&self) -> Json {
        let addrs = run("ip", &["-o", "-4", "addr", "show"]).unwrap_or_default();
        let ifaces = self
            .list_dir("/sys/class/net")
            .into_iter()
            .filter(|i| i != "lo")
            .map(|iface| {
                let ipv4: Vec<Json> = addrs
                    .lines()
                    .filter_map(|l| {
                        let mut words = l.split_whitespace();
                        let name = words.nth(1)?;
                        if name != iface {
                            return None;
                        }
                        let mut words = words.skip_while(|w| *w != "inet");
                        words.next()?;
                        Some(Json::str(words.next()?))
                    })
                    .collect();
                let wifi = if Path::new(&self.path(&format!("/sys/class/net/{iface}/wireless"))).exists()
                {
                    run("iw", &["dev", &iface, "link"])
                        .map(|out| parse_iw_link(&out))
                        .unwrap_or(Json::Null)
                } else {
                    Json::Null
                };
                Json::Obj(vec![
                    (
                        "mac",
                        Json::opt(self.read_trim(&format!("/sys/class/net/{iface}/address")), Json::Str),
                    ),
                    (
                        "state",
                        Json::opt(self.read_trim(&format!("/sys/class/net/{iface}/operstate")), Json::Str),
                    ),
                    ("ipv4", Json::Arr(ipv4)),
                    ("wifi", wifi),
                    ("name", Json::Str(iface)),
                ])
            })
            .collect();
        Json::Arr(ifaces)
    }

    fn services(&self, uptime: Option<f64>) -> Json {
        // nom du processus -> (pid, démarré depuis N secondes)
        let mut found: Vec<(String, u32, Option<f64>)> = Vec::new();
        for pid in self.list_dir("/proc").into_iter().filter_map(|p| p.parse::<u32>().ok()) {
            let Some(comm) = self.read_trim(&format!("/proc/{pid}/comm")) else {
                continue;
            };
            if !SERVICES.iter().any(|(name, _, _)| *name == comm) {
                continue;
            }
            // Champ 22 de /proc/<pid>/stat : démarrage, en ticks (USER_HZ = 100)
            let started = self.read(&format!("/proc/{pid}/stat")).and_then(|s| {
                let after_comm = &s[s.rfind(')')? + 1..];
                after_comm.split_whitespace().nth(19)?.parse::<f64>().ok()
            });
            let age = match (uptime, started) {
                (Some(u), Some(s)) => Some((u - s / 100.0).max(0.0)),
                _ => None,
            };
            found.push((comm, pid, age));
        }

        Json::Arr(
            SERVICES
                .iter()
                .map(|(name, label, essential)| {
                    // Plusieurs processus (udhcpc par interface) : le plus ancien
                    let proc = found
                        .iter()
                        .filter(|(c, _, _)| c == name)
                        .max_by(|a, b| a.2.unwrap_or(0.0).total_cmp(&b.2.unwrap_or(0.0)));
                    Json::Obj(vec![
                        ("name", Json::str(*name)),
                        ("label", Json::str(*label)),
                        ("essential", Json::Bool(*essential)),
                        ("running", Json::Bool(proc.is_some())),
                        ("pid", Json::opt(proc.map(|p| p.1), |p| Json::Num(p as f64))),
                        ("age_s", Json::opt(proc.and_then(|p| p.2), Json::Num)),
                    ])
                })
                .collect(),
        )
    }

    fn storage(&self) -> Json {
        let mounts = self.read("/proc/mounts").unwrap_or_default();
        let mount = |target: &str| {
            // Dernier montage sur ce point (celui qui est visible)
            mounts
                .lines()
                .filter_map(|l| {
                    let f: Vec<&str> = l.split_whitespace().collect();
                    (f.len() >= 4 && f[1] == target).then(|| (f[0], f[2], f[3]))
                })
                .next_back()
                .map(|(dev, fs, opts)| {
                    Json::Obj(vec![
                        ("device", Json::str(dev)),
                        ("fs", Json::str(fs)),
                        ("ro", Json::Bool(opts.split(',').any(|o| o == "ro"))),
                    ])
                })
                .unwrap_or(Json::Null)
        };

        let sd = self.read("/proc/diskstats").and_then(|s| {
            s.lines().find_map(|l| {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.len() < 10 || f[2] != "mmcblk0" {
                    return None;
                }
                Some(Json::Obj(vec![
                    ("reads", Json::Num(f[3].parse().ok()?)),
                    ("writes", Json::Num(f[7].parse().ok()?)),
                    ("sectors_written", Json::Num(f[9].parse().ok()?)),
                ]))
            })
        });

        Json::Obj(vec![
            ("root", mount("/")),
            ("boot", mount("/boot")),
            ("sd", sd.unwrap_or(Json::Null)),
        ])
    }

    /// Lignes actives de config.txt (overlays, paramètres), avec leur section.
    fn boot_config(&self) -> Json {
        let text = self.read("/boot/config.txt").unwrap_or_default();
        let mut section = String::from("all");
        let mut lines = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].to_string();
            } else if line.starts_with("dtoverlay=") || line.starts_with("dtparam=") {
                lines.push(Json::Obj(vec![
                    ("section", Json::str(section.clone())),
                    ("line", Json::str(line)),
                ]));
            }
        }
        Json::Arr(lines)
    }
}

/// Exécute une commande et renvoie sa sortie standard si elle réussit.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Valeur d'une ligne "clé: valeur".
fn field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

/// Lit /run/airplay.env (lignes CLE='valeur' produites par S01bootconf).
pub fn parse_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), shell_unquote(v)))
        })
        .collect()
}

/// Défait le quoting shell 'abc'\''def' utilisé dans airplay.env.
fn shell_unquote(s: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('\'', _) => quoted = !quoted,
            ('\\', false) => {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            (c, _) => out.push(c),
        }
    }
    out
}

/// /proc/asound/cards -> (index, id, nom)
fn parse_cards(text: &str) -> Vec<(u32, String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (index, rest) = l.split_once(' ')?;
            let index = index.parse().ok()?;
            let id = rest.split_once('[')?.1.split_once(']')?.0.trim().to_string();
            let name = rest
                .split_once("]:")
                .map(|(_, n)| n.split(" - ").next().unwrap_or(n).trim().to_string())
                .unwrap_or_default();
            Some((index, id, name))
        })
        .collect()
}

/// "hw:0", "plughw:CARD=IQaudIODAC,DEV=0", "default" -> carte pour amixer -c
fn alsa_card(device: &str) -> String {
    device
        .split_once(':')
        .map(|(_, spec)| {
            let card = spec.split(',').next().unwrap_or("0");
            card.strip_prefix("CARD=").unwrap_or(card).to_string()
        })
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| "0".to_string())
}

/// Premier "[NN%]" et "[-12.50dB]" de la sortie de amixer sget.
fn parse_amixer(out: &str) -> (Option<f64>, Option<f64>) {
    let mut percent = None;
    let mut db = None;
    for part in out.split('[').skip(1) {
        let Some(value) = part.split(']').next() else { continue };
        if percent.is_none() {
            percent = value.strip_suffix('%').and_then(|v| v.parse().ok());
        }
        if db.is_none() {
            db = value.strip_suffix("dB").and_then(|v| v.parse().ok());
        }
    }
    (percent, db)
}

fn parse_iw_link(out: &str) -> Json {
    if !out.starts_with("Connected") {
        return Json::Obj(vec![("connected", Json::Bool(false))]);
    }
    let num = |key: &str| {
        field(out, key).and_then(|v| v.split_whitespace().next()?.parse::<f64>().ok())
    };
    Json::Obj(vec![
        ("connected", Json::Bool(true)),
        ("ssid", Json::opt(field(out, "SSID"), Json::Str)),
        ("freq_mhz", Json::opt(num("freq"), Json::Num)),
        ("signal_dbm", Json::opt(num("signal"), Json::Num)),
        ("tx_mbps", Json::opt(num("tx bitrate"), Json::Num)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_unquote() {
        let env = parse_env("NAME='Chez l'\\''ami'\nMIXER=''\n");
        assert_eq!(env[0], ("NAME".into(), "Chez l'ami".into()));
        assert_eq!(env[1], ("MIXER".into(), "".into()));
    }

    #[test]
    fn cards() {
        let text = " 0 [IQaudIODAC     ]: IQaudIODAC - IQaudIODAC\n                      IQaudIODAC\n";
        assert_eq!(parse_cards(text), vec![(0, "IQaudIODAC".into(), "IQaudIODAC".into())]);
        assert!(parse_cards("--- no soundcards ---\n").is_empty());
    }

    #[test]
    fn card_from_device() {
        assert_eq!(alsa_card("hw:0"), "0");
        assert_eq!(alsa_card("plughw:CARD=IQaudIODAC,DEV=0"), "IQaudIODAC");
        assert_eq!(alsa_card("default"), "0");
    }

    #[test]
    fn amixer() {
        let out = "Simple mixer control 'Digital',0\n  Front Left: Playback 156 [75%] [-25.50dB] [on]\n";
        assert_eq!(parse_amixer(out), (Some(75.0), Some(-25.5)));
    }

    #[test]
    fn iw_link() {
        let out = "Connected to aa:bb:cc:dd:ee:ff (on wlan0)\n\tSSID: Maison\n\tfreq: 5180\n\tsignal: -57 dBm\n\ttx bitrate: 433.3 MBit/s VHT-MCS 9\n";
        assert_eq!(
            parse_iw_link(out).to_string(),
            r#"{"connected":true,"ssid":"Maison","freq_mhz":5180,"signal_dbm":-57,"tx_mbps":433.3}"#
        );
        assert_eq!(parse_iw_link("Not connected.\n").to_string(), r#"{"connected":false}"#);
    }
}
