// Modèle de configuration shairport-sync.
// /run/shairport-sync.conf est généré à chaque démarrage par
// /etc/init.d/S01bootconf à partir de /boot/airplay.conf.
// Ne pas modifier /run/shairport-sync.conf : il est recréé au boot.

general =
{
	name = "@NAME@";
	output_backend = "alsa";
	mdns_backend = "avahi";
	interpolation = "@INTERPOLATION@";
	playback_mode = "@PLAYBACK_MODE@";
};

alsa =
{
	output_device = "@OUTPUT_DEVICE@";
	mixer_control_name = "@MIXER@";
};

diagnostics =
{
	log_output_to = "syslog";
	log_verbosity = 0;
};
