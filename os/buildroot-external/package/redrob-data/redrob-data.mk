################################################################################
#
# redrob-data
#
################################################################################

REDROB_DATA_VERSION = 1.0.0
REDROB_DATA_LICENSE = Apache-2.0
REDROB_DATA_SITE = $(BR2_EXTERNAL_HAOS_PATH)/package/redrob-data
REDROB_DATA_SITE_METHOD = local
REDROB_DATA_DEPENDENCIES = host-e2fsprogs
REDROB_DATA_INSTALL_IMAGES = YES

define REDROB_DATA_INSTALL_IMAGES_CMDS
	$(BR2_EXTERNAL_HAOS_PATH)/package/redrob-data/create-data-partition.sh \
		"$(@D)" "$(BINARIES_DIR)" "$(HOST_DIR)"
endef

$(eval $(generic-package))
