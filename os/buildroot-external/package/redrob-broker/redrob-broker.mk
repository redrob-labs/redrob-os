################################################################################
#
# redrob-broker
#
################################################################################

REDROB_BROKER_VERSION = local
REDROB_BROKER_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_BROKER_SITE_METHOD = local
REDROB_BROKER_LICENSE = Apache-2.0
REDROB_BROKER_LICENSE_FILES = ../../LICENSE
REDROB_BROKER_DEPENDENCIES = redrob-agent
REDROB_BROKER_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_BROKER_BINARY))

define REDROB_BROKER_BUILD_CMDS
	@test -x "$(REDROB_BROKER_BINARY)" || \
		{ echo "redrob-broker: binary not found at $(REDROB_BROKER_BINARY); run 'cargo build --release' in modules/credential-broker/broker first"; exit 1; }
endef

define REDROB_BROKER_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_BROKER_BINARY)" $(TARGET_DIR)/usr/bin/redrob-broker
	$(INSTALL) -D -m 0644 $(@D)/config/broker.toml $(TARGET_DIR)/etc/redrob/broker.toml
endef

define REDROB_BROKER_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-broker.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-broker.service
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants
	ln -sf ../redrob-broker.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-broker.service
endef

$(eval $(generic-package))
