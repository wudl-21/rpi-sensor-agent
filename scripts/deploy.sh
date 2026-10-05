#!/usr/bin/env bash
set -euo pipefail

PI_HOST="${PI_HOST:-rpi3bv2}"
INCOMING=/var/lib/sensor-agent/incoming

echo "==> deploy artifacts to$PI_HOST:$INCOMING"
scp deploy-artifacts/sensor-agent \
    "$PI_HOST:$INCOMING/sensor-agent.new"

scp deploy-artifacts/sensor-agent.sha256 \
    "$PI_HOST:$INCOMING/sensor-agent.new.sha256"

scp deploy-artifacts/release.json \
    "$PI_HOST:$INCOMING/release.json"

echo "==> done. Pi watcher should pick it up."
