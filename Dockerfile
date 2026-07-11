# Run with:
#
# docker build --target artifacts --output type=local,dest=. .
#
# Take the artifacts from the /artifacts directory

FROM    rust:1.88 AS dev
ENV     HOME="/home/cloudpub"
USER    root

RUN     mkdir -p $HOME && \
        adduser cloudpub --home $HOME --shell /bin/bash && \
        chown -R cloudpub:cloudpub $HOME

#       Base dependencies
RUN     apt-get update
RUN     apt-get install -y sudo file curl libcap2-bin libxml2 mime-support git-core

#       Support of i686 build
RUN     dpkg --add-architecture i386 && apt-get update

#       Common dependencie
RUN     apt install -y build-essential cmake

#       Install ARM toolchains
RUN     apt install -y gcc-arm-linux-gnueabi g++-arm-linux-gnueabi
RUN     apt install -y gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
RUN     apt install -y gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf
RUN     apt install -y protobuf-compiler

#       Install Windows cross-compilation tools
RUN     apt install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64

#       Install MIPS (little-endian) toolchain
RUN     apt-get update && apt install -y gcc-mipsel-linux-gnu g++-mipsel-linux-gnu libc6-dev-mipsel-cross

#       Install Android NDK for Android builds
RUN     apt install -y wget unzip && \
        wget -q --tries=5 --timeout=30 https://dl.google.com/android/repository/android-ndk-r26d-linux.zip -O /tmp/ndk.zip && \
        unzip -q /tmp/ndk.zip -d /opt && \
        rm /tmp/ndk.zip && \
        chmod -R a+rx /opt/android-ndk-r26d
ENV     ANDROID_NDK_HOME="/opt/android-ndk-r26d"
#       cmake (>=3.20) locates the NDK via ANDROID_NDK_ROOT/ANDROID_NDK, not
#       ANDROID_NDK_HOME; without these the libz-ng-sys build for the android
#       target fails with "Neither the NDK nor a standalone toolchain was found"
ENV     ANDROID_NDK_ROOT="/opt/android-ndk-r26d"
ENV     ANDROID_NDK="/opt/android-ndk-r26d"
ENV     PATH="$PATH:/opt/android-ndk-r26d/toolchains/llvm/prebuilt/linux-x86_64/bin"

#       Install zig for cargo-zigbuild: lets the x86_64/i686 glibc builds pin
#       the target glibc version (2.17) independently of this image's glibc
RUN     apt install -y xz-utils && \
        ZIG_VERSION=0.13.0 && \
        wget -q --tries=5 --timeout=30 https://ziglang.org/download/${ZIG_VERSION}/zig-linux-x86_64-${ZIG_VERSION}.tar.xz -O /tmp/zig.tar.xz && \
        tar -xf /tmp/zig.tar.xz -C /opt && rm /tmp/zig.tar.xz && \
        ln -s /opt/zig-linux-x86_64-${ZIG_VERSION}/zig /usr/local/bin/zig

USER    cloudpub:cloudpub

# --locked pins each tool's own Cargo.lock: without it, cargo resolves newer
# transitive deps that require a rustc newer than this image's (rust:1.88)
RUN     cargo install cargo-chef --locked
RUN     cargo install cargo-zigbuild --locked

# Add MIPS target using nightly toolchain as it might have more targets
RUN     rustup toolchain install nightly
RUN     rustup target add --toolchain nightly mipsel-unknown-linux-gnu || true

RUN     rustup target add i686-unknown-linux-gnu
RUN     rustup target add arm-unknown-linux-musleabi
RUN     rustup target add armv5te-unknown-linux-musleabi
RUN     rustup target add aarch64-unknown-linux-musl
RUN     rustup target add x86_64-pc-windows-gnu
RUN     rustup target add aarch64-linux-android

##########################################
FROM    dev AS planner
COPY    --chown=cloudpub:cloudpub . $HOME

WORKDIR $HOME
RUN     cargo chef prepare --recipe-path recipe.json

##########################################
FROM    dev AS builder
COPY    --from=planner $HOME/recipe.json $HOME/recipe.json
ENV     NEXT_PUBLIC_VERSION="2.4.5"

ENV     NEXT_PUBLIC_DOMAIN="cloudpub.ru"
ENV     NEXT_PUBLIC_ONPREM="false"
ENV     NEXT_PUBLIC_SITE_NAME="CloudPub"
ENV     NEXT_PUBLIC_TRAFFIC_LIMIT=0
ENV     NEXT_PUBLIC_PORT=443
ENV     NEXT_PUBLIC_BRANCH="stable"

