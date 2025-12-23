#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE_NAME="remotedev-sidecar-builder:arm64"
DOCKERFILE="${ROOT_DIR}/server/Dockerfile.sidecar"

docker build --platform linux/arm64 -t "${IMAGE_NAME}" -f "${DOCKERFILE}" "${ROOT_DIR}"

docker run --rm --platform linux/arm64 \
  -v "${ROOT_DIR}":/workspace \
  -w /workspace \
  "${IMAGE_NAME}" \
  bash -lc '
    set -euo pipefail
    cd /workspace/uplink-server/sidecar
    npm install
    npm run gulp vscode-server-linux-arm64-lowmem
    cd /workspace/uplink-server
    cargo build --release --bin openvscode-server --bin sidecar-packager
    ./target/release/sidecar-packager \
      --build-dir ./vscode-server-linux-arm64 \
      --out /workspace/uplink-vscode-extention/resources/sidecar/sidecar.tar.gz
  '
