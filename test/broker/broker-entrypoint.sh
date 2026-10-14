#!/bin/sh
set -eu

rm -rf /mosquitto/config /mosquitto/certs
mkdir -p /mosquitto/config /mosquitto/certs

cp /src-config/mosquitto.conf /mosquitto/config/mosquitto.conf
cp /src-config/passwd /mosquitto/config/passwd
cp /src-certs/ca.crt /mosquitto/certs/ca.crt
cp /src-certs/server.crt /mosquitto/certs/server.crt
cp /src-certs/server.key /mosquitto/certs/server.key

chown -R mosquitto:mosquitto /mosquitto/config /mosquitto/certs

chmod 0644 /mosquitto/config/mosquitto.conf
chmod 0600 /mosquitto/config/passwd
chmod 0644 /mosquitto/certs/ca.crt /mosquitto/certs/server.crt
chmod 0600 /mosquitto/certs/server.key

exec mosquitto -c /mosquitto/config/mosquitto.conf