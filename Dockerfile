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

USER    cloudpub:cloudpub

RUN     cargo install cargo-chef

RUN     rustup target add arm-unknown-linux-musleabi
RUN     rustup target add armv5te-unknown-linux-musleabi
RUN     rustup target add aarch64-unknown-linux-musl
RUN     rustup target add x86_64-pc-windows-gnu

##########################################
FROM    dev AS planner
COPY    --chown=cloudpub:cloudpub . $HOME

WORKDIR $HOME
RUN     cargo chef prepare --recipe-path recipe.json

##########################################
FROM    dev AS builder
COPY    --from=planner $HOME/recipe.json $HOME/recipe.json

ENV     CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc
ENV     CARGO_TARGET_ARM_UNKNOWN_LINUX_MUSLEABI_LINKER=/usr/bin/arm-linux-gnueabi-gcc
ENV     CARGO_TARGET_ARMV5TE_UNKNOWN_LINUX_MUSLEABI_LINKER=/usr/bin/arm-linux-gnueabi-gcc
ENV     CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=/usr/bin/aarch64-linux-gnu-gcc
ENV     CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=/usr/bin/x86_64-w64-mingw32-gcc

WORKDIR $HOME

RUN     cargo chef cook --bin client --profile minimal --target x86_64-unknown-linux-gnu --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin client --profile minimal --target arm-unknown-linux-musleabi --no-default-features --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin client --profile minimal --target armv5te-unknown-linux-musleabi --no-default-features --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin client --profile minimal --target aarch64-unknown-linux-musl --no-default-features --recipe-path $HOME/recipe.json
RUN     cargo chef cook --bin client --profile minimal --target x86_64-pc-windows-gnu --recipe-path $HOME/recipe.json

COPY    --chown=cloudpub:cloudpub . $HOME
USER    cloudpub:cloudpub
ENV     PATH="$PATH:$HOME/bin"

# Build client for all targets and create artifacts
RUN     mkdir -p artifacts/win64 && \
        cargo build -p client --target x86_64-pc-windows-gnu --profile minimal && \
        cp target/x86_64-pc-windows-gnu/minimal/client.exe artifacts/win64/clo.exe

RUN     mkdir -p artifacts/x86_64 && \
        cargo build -p client --target x86_64-unknown-linux-gnu --profile minimal && \
        cp target/x86_64-unknown-linux-gnu/minimal/client artifacts/x86_64/clo

RUN     mkdir -p artifacts/aarch64 && \
        cargo build -p client --target aarch64-unknown-linux-musl --profile minimal --no-default-features && \
        cp target/aarch64-unknown-linux-musl/minimal/client artifacts/aarch64/clo

RUN     mkdir -p artifacts/arm && \
        cargo build -p client --target arm-unknown-linux-musleabi --profile minimal --no-default-features && \
        cp target/arm-unknown-linux-musleabi/minimal/client artifacts/arm/clo

RUN     mkdir -p artifacts/armv5te && \
        cargo build -p client --target armv5te-unknown-linux-musleabi --profile minimal --no-default-features && \
        cp target/armv5te-unknown-linux-musleabi/minimal/client artifacts/armv5te/clo


FROM scratch AS artifacts
COPY --from=builder /home/cloudpub/artifacts /artifacts
