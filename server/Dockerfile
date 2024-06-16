ARG ALPINE_VERSION=3.13


FROM rust:1.79-alpine as build
WORKDIR /opt/app

COPY . .

RUN apk --no-cache add \
        musl-dev \
        postgresql-dev
RUN cargo build --release


FROM alpine:$ALPINE_VERSION as release
WORKDIR /opt/app

COPY --from=build /opt/app/assets assets
COPY --from=build /opt/app/target/release/server server
COPY --from=build /opt/app/.env .env
COPY --from=build /opt/app/config.toml config.toml
COPY --from=build /opt/app/docker-start.sh docker-start.sh

ARG PUID=1000
ARG PGID=1000

RUN apk --no-cache add \
        dumb-init \
 && mkdir -p /opt/app /data \
 && addgroup -g ${PGID} app \
 && adduser -SDH -h /opt/app -g '' -G app -u ${PUID} app \
 && chown -R app:app /opt/app /data

USER app
CMD ["/opt/app/docker-start.sh"]

ARG PORT=6666
ENV PORT=${PORT}
EXPOSE ${PORT}

ARG THREADS=4
ENV THREADS=${THREADS}

VOLUME ["/data/"]

ARG DOCKER_REPO
ARG BUILD_DATE
ARG SOURCE_COMMIT
LABEL \
    maintainer="" \
    org.opencontainers.image.title="${DOCKER_REPO}" \
    org.opencontainers.image.url="https://github.com/rr-/szurubooru" \
    org.opencontainers.image.documentation="https://github.com/rr-/szurubooru/blob/${SOURCE_COMMIT}/doc/INSTALL.md" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.source="https://github.com/rr-/szurubooru" \
    org.opencontainers.image.revision="${SOURCE_COMMIT}" \
    org.opencontainers.image.licenses="GPL-3.0"
