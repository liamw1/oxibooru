ARG DEBIAN_VERSION=12.5
ARG RUST_VERSION=1.79



FROM rust:$RUST_VERSION as build
WORKDIR /opt/app

COPY . .

RUN cargo build --release



FROM debian:$DEBIAN_VERSION as release
WORKDIR /opt/app

ENV DOCKER_DEPLOYMENT=1

COPY --from=build /opt/app/target/release/server server
COPY --from=build /opt/app/.env .env
COPY --from=build /opt/app/config.toml config.toml
COPY --from=build /opt/app/docker-start.sh docker-start.sh

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