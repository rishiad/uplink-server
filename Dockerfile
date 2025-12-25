# Stage 1: Build Rust binary on older GLIBC for compatibility
FROM amazonlinux:2 AS rust-builder

RUN yum install -y gcc make tar gzip && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    yum clean all

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
COPY crates /workspace/crates
COPY Cargo.toml Cargo.lock /workspace/
RUN cargo build --release --package uplink-pty

# Stage 2: Build VSCode server
FROM ubuntu:22.04

ARG TARGETARCH
ARG VSCODE_VERSION=1.107.0

ENV DEBIAN_FRONTEND=noninteractive

# Install build dependencies
RUN apt-get update && apt-get install -y \
    git tar gzip build-essential python3 curl ca-certificates \
    libx11-dev libxkbfile-dev libsecret-1-dev libkrb5-dev \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Copy Rust binary from first stage
COPY --from=rust-builder /workspace/target/release/uplink-pty /workspace/uplink-pty

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

# Copy VSCode server source
COPY vscode-server /workspace/vscode

WORKDIR /workspace/vscode

# Install dependencies and build
RUN npm ci

# Build server based on architecture (lowmem build for reduced memory usage)
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        npm run gulp vscode-server-linux-arm64-lowmem; \
    else \
        npm run gulp vscode-server-linux-x64-lowmem; \
    fi

# Copy uplink-pty binary and vsda module into the built server
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        mkdir -p ../vscode-server-linux-arm64/node_modules && \
        mkdir -p ../vscode-server-linux-arm64/bin && \
        cp -r /vsda/vsda ../vscode-server-linux-arm64/node_modules/ && \
        cp /workspace/uplink-pty ../vscode-server-linux-arm64/bin/; \
    else \
        mkdir -p ../vscode-server-linux-x64/node_modules && \
        mkdir -p ../vscode-server-linux-x64/bin && \
        cp -r /vsda/vsda ../vscode-server-linux-x64/node_modules/ && \
        cp /workspace/uplink-pty ../vscode-server-linux-x64/bin/; \
    fi

# Package the server
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        cd ../vscode-server-linux-arm64 && tar czf /server.tar.gz .; \
    else \
        cd ../vscode-server-linux-x64 && tar czf /server.tar.gz .; \
    fi

FROM scratch
COPY --from=1 /server.tar.gz /
