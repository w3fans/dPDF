#!/usr/bin/env bash
# Compatibility name; the installed standard workflow remains authoritative.
set -euo pipefail
cd "$(dirname "$0")"
exec ./build "$@"
