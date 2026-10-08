################################################################################
#
# redrob-usb-broker
#
################################################################################

REDROB_USB_BROKER_VERSION = local
REDROB_USB_BROKER_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_USB_BROKER_SITE_METHOD = local
REDROB_USB_BROKER_LICENSE = Apache-2.0
REDROB_USB_BROKER_LICENSE_FILES = ../../LICENSE
REDROB_USB_BROKER_DEPENDENCIES = redrob-agent
REDROB_USB_BROKER_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_USB_BROKER_BINARY))

define REDROB_USB_BROKER_BUILD_CMDS
	@test -x "$(REDROB_USB_BROKER_BINARY)" || \
		{ echo "redrob-usb-broker: binary not found at $(REDROB_USB_BROKER_BINARY); run 'cargo build --release' in modules/usb-broker/broker first"; exit 1; }
endef

define REDROB_USB_BROKER_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_USB_BROKER_BINARY)" $(TARGET_DIR)/usr/bin/redrob-usb-broker
endef

define REDROB_USB_BROKER_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-usb-broker.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-usb-broker.service
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants
	ln -sf ../redrob-usb-broker.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-usb-broker.service
endef

$(eval $(generic-package))
