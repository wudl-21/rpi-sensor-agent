#!/usr/bin/env bash
set -euo pipefail

PI_HOST="${PI_HOST:-rpi3bv2}"
INCOMING="${INCOMING:-/var/lib/sensor-agent/incoming}"
RELEASE_ID="${RELEASE_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"

if [[ ! "$RELEASE_ID" =~ ^[A-Za-z0-9._-]+$ ]]; then
    echo "invalid RELEASE_ID: $RELEASE_ID" >&2
    exit 1
fi

READY_FILE="$(mktemp)"
trap 'rm -f "$READY_FILE"' EXIT
printf '%s\n' "$RELEASE_ID" > "$READY_FILE"

echo "==> deploy release $RELEASE_ID to $PI_HOST:$INCOMING"
scp deploy-artifacts/sensor-agent \
    "$PI_HOST:$INCOMING/release.$RELEASE_ID.sensor-agent"

scp deploy-artifacts/sensor-agent-updater \
    "$PI_HOST:$INCOMING/release.$RELEASE_ID.sensor-agent-updater"

scp deploy-artifacts/release.json \
    "$PI_HOST:$INCOMING/release.$RELEASE_ID.json"

# ready is uploaded last; the updater retries incomplete releases safely.
scp "$READY_FILE" \
    "$PI_HOST:$INCOMING/release.$RELEASE_ID.ready"

echo "==> done. Root updater service will process release.$RELEASE_ID.ready."
