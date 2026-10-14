#!/usr/bin/env bash
set -euo pipefail

TARGET=aarch64-unknown-linux-gnu
BIN_DIR=target/$TARGET/release
OUT=deploy-artifacts
SEQ_FILE="${RELEASE_SEQ_FILE:-.release-seq}"

VERSION="${VERSION:-${1:-$(git describe --tags --always)}}"
GIT_SHA="$(git rev-parse --short HEAD)"

if [[ -n "${RELEASE_SEQ:-}" ]]; then
  [[ "$RELEASE_SEQ" =~ ^[1-9][0-9]*$ ]] || {
    echo "RELEASE_SEQ must be a positive integer" >&2
    exit 1
  }
  if [[ -f "$SEQ_FILE" ]]; then
    previous_seq="$(<"$SEQ_FILE")"
    [[ "$previous_seq" =~ ^[0-9]+$ ]] || {
      echo "$SEQ_FILE must contain a non-negative integer" >&2
      exit 1
    }
    if (( RELEASE_SEQ < previous_seq )); then
      echo "RELEASE_SEQ must not be lower than $SEQ_FILE" >&2
      exit 1
    fi
  fi
else
  previous_seq=0
  if [[ -f "$SEQ_FILE" ]]; then
    previous_seq="$(<"$SEQ_FILE")"
    [[ "$previous_seq" =~ ^[0-9]+$ ]] || {
      echo "$SEQ_FILE must contain a non-negative integer" >&2
      exit 1
    }
  fi
  RELEASE_SEQ=$((previous_seq + 1))
fi

printf '%s\n' "$RELEASE_SEQ" > "$SEQ_FILE"

echo "==> build $VERSION (release_seq=$RELEASE_SEQ, $GIT_SHA)"
cargo clean --target "$TARGET" --release
BUILD_VERSION="$VERSION" RELEASE_SEQ="$RELEASE_SEQ" cargo build --target "$TARGET" --release --bin sensor-agent --bin sensor-agent-updater

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
  "schema": 1,
  "version": "$VERSION",
  "release_seq": $RELEASE_SEQ,
  "release_id": "$GIT_SHA-$RELEASE_SEQ",
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
