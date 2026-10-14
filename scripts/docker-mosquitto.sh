#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MOSQUITTO_DIR="$ROOT_DIR/mosquitto"
UI_DIR="$MOSQUITTO_DIR/ui"

test -f "$MOSQUITTO_DIR/config/mosquitto-secure.conf"
test -f "$MOSQUITTO_DIR/config/passwd"
test -f "$MOSQUITTO_DIR/config/acl"
test -f "$MOSQUITTO_DIR/certs/ca.crt"
test -f "$MOSQUITTO_DIR/certs/server.crt"
test -f "$MOSQUITTO_DIR/certs/server.key"
test -f "$UI_DIR/index.html"

docker rm -f sensor-mosquitto 2>/dev/null || true
docker volume create sensor-mosquitto-files >/dev/null

# Copy files into a Docker volume so the container can own secrets as uid 1883.
docker run --rm \
  --user 0:0 \
  --entrypoint sh \
  -v "$MOSQUITTO_DIR/config:/src-config:ro" \
  -v "$MOSQUITTO_DIR/certs:/src-certs:ro" \
  -v sensor-mosquitto-files:/mosquitto \
  eclipse-mosquitto:2 \
  -c 'rm -rf /mosquitto/config /mosquitto/certs && \
      mkdir -p /mosquitto/config /mosquitto/certs && \
      cp /src-config/mosquitto-secure.conf /mosquitto/config/mosquitto.conf && \
      cp /src-config/passwd /mosquitto/config/passwd && \
      cp /src-config/acl /mosquitto/config/acl && \
      cp /src-certs/ca.crt /src-certs/server.crt /src-certs/server.key /mosquitto/certs/ && \
      chown -R 1883:1883 /mosquitto && \
      chown root:1883 /mosquitto/config/passwd && \
      chmod 0644 /mosquitto/config/mosquitto.conf /mosquitto/config/acl /mosquitto/certs/ca.crt /mosquitto/certs/server.crt && \
      chmod 0640 /mosquitto/config/passwd && \
      chmod 0600 /mosquitto/certs/server.key'

docker run -d \
  --name sensor-mosquitto \
  --restart unless-stopped \
  -p 8883:8883 \
  -p 9001:9001 \
  -v sensor-mosquitto-files:/mosquitto:ro \
  eclipse-mosquitto:2

docker rm -f sensor-mosquitto-ui 2>/dev/null || true
docker run -d \
  --name sensor-mosquitto-ui \
  --restart unless-stopped \
  -p 8080:80 \
  -v "$UI_DIR:/usr/share/nginx/html:ro" \
  nginx:alpine