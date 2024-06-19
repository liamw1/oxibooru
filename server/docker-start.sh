#!/bin/bash
set -e
cd /opt/app

echo "Starting oxibooru API on port ${PORT}"
exec ./server
