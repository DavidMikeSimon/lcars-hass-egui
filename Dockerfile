# Build Stage

FROM --platform=$BUILDPLATFORM rust:1.72.0 AS builder
ARG TARGETARCH
WORKDIR /root/workdir

RUN case "$TARGETARCH" in \
    "386") \
        export RUST_TARGET="i686-unknown-linux-gnu" GCC_PKG="gcc-aarch64-linux-gnu" \
    ;; \
    "amd64") \
        export RUST_TARGET="x86_64-unknown-linux-gnu" GCC_PKG="gcc-x86-64-linux-gnu" \
    ;; \
    "arm64") \
        export RUST_TARGET="aarch64-unknown-linux-gnu" GCC_PKG="gcc-aarch64-linux-gnu" \
    ;; \
    *) \
        echo "Doesn't support $TARGETARCH architecture" \
        exit 1 \
    ;; \
    esac \
    && apt-get update \
    && apt-get install -y $GCC_PKG \
    && apt-get clean \
    && echo $RUST_TARGET > /root/rust_target

RUN rustup target add "$(cat /root/rust_target)"

COPY Cargo.toml Cargo.lock .
COPY .cargo ./.cargo
RUN \
    mkdir /root/workdir/src && \
    echo 'fn main() {}' > /root/workdir/src/main.rs && \
    cargo build --release --target "$(cat /root/rust_target)" && \
    rm -Rvf /root/workdir/src

COPY assets ./assets
COPY src ./src
RUN cargo build --release --target "$(cat /root/rust_target)"

# Bundle Stage

FROM ubuntu:23.04
RUN apt-get update \
  && apt-get install -y \
  libx11-6 libxcursor1 libxrandr2 libxi6 libx11-xcb1 libgl1 \
  && apt-get clean
COPY --from=builder /root/workdir/target/*/release/lcars_hass_egui .
CMD ["./lcars_hass_egui"]
