ARG ALPINE_VERSION=3.23
ARG RUST_VERSION=1.94.0
ARG CHEF_VERSION=0.1.77
ARG FFMPEG_VERSION=8.1

############################### FFmpeg Phase ###############################
FROM alpine:${ALPINE_VERSION} AS ffmpeg_builder
ARG FFMPEG_VERSION
WORKDIR /build

RUN apk add --no-cache build-base curl tar xz nasm

RUN curl -L https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz | tar -xJ

WORKDIR /build/ffmpeg-${FFMPEG_VERSION}
RUN ./configure \
    --disable-everything \
    --disable-network \
    --disable-autodetect \
    --disable-doc \
    --disable-htmlpages \
    --disable-manpages \
    --disable-podpages \
    --disable-txtpages \
    --disable-debug \
    --disable-ffprobe \
    --disable-shared \
    --disable-pic \
    \
    --enable-small \
    --enable-static \
    --enable-protocol=file,pipe \
    --enable-demuxer=mov,matroska \
    --enable-parser=h264,hevc,vp8,vp9,av1 \
    --enable-decoder=h264,hevc,vp8,vp9,av1 \
    --enable-filter=scale,format,null,copy,thumbnail \
    --enable-encoder=rawvideo \
    --enable-muxer=rawvideo \
    --enable-swscale \
    \
    --extra-ldflags="-static" \
    --extra-ldexeflags="-static -no-pie" \
    --pkg-config-flags="--static" \
  && make -j$(nproc) \
  && strip ffmpeg

# Verify ffmpeg has no dynamic dependencies
RUN readelf -d ffmpeg | grep -q "NEEDED" \
  && (echo "ffmpeg has dynamic dependencies!" && readelf -d ffmpeg | grep NEEDED && exit 1) \
  || echo "ffmpeg is fully static"

############################## Planning Phase ##############################
FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION}-alpine${ALPINE_VERSION} AS chef
WORKDIR /opt/app

# Create recipe of crate dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

################################ Build Phase ###############################
FROM chef AS builder
ENV DOCKER_DEPLOYMENT=1

RUN mkdir -p /data

# Install system dependencies
RUN apk add --no-cache build-base perl

COPY --from=planner /opt/app/recipe.json recipe.json

# Copy local dependencies for cargo chef cook
COPY macros/Cargo.toml macros/Cargo.toml
COPY macros/src macros/src

# Build dependencies (this will be cached for future builds)
ARG TARGET_CPU
ARG CODEGEN_OPTIONS="-C target-cpu=$TARGET_CPU"
RUN RUSTFLAGS=${CODEGEN_OPTIONS} cargo chef cook --release --recipe-path recipe.json

# Buid application
COPY . .
RUN RUSTFLAGS=${CODEGEN_OPTIONS} cargo build --release

# Verify the Rust binary has no dynamic dependencies
RUN readelf -d /opt/app/target/release/oxibooru_server | grep -q "NEEDED" \
  && (echo "Binary has dynamic dependencies!" && readelf -d /opt/app/target/release/oxibooru_server | grep NEEDED && exit 1) \
  || echo "Binary is fully static"

################################# Run Phase ################################
FROM scratch AS runtime
ARG PUID=1000
ARG PGID=1000
ARG FFMPEG_VERSION
WORKDIR /opt/app
ENV RUST_BACKTRACE=full


COPY --from=builder --chown=${PUID}:${PGID} /data /data

# Copy SSL certs for outbound requests
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# Copy binaries with ownership
COPY --from=ffmpeg_builder --chown=${PUID}:${PGID} /build/ffmpeg-${FFMPEG_VERSION}/ffmpeg /opt/app/ffmpeg
COPY --from=builder --chown=${PUID}:${PGID} /opt/app/target/release/oxibooru_server /opt/app/server

# Run it!
USER ${PUID}:${PGID}
ENTRYPOINT ["/opt/app/server"]

ENV SERVER_PORT=6666
EXPOSE ${SERVER_PORT}

VOLUME ["/data/"]

ARG DOCKER_REPO
ARG BUILD_DATE
ARG SOURCE_COMMIT
LABEL maintainer="" \
  org.opencontainers.image.title="${DOCKER_REPO}" \
  org.opencontainers.image.url="https://github.com/liamw1/oxibooru" \
  org.opencontainers.image.documentation="https://github.com/liamw1/oxibooru/blob/${SOURCE_COMMIT}/docs/INSTALL.md" \
  org.opencontainers.image.created="${BUILD_DATE}" \
  org.opencontainers.image.source="https://github.com/liamw1/oxibooru" \
  org.opencontainers.image.revision="${SOURCE_COMMIT}" \
  org.opencontainers.image.licenses="GPL-3.0"