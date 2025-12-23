#!/bin/bash
set -e

VSCODE_VERSION="${VSCODE_VERSION:-1.103.2}"
OUTPUT_DIR="${OUTPUT_DIR:-./dist}"

echo "Building VSCode servers for version ${VSCODE_VERSION}"
mkdir -p "${OUTPUT_DIR}"

# Build GLIBC 2.27 version for x64
echo "Building GLIBC 2.27 x64..."
docker buildx build \
  --platform linux/amd64 \
  --build-arg VSCODE_VERSION="${VSCODE_VERSION}" \
  -f Dockerfile.glibc227 \
  -t uplink-server:glibc227-x64 \
  --output type=local,dest="${OUTPUT_DIR}/tmp-x64" \
  .

mv "${OUTPUT_DIR}/tmp-x64/server.tar.gz" \
   "${OUTPUT_DIR}/server-linux-x64-glibc227-${VSCODE_VERSION}.tar.gz"
rm -rf "${OUTPUT_DIR}/tmp-x64"

# Build GLIBC 2.27 version for arm64
echo "Building GLIBC 2.27 arm64..."
docker buildx build \
  --platform linux/arm64 \
  --build-arg VSCODE_VERSION="${VSCODE_VERSION}" \
  -f Dockerfile.glibc227 \
  -t uplink-server:glibc227-arm64 \
  --output type=local,dest="${OUTPUT_DIR}/tmp-arm64" \
  .

mv "${OUTPUT_DIR}/tmp-arm64/server.tar.gz" \
   "${OUTPUT_DIR}/server-linux-arm64-glibc227-${VSCODE_VERSION}.tar.gz"
rm -rf "${OUTPUT_DIR}/tmp-arm64"

# Build MUSL static version for x64
echo "Building MUSL static x64..."
docker buildx build \
  --platform linux/amd64 \
  --build-arg VSCODE_VERSION="${VSCODE_VERSION}" \
  -f Dockerfile.musl \
  -t uplink-server:musl-x64 \
  --output type=local,dest="${OUTPUT_DIR}/tmp-musl-x64" \
  .

mv "${OUTPUT_DIR}/tmp-musl-x64/server.tar.gz" \
   "${OUTPUT_DIR}/server-linux-x64-musl-${VSCODE_VERSION}.tar.gz"
rm -rf "${OUTPUT_DIR}/tmp-musl-x64"

# Build MUSL static version for arm64
echo "Building MUSL static arm64..."
docker buildx build \
  --platform linux/arm64 \
  --build-arg VSCODE_VERSION="${VSCODE_VERSION}" \
  -f Dockerfile.musl \
  -t uplink-server:musl-arm64 \
  --output type=local,dest="${OUTPUT_DIR}/tmp-musl-arm64" \
  .

mv "${OUTPUT_DIR}/tmp-musl-arm64/server.tar.gz" \
   "${OUTPUT_DIR}/server-linux-arm64-musl-${VSCODE_VERSION}.tar.gz"
rm -rf "${OUTPUT_DIR}/tmp-musl-arm64"

echo "Build complete! Artifacts in ${OUTPUT_DIR}:"
ls -lh "${OUTPUT_DIR}"/*.tar.gz
