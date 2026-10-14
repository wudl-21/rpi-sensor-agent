#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_DIR="$ROOT_DIR/test"
CERT_DIR="$TEST_DIR/certs"
BROKER_DIR="$TEST_DIR/broker"
PACKAGE_DIR="$TEST_DIR/package"

command -v openssl >/dev/null || { echo "openssl is required" >&2; exit 1; }
command -v docker >/dev/null || { echo "docker is required" >&2; exit 1; }

mkdir -p "$CERT_DIR" "$BROKER_DIR" "$TEST_DIR/ota/releases" "$PACKAGE_DIR/deploy-artifacts" "$PACKAGE_DIR/deploy/systemd" "$PACKAGE_DIR/scripts"
umask 077

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$CERT_DIR/ca.key" \
  -out "$CERT_DIR/ca.crt" \
  -subj '/CN=sensor-test-ca'

openssl req -newkey rsa:2048 -nodes \
  -keyout "$CERT_DIR/server.key" \
  -out "$CERT_DIR/server.csr" \
  -subj '/CN=test-broker'

cat > "$CERT_DIR/server.ext" <<'EOF'
[server_ext]
subjectAltName = DNS:test-broker, DNS:localhost, IP:127.0.0.1
EOF

openssl x509 -req -days 3650 -sha256 \
  -in "$CERT_DIR/server.csr" \
  -CA "$CERT_DIR/ca.crt" \
  -CAkey "$CERT_DIR/ca.key" \
  -CAcreateserial \
  -out "$CERT_DIR/server.crt" \
  -extfile "$CERT_DIR/server.ext" \
  -extensions server_ext

rm -f "$CERT_DIR/server.csr" "$CERT_DIR/server.ext" "$CERT_DIR/ca.srl"
chmod 0644 "$CERT_DIR/ca.crt" "$CERT_DIR/server.crt"
chmod 0600 "$CERT_DIR/ca.key" "$CERT_DIR/server.key"

if [[ -d "$BROKER_DIR/passwd" ]]; then
    rm -rf "$BROKER_DIR/passwd"
fi

install -m 0600 /dev/null "$BROKER_DIR/passwd"

docker run --rm \
  --user 0:0 \
  --entrypoint mosquitto_passwd \
  -v "$BROKER_DIR:/mosquitto/config" \
  eclipse-mosquitto:2 \
  -b /mosquitto/config/passwd sensor-console wudl1221

test -f "$BROKER_DIR/passwd"
chmod 0644 "$BROKER_DIR/passwd"

cargo build --release --bins
rm -rf "$PACKAGE_DIR/deploy-artifacts" "$PACKAGE_DIR/deploy" "$PACKAGE_DIR/scripts"
mkdir -p "$PACKAGE_DIR/deploy-artifacts" "$PACKAGE_DIR/deploy/systemd" "$PACKAGE_DIR/scripts"
cp "$ROOT_DIR/target/release/sensor-agent" "$PACKAGE_DIR/deploy-artifacts/"
cp "$ROOT_DIR/target/release/sensor-agent-updater" "$PACKAGE_DIR/deploy-artifacts/"
cp "$ROOT_DIR/scripts/install.sh" "$PACKAGE_DIR/scripts/install.sh"
chmod 0755 "$PACKAGE_DIR/scripts/install.sh"
cp "$ROOT_DIR/deploy/systemd/"*.in "$PACKAGE_DIR/deploy/systemd/"
cp "$CERT_DIR/ca.crt" "$PACKAGE_DIR/ca.crt"

printf 'prepared test environment\n'
printf 'SSH ports: 2221..2230\n'
printf 'MQTT TLS: test-broker:8883\n'
printf 'MQTT credentials: sensor-agent / test-password\n'
printf 'OTA: http://ota-server/releases\n'
