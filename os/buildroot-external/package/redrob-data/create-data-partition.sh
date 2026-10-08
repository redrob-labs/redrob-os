#!/bin/bash
# Build data.ext4 from a directory, no root, no container engine.
#   $1 build dir   $2 BINARIES_DIR   $3 HOST_DIR
set -e

build_dir=$1
dst_dir=$2
host_dir=$3
data_img="${dst_dir}/data.ext4"
data_dir="${build_dir}/data"

rm -rf "${data_dir}" "${data_img}"
mkdir -p "${data_dir}/redrob/agent" "${data_dir}/redrob/broker" \
         "${data_dir}/redrob/identity" "${data_dir}/redrob/audit" \
         "${data_dir}/redrob/models" "${data_dir}/redrob/modules"

# Marker read by redrob-firstboot: present on a fresh image, removed after
# identity generation.
touch "${data_dir}/redrob/identity/.firstboot"

# Initial size large enough for the tree; shrunk below.
truncate --size="256M" "${data_img}"
"${host_dir}/sbin/mkfs.ext4" -q -L "redrob-data" \
    -E lazy_itable_init=0,lazy_journal_init=0 \
    -d "${data_dir}" "${data_img}"

"${host_dir}/sbin/e2fsck" -f -y "${data_img}" >/dev/null
"${host_dir}/sbin/resize2fs" -M "${data_img}" >/dev/null 2>&1
block_count=$("${host_dir}/sbin/dumpe2fs" -h "${data_img}" 2>/dev/null | awk '/^Block count:/{print $3}')
block_size=$("${host_dir}/sbin/dumpe2fs" -h "${data_img}" 2>/dev/null | awk '/^Block size:/{print $3}')
truncate --size="$((block_count * block_size))" "${data_img}"
echo "redrob-data: $(du -h "${data_img}" | cut -f1) ${data_img}"
