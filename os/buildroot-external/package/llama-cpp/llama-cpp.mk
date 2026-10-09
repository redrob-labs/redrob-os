################################################################################
#
# llama-cpp
#
################################################################################

LLAMA_CPP_VERSION = b4406
LLAMA_CPP_SITE = $(call github,ggml-org,llama.cpp,$(LLAMA_CPP_VERSION))
LLAMA_CPP_LICENSE = MIT
LLAMA_CPP_LICENSE_FILES = LICENSE

# CPU-only server. No libcurl (model is provisioned locally, never fetched by
# the server) and no OpenMP (llama.cpp uses its own thread pool); both keep the
# device dependency surface minimal. GGML_NATIVE=OFF so the cross toolchain's
# own -march is honoured instead of the build host's.
LLAMA_CPP_CONF_OPTS = \
	-DLLAMA_CURL=OFF \
	-DLLAMA_BUILD_SERVER=ON \
	-DLLAMA_BUILD_TESTS=OFF \
	-DLLAMA_BUILD_EXAMPLES=ON \
	-DGGML_NATIVE=OFF \
	-DGGML_OPENMP=OFF \
	-DBUILD_SHARED_LIBS=ON

# Headers and import libs are build-time only; the target keeps the server
# binary and the ggml/llama shared libraries. The upstream install also drops
# a dozen extra CLI tools (llama-cli, llama-bench, ...) the device never runs,
# so trim the target to llama-server alone to cut size and attack surface.
define LLAMA_CPP_REMOVE_DEV_FILES
	rm -rf $(TARGET_DIR)/usr/include/ggml* $(TARGET_DIR)/usr/include/llama* \
		$(TARGET_DIR)/usr/lib/cmake/llama $(TARGET_DIR)/usr/lib/pkgconfig/llama.pc
	find $(TARGET_DIR)/usr/bin -maxdepth 1 -name 'llama-*' ! -name 'llama-server' \
		-type f -delete
	rm -f $(TARGET_DIR)/usr/bin/convert_hf_to_gguf.py $(TARGET_DIR)/usr/bin/test-*
endef
LLAMA_CPP_POST_INSTALL_TARGET_HOOKS += LLAMA_CPP_REMOVE_DEV_FILES

$(eval $(cmake-package))
