# Build VSCode server with GLIBC 2.35 for broad compatibility
# Works on: Amazon Linux 2, AL2023, Ubuntu 22.04+, Debian 11+
FROM ubuntu:22.04

ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    git tar gzip build-essential python3 curl ca-certificates \
    libx11-dev libxkbfile-dev libsecret-1-dev libkrb5-dev \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Copy custom sidecar
COPY sidecar /workspace/vscode

WORKDIR /workspace/vscode

# Install dependencies and build
RUN npm ci

# Build server based on architecture (lowmem build for reduced memory usage)
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        npm run gulp vscode-server-linux-arm64-lowmem; \
    else \
        npm run gulp vscode-server-linux-x64-lowmem; \
    fi

# Package the server
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cd ../vscode-server-linux-arm64 && tar czf /server.tar.gz .; \
    else \
        cd ../vscode-server-linux-x64 && tar czf /server.tar.gz .; \
    fi

FROM scratch
COPY --from=0 /server.tar.gz /
