#!/usr/bin/env bash
# Sync editable source to the app Git authority using the installed standard transport.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -f ops/project.env ]]; then source ops/project.env; fi
checkout="${DPDF_RELEASE_CHECKOUT:-/app/dPDF/source}"
target="${DPDF_RELEASE_TARGET:?Set DPDF_RELEASE_TARGET in private ops/project.env}"
standard="${ORIG_HOME:-/www/0rig}"
[[ "$PWD" != "$checkout" ]] || { echo 'Already in release checkout.' >&2; exit 2; }
[[ -d "$checkout/.git" ]] || { echo "Clone the configured dPDF repository into $checkout first." >&2; exit 2; }
[[ "$(git -C "$checkout" remote get-url origin)" == git@github-w3f:w3fans/dPDF.git ]] || {
    echo 'Release checkout origin does not match the configured owner repository.' >&2; exit 2;
}
if [[ -n "$(git -C "$checkout" status --porcelain)" ]]; then
    echo 'Release checkout has uncommitted work; checkpoint or reconcile it before syncing.' >&2; exit 2
fi
# Pull requests merged on GitHub must reach the editable source before it overwrites
# the checkout; otherwise the next checkpoint would silently revert them.
git -C "$checkout" fetch --quiet origin '+refs/heads/main:refs/remotes/origin/main'
if ! git -C "$checkout" merge-base --is-ancestor origin/main HEAD; then
    echo 'GitHub main has merged changes that are not in the release checkout.' >&2
    echo 'Run ./release.sh upstream first; nothing was synchronized.' >&2
    exit 2
fi
"$standard/tools/rsyn" --chown="$(id -u):$(id -g)" --no-acls \
    --exclude='.git/' --exclude='target/' --exclude='dist/' --exclude='artifacts/' \
    --exclude='.agents/runtime/' --exclude='*.env' --exclude='.env' --exclude='.env.*' \
    --exclude='.aws/' --exclude='.ssh/' --exclude='.creditentials/' \
    --exclude='*.pem' --exclude='*.key' --exclude='*.p12' --exclude='*.pfx' \
    --exclude='asset-*/' --exclude='.import-*/' --exclude='recovery/' --exclude='*.dpdf' \
    --exclude='settings.local.json' --exclude='__pycache__/' "$PWD/" "$target"
