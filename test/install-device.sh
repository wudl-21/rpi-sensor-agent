#!/usr/bin/env bash
set -euo pipefail

PORT="${1:?usage: test/install-device.sh SSH_PORT}"
HOST="${TEST_DEVICE_HOST:-127.0.0.1}"

ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p "$PORT" "root@$HOST" <<'REMOTE'
set -eu
install -d -m 0755 /etc/sensor-agent
install -o root -g root -m 0644 /root/package/ca.crt /etc/sensor-agent/ca.crt
/root/package/scripts/install.sh \
  --no-start \
  --mqtt-host test-broker \
  --mqtt-port 8883 \
  --mqtt-user sensor-agent \
  --mqtt-password test-password \
  --mqtt-ca-file /etc/sensor-agent/ca.crt \
  --mqtt-tls \
  --ota-base-url http://ota-server/releases
systemctl enable --now sensor-agent-updater.service sensor-agent.service
REMOTE

printf 'installed device on SSH port %s\n' "$PORT"
