ARG DEBIAN_VERSION=12.8
ARG RUST_VERSION=1.83
ARG CHEF_VERSION=0.1.68


FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION}-slim-bookworm AS chef
WORKDIR /opt/app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder
RUN apt-get update && apt-get install -y clang pkg-config libssl-dev libpq-dev \
  libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev
COPY --from=planner /opt/app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release


FROM debian:${DEBIAN_VERSION}-slim AS runtime
WORKDIR /opt/app

ENV RUST_BACKTRACE=full
ENV DOCKER_DEPLOYMENT=1

ARG PUID=1000
ARG PGID=1000

# Dynamic libs needed at runtime
RUN apt-get update && apt-get install -y libpq5 libavcodec-dev libavformat-dev libavutil-dev libavfilter-dev libavdevice-dev

RUN mkdir -p /opt/app /data \
  && groupadd -g ${PGID} app \
  && useradd -u ${PUID} -g ${PGID} -ms /bin/bash app \
  && chown -R app:app /opt/app /data

COPY --from=builder /opt/app/config.toml config.toml
COPY --from=builder /opt/app/target/release/server server

USER app
CMD ["/opt/app/server"]

ARG PORT=6666
ENV PORT=${PORT}
EXPOSE ${PORT}

VOLUME ["/data/"]