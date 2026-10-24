# SPDX-License-Identifier: AGPL-3.0-or-later
#
# `docker build .` builds tinystream from source, which takes a while: FFmpeg, libtorrent and the rest
# are built too. Releases use the `prebuilt` target instead, with an unpacked release archive as the
# context:
#
#   docker build -f Dockerfile --target prebuilt tinystream-<version>-<target>/

FROM debian:trixie-slim AS runtime

# libva and its drivers are only opened at runtime, for VA-API; without them transcoding is in software.
RUN set -eu; \
    drivers=mesa-va-drivers; \
    if [ "$(dpkg --print-architecture)" = amd64 ]; then \
        drivers="$drivers intel-media-va-driver i965-va-driver"; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
        libstdc++6 libgcc-s1 libssl3t64 zlib1g libbz2-1.0 liblzma5 \
        libva2 libva-drm2 $drivers; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --uid 1000 --user-group --create-home tinystream; \
    mkdir -p /config /data; \
    chown tinystream:tinystream /config /data

LABEL org.opencontainers.image.source=https://github.com/tinystream-dev/tinystream \
      org.opencontainers.image.licenses=AGPL-3.0-or-later

ENV TINYSTREAM_CONFIG=/config/config.toml \
    TINYSTREAM_DATA_DIR=/data

VOLUME ["/config", "/data"]
EXPOSE 3000/tcp 6881/tcp 6881/udp
USER tinystream
ENTRYPOINT ["tinystream"]
CMD ["serve"]

FROM runtime AS prebuilt

COPY tinystream /usr/local/bin/tinystream
COPY LICENSE THIRD_PARTY_LICENSES /usr/share/doc/tinystream/

FROM debian:trixie AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential pkg-config ca-certificates curl git tar \
        meson ninja-build autoconf automake libtool \
        libclang-dev libssl-dev libdrm-dev \
        zlib1g-dev libbz2-dev liblzma-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none

COPY --from=oven/bun:1.3 /usr/local/bin/bun /usr/local/bin/bun

WORKDIR /src
COPY rust-toolchain.toml .
RUN rustup toolchain install && rustup target add wasm32-unknown-unknown
RUN cargo install --locked wasm-bindgen-cli@0.2.126 cargo-about@0.9.2

COPY . .

ENV TINYSTREAM_NATIVE_DIR=/native

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/native \
    set -eu; \
    bun install --frozen-lockfile --filter tinystream-web --filter @tinystream/shared; \
    cargo build --release --locked; \
    mkdir -p /out; \
    cp target/release/tinystream LICENSE /out/; \
    licenses/collect.sh /native "$(rustc -vV | sed -n 's/^host: //p')" > /out/THIRD_PARTY_LICENSES

FROM runtime

COPY --from=build /out/tinystream /usr/local/bin/tinystream
COPY --from=build /out/LICENSE /out/THIRD_PARTY_LICENSES /usr/share/doc/tinystream/
