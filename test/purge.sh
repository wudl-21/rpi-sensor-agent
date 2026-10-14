#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

rm -rf "$SCRIPT_DIR/certs" "$SCRIPT_DIR/package"
rm -rf "$SCRIPT_DIR/broker/passwd"
rm -f "$SCRIPT_DIR/ota/releases/*"