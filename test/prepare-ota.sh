#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-0.2.0}"
SEQ="${2:-2}"
OTA_DIR="$ROOT_DIR/test/ota/releases"

mkdir -p "$OTA_DIR"
BUILD_VERSION="$VERSION" RELEASE_SEQ="$SEQ" cargo build --release --bin sensor-agent
cp "$ROOT_DIR/target/release/sensor-agent" "$OTA_DIR/sensor-agent"
SHA256="$(sha256sum "$OTA_DIR/sensor-agent" | awk '{print $1}')"
cat > "$OTA_DIR/latest.json" <<EOF
{
  "version": "$VERSION",
  "release_seq": $SEQ,
  "url": "http://ota-server/releases/sensor-agent",
  "sha256": "$SHA256"
}
EOF
printf 'prepared OTA version %s, release_seq %s\n' "$VERSION" "$SEQ"