ENV     CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc
ENV     CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_LINKER=/usr/bin/arm-linux-gnueabi-gcc
ENV     CARGO_TARGET_ARMV5TE_UNKNOWN_LINUX_MUSLEABI_LINKER=/usr/bin/arm-linux-gnueabi-gcc
ENV     CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=/usr/bin/aarch64-linux-gnu-gcc
ENV     CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=/usr/bin/x86_64-w64-mingw32-gcc
ENV     RUSTFLAGS_X86_64_PC_WINDOWS_GNU="-C target-feature=+crt-static -C link-arg=-Wl,--subsystem,console:6.01"
ENV     CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/mipsel-linux-gnu-gcc
ENV     CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android24-clang
ENV     CC_aarch64_linux_android=aarch64-linux-android24-clang
ENV     AR_aarch64_linux_android=llvm-ar

WORKDIR $HOME

RUN     cargo chef cook --bin clo --profile minimal --target x86_64-unknown-linux-gnu --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin clo --profile minimal --target arm-unknown-linux-musleabi --no-default-features --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin clo --profile minimal --target armv5te-unknown-linux-musleabi --no-default-features --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin clo --profile minimal --target aarch64-unknown-linux-musl --no-default-features --recipe-path $HOME/recipe.json
RUN     RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-Wl,--subsystem,console:6.01" \
        cargo chef cook --bin clo --profile minimal --target x86_64-pc-windows-gnu --recipe-path $HOME/recipe.json
# Try to build with nightly for MIPS (little-endian)
RUN     rustup component add rust-src --toolchain nightly && \
        RUSTFLAGS="-C target-feature=+crt-static -C linker=/usr/bin/mipsel-linux-gnu-gcc" \
        cargo +nightly chef cook \
        --bin clo \
        --profile minimal \
        --target mipsel-unknown-linux-gnu \
        --no-default-features \
        -Z build-std=std,panic_abort,core,alloc \
        --recipe-path $HOME/recipe.json

RUN     cargo chef cook --bin clo --profile minimal --target aarch64-linux-android --no-default-features --recipe-path $HOME/recipe.json

COPY    --chown=cloudpub:cloudpub . $HOME
USER    cloudpub:cloudpub
ENV     PATH="$PATH:$HOME/bin"

# Build client for all targets and create artifacts
RUN     mkdir -p artifacts/win64 && \
        RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-Wl,--subsystem,console:6.01" \
        cargo build -p cloudpub-client --target x86_64-pc-windows-gnu --profile minimal && \
        cp target/x86_64-pc-windows-gnu/minimal/clo.exe artifacts/win64/clo.exe

RUN     mkdir -p artifacts/x86_64 && \
        cargo zigbuild -p cloudpub-client --target x86_64-unknown-linux-gnu.2.17 --profile minimal && \
        cp target/x86_64-unknown-linux-gnu/minimal/clo artifacts/x86_64/clo

RUN     mkdir -p artifacts/i686 && \
        cargo zigbuild -p cloudpub-client --target i686-unknown-linux-gnu.2.17 --profile minimal && \
        cp target/i686-unknown-linux-gnu/minimal/clo artifacts/i686/clo

RUN     mkdir -p artifacts/aarch64 && \
        cargo build -p cloudpub-client --target aarch64-unknown-linux-musl --profile minimal --no-default-features && \
        cp target/aarch64-unknown-linux-musl/minimal/clo artifacts/aarch64/clo

RUN     mkdir -p artifacts/arm && \
        cargo build -p cloudpub-client --target arm-unknown-linux-musleabi --profile minimal --no-default-features && \
        cp target/arm-unknown-linux-musleabi/minimal/clo artifacts/arm/clo

RUN     mkdir -p artifacts/armv5te && \
        cargo build -p cloudpub-client --target armv5te-unknown-linux-musleabi --profile minimal --no-default-features && \
        cp target/armv5te-unknown-linux-musleabi/minimal/clo artifacts/armv5te/clo

# Build MIPS (little-endian) target with nightly toolchain and build-std
RUN     mkdir -p artifacts/mipsel && \
        rustup component add rust-src --toolchain nightly && \
        RUSTFLAGS="-C target-feature=+crt-static -C linker=/usr/bin/mipsel-linux-gnu-gcc" \
        cargo +nightly build \
        -p cloudpub-client \
        --target mipsel-unknown-linux-gnu \
        --profile minimal \
        --no-default-features \
        -Z build-std=std,panic_abort,core,alloc && \
        cp target/mipsel-unknown-linux-gnu/minimal/clo artifacts/mipsel/clo && \
        file artifacts/mipsel/clo

RUN     mkdir -p artifacts/android-arm64 && \
        cargo build -p cloudpub-client --target aarch64-linux-android --profile minimal --no-default-features && \
        cp target/aarch64-linux-android/minimal/clo artifacts/android-arm64/clo

FROM scratch AS artifacts
COPY --from=builder /home/cloudpub/artifacts /artifacts
