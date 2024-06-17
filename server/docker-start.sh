#!/bin/bash
set -e
cd /opt/app

echo "Starting szurubooru API on port ${PORT}"
exec ./server
