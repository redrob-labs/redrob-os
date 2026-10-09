################################################################################
#
# redrob-display
#
################################################################################

REDROB_DISPLAY_VERSION = local
REDROB_DISPLAY_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_DISPLAY_SITE_METHOD = local
REDROB_DISPLAY_LICENSE = Apache-2.0
REDROB_DISPLAY_LICENSE_FILES = ../../LICENSE
REDROB_DISPLAY_DEPENDENCIES = redrob-agent dejavu
REDROB_DISPLAY_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_DISPLAY_BINARY))

define REDROB_DISPLAY_BUILD_CMDS
	@test -x "$(REDROB_DISPLAY_BINARY)" || \
		{ echo "redrob-display: binary not found at $(REDROB_DISPLAY_BINARY); run 'cargo build --release' in modules/display/kiosk first"; exit 1; }
endef

define REDROB_DISPLAY_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_DISPLAY_BINARY)" $(TARGET_DIR)/usr/bin/redrob-display
endef

define REDROB_DISPLAY_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-display.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-display.service
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-display-mode.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-display-mode.service
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants
	ln -sf ../redrob-display-mode.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-display-mode.service
endef

# udev (systemd package) already defines the input group and puts /dev/input/* in it;
# redrob-display is the only member of video and input.
define REDROB_DISPLAY_USERS
	redrob-display -1 redrob-display -1 * /mnt/data/redrob/display /bin/false video,input Redrob display module
endef

$(eval $(generic-package))
