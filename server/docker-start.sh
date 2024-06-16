#!/usr/bin/dumb-init /bin/sh
set -e
cd /opt/app

echo "Starting szurubooru API on port ${PORT}"
exec ./server
