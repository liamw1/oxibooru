ARG ALPINE_VERSION=3.22
ARG RUST_VERSION=1.91.1
ARG CHEF_VERSION=0.1.73

############################## Planning Phase ##############################
FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION}-alpine AS chef
WORKDIR /opt/app

# Create recipe of crate dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

################################ Build Phase ###############################
FROM chef AS builder

ENV DOCKER_DEPLOYMENT=1

# Install system dependencies
RUN apk update && apk add --no-cache pkgconfig clang-dev postgresql-dev dav1d-dev ffmpeg-dev

COPY --from=planner /opt/app/recipe.json recipe.json

# Build dependencies (this will be cached for future builds)
ARG TARGET_CPU
ARG CODEGEN_OPTIONS="-C target-feature=-crt-static -C target-cpu=$TARGET_CPU"
RUN RUSTFLAGS=${CODEGEN_OPTIONS} cargo chef cook --release --recipe-path recipe.json

# Buid application
COPY . .
RUN RUSTFLAGS=${CODEGEN_OPTIONS} cargo build --release

################################# Run Phase ################################
FROM alpine:${ALPINE_VERSION} AS runtime
WORKDIR /opt/app

ENV RUST_BACKTRACE=full

ARG PUID=1000
ARG PGID=1000

# Install dynamic libs needed at runtime
RUN apk update && apk add --no-cache libpq ffmpeg-libavdevice

# Set permissions
RUN mkdir -p /opt/app /data \
 && addgroup -g ${PGID} app \
 && adduser -SDH -h /opt/app -g '' -G app -u ${PUID} app \
 && chown -R app:app /opt/app /data

# Copy over application binary
COPY --from=builder /opt/app/target/release/server server

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