// shairport-sync configuration template.
// /run/shairport-sync.conf is generated at every boot by
// /etc/init.d/S01bootconf from /boot/airplay.conf.
// Do not edit /run/shairport-sync.conf: it is recreated at boot.

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
