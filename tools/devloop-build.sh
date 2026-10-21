#!/usr/bin/bash
# devloop-build.sh VARIANT -- dev loop step 1: build the OS image locally through the
# host's root podman socket and push it to the loopback registry the VM pulls from.
#   VARIANT = cosmic | kde
# Logs: target/devloop/build-<variant>.log ; done marker: target/devloop/<variant>.done
set -euo pipefail
cd "$(dirname "$0")/.."
export CONTAINER_HOST=unix:///run/podman/podman.sock
v="${1:?cosmic|kde}"
case "$v" in
  cosmic) env=image/base.cosmic.env; cf=image/Containerfile.cosmic ;;
  kde)    env=image/base.env;        cf=image/Containerfile ;;
  *) echo "unknown variant $v" >&2; exit 2 ;;
esac
args=()
while IFS= read -r line; do
  case "$line" in ''|'#'*) continue ;; esac
  args+=(--build-arg "$line")
done < "$env"
mkdir -p target/devloop
rm -f "target/devloop/$v.done"
tag="localhost:5000/redrob-os:$v"
start=$(date +%s)
# 14 cores max on this shared host (user rule).
podman --remote build --cpuset-cpus 0-13 "${args[@]}" \
  --build-arg VERSION="dev-$v" --build-arg GIT_SHA="$(git rev-parse --short HEAD)" \
  -f "$cf" -t "$tag" .
podman --remote push --tls-verify=false "$tag"
echo "build+push $v: $(( $(date +%s) - start ))s" | tee "target/devloop/$v.done"
