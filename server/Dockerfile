ARG DEBIAN_VERSION=12.8
ARG RUST_VERSION=1.83
ARG CHEF_VERSION=0.1.68

############################## Planning Phase ##############################
FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION}-slim-bookworm AS chef
WORKDIR /opt/app

# Create recipe of crate dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

################################ Build Phase ###############################
FROM chef AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y clang pkg-config libssl-dev libpq-dev \
  libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev
RUN cargo install diesel_cli --no-default-features --features postgres

# Run migrations
ARG POSTGRES_USER
ARG POSTGRES_PASSWORD
ARG POSTGRES_PORT
ARG POSTGRES_DB
COPY ./migrations ./migrations
RUN diesel migration run --database-url=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@host.docker.internal:${POSTGRES_PORT}/${POSTGRES_DB}

# Build dependencies (this will be cached for future builds)
COPY --from=planner /opt/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Buid application
COPY . .
RUN cargo build --release

################################# Run Phase ################################
FROM debian:${DEBIAN_VERSION}-slim AS runtime
WORKDIR /opt/app

ENV RUST_BACKTRACE=full
ENV DOCKER_DEPLOYMENT=1

ARG PUID=1000
ARG PGID=1000

# Install dynamic libs needed at runtime
RUN apt-get update && apt-get install -y libpq5 libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev

# Set permissions
RUN mkdir -p /opt/app /data \
  && groupadd -g ${PGID} app \
  && useradd -u ${PUID} -g ${PGID} -ms /bin/bash app \
  && chown -R app:app /opt/app /data

# Copy over application binary
COPY --from=builder /opt/app/target/release/server server

# Run it!
USER app
ENTRYPOINT ["/opt/app/server"]

ARG PORT=6666
ENV PORT=${PORT}
EXPOSE ${PORT}

VOLUME ["/data/"]

LABEL \
    maintainer="" \
    org.opencontainers.image.title="oxibooru/server" \
    org.opencontainers.image.url="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.source="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.licenses="GPL-3.0"