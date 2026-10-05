#!/usr/bin/env bash
set -euo pipefail

TARGET=aarch64-unknown-linux-gnu
BIN=target/$TARGET/release/sensor-agent
OUT=deploy-artifacts

VERSION="${1:-$(git describe --tags --always)}"
GIT_SHA="$(git rev-parse --short HEAD)"

echo "==> build $VERSION ($GIT_SHA)"
cargo build --target "$TARGET" --release

mkdir -p "$OUT"

# 1. 二进制
cp "$BIN" "$OUT/sensor-agent"

# 2. 纯 hash（关键：只要第一列）
sha256sum "$BIN" | awk '{print$1}' > "$OUT/sensor-agent.sha256"

# 3. manifest
cat > "$OUT/release.json" <<EOF # EOF=多行输入起止符
{
  "version": "$VERSION",
  "target": "$TARGET",
  "sha256": "$(awk '{print$1}' "$OUT/sensor-agent.sha256")",
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  ersion": "$VERSION",
  "git_sha": "$GIT_SHA"
}
EOF

echo "==> artifacts:"
ls -l "$OUT"
cat "$OUT/sensor-agent.sha256"
