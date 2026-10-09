################################################################################
#
# redrob-pairing
#
################################################################################

REDROB_PAIRING_VERSION = local
REDROB_PAIRING_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_PAIRING_SITE_METHOD = local
REDROB_PAIRING_LICENSE = Apache-2.0
REDROB_PAIRING_LICENSE_FILES = ../../LICENSE
REDROB_PAIRING_DEPENDENCIES = redrob-agent
REDROB_PAIRING_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_PAIRING_BINARY))

define REDROB_PAIRING_BUILD_CMDS
	@test -x "$(REDROB_PAIRING_BINARY)" || \
		{ echo "redrob-pairing: binary not found at $(REDROB_PAIRING_BINARY); run 'cargo build --release' in tools/redrob-pairing first"; exit 1; }
endef

define REDROB_PAIRING_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_PAIRING_BINARY)" $(TARGET_DIR)/usr/bin/redrob-pairing
endef

define REDROB_PAIRING_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-pairing.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-pairing.service
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-pairing.timer \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-pairing.timer
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/timers.target.wants
	ln -sf ../redrob-pairing.timer \
		$(TARGET_DIR)/usr/lib/systemd/system/timers.target.wants/redrob-pairing.timer
endef

$(eval $(generic-package))
