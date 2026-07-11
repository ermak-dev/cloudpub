# Run with:
#
# docker build --target artifacts --output type=local,dest=. .
#
# Take the artifacts from the /artifacts directory

##########################################
# Base stage with all build dependencies
##########################################
FROM debian:bookworm-slim AS base

# Install base dependencies
RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    gcc \
    g++ \
    make \
    pkg-config \
    libssl-dev \
    xz-utils \
    unzip \
    python3 \
    python3-pip \
    python3-venv \
    # Cross-compilation tools
    gcc-mingw-w64-x86-64 \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    binutils-mingw-w64-x86-64 \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf --retry 5 --retry-all-errors --connect-timeout 20 https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup --version \
    && cargo --version \
    && rustc --version

# Add rust targets for cross-compilation
RUN rustup target add \
    x86_64-unknown-linux-gnu \
    x86_64-unknown-linux-musl \
    aarch64-unknown-linux-gnu \
    aarch64-unknown-linux-musl \
    x86_64-pc-windows-gnu

# Install Python packages including maturin (pinned: new majors change
# project layout validation and can break the build)
RUN pip3 install --no-cache-dir --break-system-packages \
    --upgrade pip setuptools wheel \
    'maturin>=1.9,<2.0' \
    ziglang

# Install cargo-zigbuild for cross-compilation
RUN cargo install cargo-zigbuild

# Install protoc
RUN wget --tries=5 --timeout=30 https://github.com/protocolbuffers/protobuf/releases/download/v21.12/protoc-21.12-linux-x86_64.zip \
    && unzip protoc-21.12-linux-x86_64.zip -d /usr/local \
    && rm protoc-21.12-linux-x86_64.zip

# Install Zig for cross-compilation
RUN wget --tries=5 --timeout=30 https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz \
    && xz -d zig-linux-x86_64-0.13.0.tar.xz \
    && tar -xf zig-linux-x86_64-0.13.0.tar \
    && mv zig-linux-x86_64-0.13.0 /opt/zig \
    && ln -s /opt/zig/zig /usr/local/bin/zig \
    && rm zig-linux-x86_64-0.13.0.tar

ENV CARGO_ZIGBUILD_ZIG_PATH=/usr/local/bin/zig

##########################################
# Builder stage - builds all wheels
##########################################
FROM base AS builder

# Build arguments with default values
ARG NEXT_PUBLIC_DOMAIN=cloudpub.ru
ARG NEXT_PUBLIC_PORT=443
ARG NEXT_PUBLIC_VERSION=2.x
ARG NEXT_PUBLIC_ONPREM=false
ARG NEXT_PUBLIC_SITE_NAME=CloudPub
ARG NEXT_PUBLIC_BRANCH=stable
ARG NEXT_PUBLIC_TRAFFIC_LIMIT=0

# Set environment variables
ENV NEXT_PUBLIC_DOMAIN=${NEXT_PUBLIC_DOMAIN}
ENV NEXT_PUBLIC_PORT=${NEXT_PUBLIC_PORT}
ENV NEXT_PUBLIC_VERSION=${NEXT_PUBLIC_VERSION}
ENV NEXT_PUBLIC_ONPREM=${NEXT_PUBLIC_ONPREM}
ENV NEXT_PUBLIC_SITE_NAME=${NEXT_PUBLIC_SITE_NAME}
ENV NEXT_PUBLIC_BRANCH=${NEXT_PUBLIC_BRANCH}
ENV NEXT_PUBLIC_TRAFFIC_LIMIT=${NEXT_PUBLIC_TRAFFIC_LIMIT}

# Create artifacts directory
RUN mkdir -p /artifacts/wheels

# Configure cross-compilation environment
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=/usr/bin/x86_64-w64-mingw32-gcc
ENV RUSTFLAGS_X86_64_PC_WINDOWS_GNU="-C target-feature=+crt-static -C link-arg=-Wl,--subsystem,console:6.01"

# Copy sources last, after all the rarely-changing layers above.
# Only the crates needed to load the cargo workspace and build the python
# SDK: changes elsewhere in the repo (frontend, backend, docs, bin) must
# not invalidate these layers.
WORKDIR /workspace
COPY Cargo.toml Cargo.lock /workspace/
COPY common /workspace/common
COPY client /workspace/client
COPY server /workspace/server
COPY tests /workspace/tests
COPY gui /workspace/gui
COPY sdk /workspace/sdk

WORKDIR /workspace/sdk/python

# Cache mounts must match CARGO_HOME=/usr/local/cargo (set in the base
# stage), otherwise the whole crates registry is re-downloaded every build
# Build Windows x86_64 wheel
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    maturin build --release \
        --target x86_64-pc-windows-gnu \
        --out /artifacts/wheels \
    && echo "Built Windows x86_64 wheel"

# Build Linux x86_64 wheel
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    maturin build --release \
        --target x86_64-unknown-linux-gnu \
        --zig \
        --out /artifacts/wheels \
    && echo "Built Linux x86_64 wheel"

# Build Linux aarch64 wheel
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    maturin build --release \
        --target aarch64-unknown-linux-gnu \
        --zig \
        --out /artifacts/wheels \
    && echo "Built Linux aarch64 wheel"

# List all built artifacts
RUN ls -la /artifacts/

##########################################
# Final stage - export artifacts
##########################################
FROM scratch AS artifacts
COPY --from=builder /artifacts /artifacts
