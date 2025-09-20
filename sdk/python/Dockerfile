FROM ghcr.io/pyo3/maturin:latest

RUN apt-get update && apt-get install -y \
    curl \
    wget \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl -L https://github.com/ziglang/zig/releases/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz | tar -xJ \
    && mv zig-linux-x86_64-0.13.0 /opt/zig \
    && ln -s /opt/zig/zig /usr/local/bin/zig

RUN rustup target add \
    x86_64-unknown-linux-gnu \
    x86_64-unknown-linux-musl \
    aarch64-unknown-linux-gnu \
    aarch64-unknown-linux-musl \
    x86_64-pc-windows-gnu \
    x86_64-apple-darwin \
    aarch64-apple-darwin

RUN pip install --upgrade pip setuptools wheel ziglang

RUN cargo install cargo-zigbuild

ENV CARGO_ZIGBUILD_ZIG_PATH=/usr/local/bin/zig

WORKDIR /workspace

COPY . /workspace/

COPY sdk/python/Cargo.toml /workspace/sdk/python/
COPY sdk/python/pyproject.toml /workspace/sdk/python/
COPY sdk/python/src /workspace/sdk/python/src
COPY sdk/python/*.py /workspace/sdk/python/

COPY sdk/rust /workspace/sdk/rust
COPY common /workspace/common
COPY Cargo.toml /workspace/

WORKDIR /workspace/sdk/python

ARG PYTHON_VERSIONS="3.11 3.12 3.13 3.14"
ARG TARGET_PLATFORMS="linux_x86_64 linux_aarch64 macos_x86_64 macos_aarch64 windows_x86_64"

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/workspace/target \
    mkdir -p /output && \
    for py_version in ${PYTHON_VERSIONS}; do \
        py_major=$(echo $py_version | cut -d. -f1); \
        py_minor=$(echo $py_version | cut -d. -f2); \
        py_tag="cp${py_major}${py_minor}"; \
        echo "Building for Python ${py_version} (${py_tag})..."; \
        \
        # Linux x86_64 (glibc)
        if echo "${TARGET_PLATFORMS}" | grep -q "linux_x86_64"; then \
            echo "Building Linux x86_64 wheel for Python ${py_version}..."; \
            maturin build --release \
                --target x86_64-unknown-linux-gnu \
                --compatibility manylinux2014 \
                --interpreter python${py_version} \
                --zig \
                --out /output || echo "Failed to build for Linux x86_64 Python ${py_version}"; \
        fi; \
        \
        # Linux aarch64 (ARM64)
        if echo "${TARGET_PLATFORMS}" | grep -q "linux_aarch64"; then \
            echo "Building Linux aarch64 wheel for Python ${py_version}..."; \
            maturin build --release \
                --target aarch64-unknown-linux-gnu \
                --compatibility manylinux2014 \
                --interpreter python${py_version} \
                --zig \
                --out /output || echo "Failed to build for Linux aarch64 Python ${py_version}"; \
        fi; \
        \
        # macOS x86_64
        if echo "${TARGET_PLATFORMS}" | grep -q "macos_x86_64"; then \
            echo "Building macOS x86_64 wheel for Python ${py_version}..."; \
            maturin build --release \
                --target x86_64-apple-darwin \
                --interpreter python${py_version} \
                --zig \
                --out /output || echo "Failed to build for macOS x86_64 Python ${py_version}"; \
        fi; \
        \
        # macOS aarch64 (Apple Silicon)
        if echo "${TARGET_PLATFORMS}" | grep -q "macos_aarch64"; then \
            echo "Building macOS aarch64 wheel for Python ${py_version}..."; \
            maturin build --release \
                --target aarch64-apple-darwin \
                --interpreter python${py_version} \
                --zig \
                --out /output || echo "Failed to build for macOS aarch64 Python ${py_version}"; \
        fi; \
        \
        # Windows x86_64
        if echo "${TARGET_PLATFORMS}" | grep -q "windows_x86_64"; then \
            echo "Building Windows x86_64 wheel for Python ${py_version}..."; \
            maturin build --release \
                --target x86_64-pc-windows-gnu \
                --interpreter python${py_version} \
                --zig \
                --out /output || echo "Failed to build for Windows x86_64 Python ${py_version}"; \
        fi; \
    done

RUN ls -la /output/