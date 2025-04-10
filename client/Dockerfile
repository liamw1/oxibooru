FROM --platform=$BUILDPLATFORM node:lts AS builder
WORKDIR /opt/app

COPY package.json package-lock.json ./
RUN npm install

COPY . ./

ARG BUILD_INFO="docker-latest"
ARG CLIENT_BUILD_ARGS=""
RUN BASE_URL="__BASEURL__" node build.js --gzip ${CLIENT_BUILD_ARGS}


FROM --platform=$BUILDPLATFORM scratch AS approot

COPY docker-start.sh /

WORKDIR /etc/nginx
COPY nginx.conf.docker ./nginx.conf

WORKDIR /var/www
COPY --from=builder /opt/app/public/ .


FROM nginx:alpine AS release

RUN apk --no-cache add dumb-init
COPY --from=approot / /

CMD ["/docker-start.sh"]
VOLUME ["/data"]

LABEL \
    maintainer="" \
    org.opencontainers.image.title="oxibooru/client" \
    org.opencontainers.image.url="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.source="https://github.com/liamw1/oxibooru" \
    org.opencontainers.image.licenses="GPL-3.0"