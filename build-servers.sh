#!/bin/bash
set -e

# Default values
VSCODE_VERSION="${VSCODE_VERSION:-1.103.2}"
OUTPUT_DIR="${OUTPUT_DIR:-./dist}"
ARCH=""

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --arch)
      ARCH="$2"
      shift 2
      ;;
    --version)
      VSCODE_VERSION="$2"
      shift 2
      ;;
    --output)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --arch <x64|arm64>        Build specific architecture (default: all)"
      echo "  --version <version>       VSCode version (default: 1.103.2)"
      echo "  --output <dir>            Output directory (default: ./dist)"
      echo "  --help, -h                Show this help message"
      echo ""
      echo "Examples:"
      echo "  $0                        # Build all architectures"
      echo "  $0 --arch arm64           # Build only arm64"
      echo "  $0 --arch x64             # Build only x64"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use --help for usage information"
      exit 1
      ;;
  esac
done

# Validate inputs
if [[ -n "$ARCH" && "$ARCH" != "x64" && "$ARCH" != "arm64" ]]; then
  echo "Error: --arch must be 'x64' or 'arm64'"
  exit 1
fi

echo "Building VSCode servers for version ${VSCODE_VERSION}"
mkdir -p "${OUTPUT_DIR}"

# Function to build a specific variant
build_variant() {
  local variant=$1
  local arch=$2
  local platform=$3
  local dockerfile=$4
  
  echo "Building ${variant} ${arch}..."
  docker buildx build \
    --platform "${platform}" \
    --build-arg VSCODE_VERSION="${VSCODE_VERSION}" \
    -f "${dockerfile}" \
    -t "uplink-server:${variant}-${arch}" \
    --output "type=local,dest=${OUTPUT_DIR}/tmp-${variant}-${arch}" \
    .
  
  mv "${OUTPUT_DIR}/tmp-${variant}-${arch}/server.tar.gz" \
     "${OUTPUT_DIR}/server-linux-${arch}-${variant}-${VSCODE_VERSION}.tar.gz"
  rm -rf "${OUTPUT_DIR}/tmp-${variant}-${arch}"
}

# Determine what to build
should_build() {
  local check_arch=$1
  
  if [[ -n "$ARCH" && "$ARCH" != "$check_arch" ]]; then
    return 1
  fi
  
  return 0
}

# Build requested variants (glibc only)
if should_build "x64"; then
  build_variant "glibc" "x64" "linux/amd64" "Dockerfile"
fi

if should_build "arm64"; then
  build_variant "glibc" "arm64" "linux/arm64" "Dockerfile"
fi

echo "Build complete! Artifacts in ${OUTPUT_DIR}:"
ls -lh "${OUTPUT_DIR}"/*.tar.gz 2>/dev/null || echo "No artifacts found"
