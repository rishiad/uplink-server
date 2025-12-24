# Build VSCode server with GLIBC 2.35 for broad compatibility
# Works on: Amazon Linux 2, AL2023, Ubuntu 22.04+, Debian 11+
FROM ubuntu:22.04

ARG TARGETARCH
ARG VSCODE_VERSION=1.107.0

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    git tar gzip build-essential python3 curl ca-certificates \
    libx11-dev libxkbfile-dev libsecret-1-dev libkrb5-dev \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Download official VSCode to extract vsda module (pinned version for reproducibility)
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        curl -L "https://update.code.visualstudio.com/${VSCODE_VERSION}/linux-arm64/stable" -o /tmp/vscode.tar.gz; \
    else \
        curl -L "https://update.code.visualstudio.com/${VSCODE_VERSION}/linux-x64/stable" -o /tmp/vscode.tar.gz; \
    fi && \
    mkdir -p /tmp/vscode && \
    tar -xzf /tmp/vscode.tar.gz -C /tmp/vscode --strip-components=1 && \
    mkdir -p /vsda && \
    if [ ! -d "/tmp/vscode/resources/app/node_modules/vsda" ]; then \
        echo "Error: vsda module not found in VSCode ${VSCODE_VERSION}" && exit 1; \
    fi && \
    cp -r /tmp/vscode/resources/app/node_modules/vsda /vsda/ && \
    rm -rf /tmp/vscode /tmp/vscode.tar.gz

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

# Copy vsda module into the built server
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        mkdir -p ../vscode-server-linux-arm64/node_modules && \
        cp -r /vsda/vsda ../vscode-server-linux-arm64/node_modules/; \
    else \
        mkdir -p ../vscode-server-linux-x64/node_modules && \
        cp -r /vsda/vsda ../vscode-server-linux-x64/node_modules/; \
    fi

# Package the server
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cd ../vscode-server-linux-arm64 && tar czf /server.tar.gz .; \
    else \
        cd ../vscode-server-linux-x64 && tar czf /server.tar.gz .; \
    fi

FROM scratch
COPY --from=0 /server.tar.gz /
