################################################################################
#
# redrob-agent
#
################################################################################

REDROB_AGENT_VERSION = local
REDROB_AGENT_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_AGENT_SITE_METHOD = local
REDROB_AGENT_LICENSE = Apache-2.0
REDROB_AGENT_LICENSE_FILES = ../../LICENSE
REDROB_AGENT_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_AGENT_BINARY))

define REDROB_AGENT_BUILD_CMDS
	@test -x "$(REDROB_AGENT_BINARY)" || \
		{ echo "redrob-agent: binary not found at $(REDROB_AGENT_BINARY); run 'cargo build --release' in agent/ first"; exit 1; }
endef

define REDROB_AGENT_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_AGENT_BINARY)" $(TARGET_DIR)/usr/bin/redrob-agent
	$(INSTALL) -D -m 0644 $(@D)/config/agent.toml $(TARGET_DIR)/usr/share/redrob/agent.toml
	$(INSTALL) -D -m 0755 $(@D)/firstboot/redrob-firstboot $(TARGET_DIR)/usr/libexec/redrob-firstboot
endef

define REDROB_AGENT_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-agent.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-agent.service
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-firstboot.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-firstboot.service
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-state-dirs.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-state-dirs.service
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-mark-good.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-mark-good.service
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants
	ln -sf ../redrob-agent.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-agent.service
	ln -sf ../redrob-firstboot.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-firstboot.service
	ln -sf ../redrob-state-dirs.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-state-dirs.service
	ln -sf ../redrob-mark-good.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-mark-good.service
endef

define REDROB_AGENT_USERS
	redrob-agent -1 redrob-agent -1 * /mnt/data/redrob/agent /bin/false - Redrob agent runtime
	redrob-broker -1 redrob-broker -1 * /mnt/data/redrob/broker /bin/false - Redrob credential broker
endef

$(eval $(generic-package))
