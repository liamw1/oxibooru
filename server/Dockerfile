ARG DEBIAN_VERSION=13.3
ARG RUST_VERSION=1.91.1
ARG CHEF_VERSION=0.1.73

############################## Planning Phase ##############################
FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION}-slim-trixie AS chef
WORKDIR /opt/app

# Create recipe of crate dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

################################ Build Phase ###############################
FROM chef AS builder

ENV DOCKER_DEPLOYMENT=1

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    pkg-config \
    libpq-dev \
    libdav1d-dev \
    libavcodec-dev \
    libavformat-dev \
    libavutil-dev \
    libavfilter-dev \
    libavdevice-dev \
    && rm -rf /var/lib/apt/lists/*

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

################################# Run Phase ################################
FROM debian:${DEBIAN_VERSION}-slim AS runtime
WORKDIR /opt/app

ENV RUST_BACKTRACE=full

ARG PUID=1000
ARG PGID=1000

# Install dynamic libs needed at runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq5 \
    libdav1d7 \
    libavcodec61 \
    libavformat61 \
    libavutil59 \
    libavfilter10 \
    libavdevice61 \
    && rm -rf /var/lib/apt/lists/*

# Set permissions
RUN mkdir -p /opt/app /data \
    && groupadd -g ${PGID} app \
    && useradd -u ${PUID} -g ${PGID} -r -M -d /opt/app -s /sbin/nologin app \
    && chown -R app:app /opt/app /data

# Copy over application binary and bundled dependencies
COPY --from=builder /opt/app/target/release/oxibooru_server server
COPY --from=builder /opt/app/target/release/libwebgpu_dawn.so /usr/lib/libwebgpu_dawn.so

# Run it!
USER app
ENTRYPOINT ["/opt/app/server"]

ARG SERVER_PORT=6666
ENV SERVER_PORT=${SERVER_PORT}
EXPOSE ${SERVER_PORT}

VOLUME ["/data/"]

ARG DOCKER_REPO
ARG BUILD_DATE
ARG SOURCE_COMMIT
LABEL \
    maintainer="" \
    org.opencontainers.image.title="${DOCKER_REPO}" \
    org.opencontainers.image.url="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.documentation="https://github.com/liamw1/oxibooru/blob/${SOURCE_COMMIT}/docs/INSTALL.md" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.source="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.revision="${SOURCE_COMMIT}" \
    org.opencontainers.image.licenses="GPL-3.0"