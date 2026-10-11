#!/usr/bin/env bash
set -euo pipefail

TARGET=aarch64-unknown-linux-gnu
BIN_DIR=target/$TARGET/release
OUT=deploy-artifacts

VERSION="${1:-$(git describe --tags --always)}"
GIT_SHA="$(git rev-parse --short HEAD)"

echo "==> build $VERSION ($GIT_SHA)"
cargo clean --target "$TARGET" --release
cargo build --target "$TARGET" --release --bin sensor-agent --bin sensor-agent-updater

mkdir -p "$OUT"

# 1. 二进制
cp "$BIN_DIR/sensor-agent" "$OUT/sensor-agent"
cp "$BIN_DIR/sensor-agent-updater" "$OUT/sensor-agent-updater"

# 2. 纯 hash（关键：只要第一列）
sha256sum "$BIN_DIR/sensor-agent" | awk '{print$1}' > "$OUT/sensor-agent.sha256"
sha256sum "$BIN_DIR/sensor-agent-updater" | awk '{print$1}' > "$OUT/sensor-agent-updater.sha256"

# 3. manifest
cat > "$OUT/release.json" <<EOF # EOF=多行输入起止符
{
  "version": "$VERSION",
  "target": "$TARGET",
  "sha256": "$(awk '{print$1}' "$OUT/sensor-agent.sha256")",
  "updater_sha256": "$(awk '{print$1}' "$OUT/sensor-agent-updater.sha256")",
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "git_sha": "$GIT_SHA"
}
EOF

echo "==> artifacts:"
ls -l "$OUT"
cat "$OUT/sensor-agent.sha256"
