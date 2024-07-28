ARG DEBIAN_VERSION=12.5
ARG RUST_VERSION=1.80
ARG CHEF_VERSION=0.1.67


FROM lukemathwalker/cargo-chef:${CHEF_VERSION}-rust-${RUST_VERSION} AS chef
WORKDIR /opt/app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder
COPY --from=planner /opt/app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release


FROM debian:$DEBIAN_VERSION AS release
WORKDIR /opt/app

ENV DOCKER_DEPLOYMENT=1

COPY --from=builder /opt/app/target/release/server server
COPY --from=builder /opt/app/config.toml config.toml
COPY --from=builder /opt/app/docker-start.sh docker-start.sh

ARG PUID=1000
ARG PGID=1000

RUN apt-get update && apt-get install libpq5 -y

RUN mkdir -p /opt/app /data \
 && groupadd -g ${PGID} app \
 && useradd -u ${PUID} -g ${PGID} -ms /bin/bash app \
 && chown -R app:app /opt/app /data

USER app
CMD ["/opt/app/docker-start.sh"]

ARG PORT=6666
ENV PORT=${PORT}
EXPOSE ${PORT}

VOLUME ["/data/"]