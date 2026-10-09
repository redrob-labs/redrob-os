################################################################################
#
# redrob-local-inference
#
################################################################################

REDROB_LOCAL_INFERENCE_VERSION = local
REDROB_LOCAL_INFERENCE_SITE = $(BR2_EXTERNAL_HAOS_PATH)/../../deploy
REDROB_LOCAL_INFERENCE_SITE_METHOD = local
REDROB_LOCAL_INFERENCE_LICENSE = Apache-2.0
REDROB_LOCAL_INFERENCE_LICENSE_FILES = ../../LICENSE
REDROB_LOCAL_INFERENCE_DEPENDENCIES = redrob-agent llama-cpp
REDROB_LOCAL_INFERENCE_BINARY = $(call qstrip,$(BR2_PACKAGE_REDROB_LOCAL_INFERENCE_BINARY))

define REDROB_LOCAL_INFERENCE_BUILD_CMDS
	@test -x "$(REDROB_LOCAL_INFERENCE_BINARY)" || \
		{ echo "redrob-local-inference: binary not found at $(REDROB_LOCAL_INFERENCE_BINARY); run 'cargo build --release' in modules/local-inference/supervisor first"; exit 1; }
endef

define REDROB_LOCAL_INFERENCE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 "$(REDROB_LOCAL_INFERENCE_BINARY)" \
		$(TARGET_DIR)/usr/bin/redrob-local-inference
	$(INSTALL) -D -m 0644 $(@D)/config/local-inference.toml \
		$(TARGET_DIR)/etc/redrob/local-inference.toml
endef

define REDROB_LOCAL_INFERENCE_INSTALL_INIT_SYSTEMD
	$(INSTALL) -D -m 0644 $(@D)/systemd/redrob-local-inference.service \
		$(TARGET_DIR)/usr/lib/systemd/system/redrob-local-inference.service
	mkdir -p $(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants
	ln -sf ../redrob-local-inference.service \
		$(TARGET_DIR)/usr/lib/systemd/system/multi-user.target.wants/redrob-local-inference.service
endef

# redrob-infer owns nothing writable: it only reads the GGUF under
# /mnt/data/redrob/models. No supplementary groups.
define REDROB_LOCAL_INFERENCE_USERS
	redrob-infer -1 redrob-infer -1 * /mnt/data/redrob/models /bin/false - Redrob local-inference module
endef

$(eval $(generic-package))
