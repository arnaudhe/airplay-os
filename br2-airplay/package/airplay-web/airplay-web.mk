################################################################################
#
# airplay-web
#
################################################################################

AIRPLAY_WEB_VERSION = 0.1.0
AIRPLAY_WEB_SITE = $(BR2_EXTERNAL_AIRPLAY_PATH)/src/airplay-web
AIRPLAY_WEB_SITE_METHOD = local
AIRPLAY_WEB_LICENSE = MIT

# Un éventuel build local (cargo build) ne doit pas être recopié
AIRPLAY_WEB_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = --exclude target

# Lanceur pour init (respawn) et annonce mDNS du service HTTP
define AIRPLAY_WEB_INSTALL_EXTRA
	$(INSTALL) -D -m 0755 $(AIRPLAY_WEB_PKGDIR)/run-airplay-web \
		$(TARGET_DIR)/usr/libexec/airplay/run-airplay-web
	$(INSTALL) -D -m 0644 $(AIRPLAY_WEB_PKGDIR)/airplay-web.service \
		$(TARGET_DIR)/etc/avahi/services/airplay-web.service
endef
AIRPLAY_WEB_POST_INSTALL_TARGET_HOOKS += AIRPLAY_WEB_INSTALL_EXTRA

$(eval $(cargo-package))
