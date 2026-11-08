#!/usr/bin/env bash
# Standard release adapter: guarded development checkpoints and deterministic version checks.
set -euo pipefail
cd "$(dirname "$0")"
if [[ -f ops/project.env ]]; then source ops/project.env; fi
if [[ "${1:-}" == delivery-status || "${1:-}" == deliver ]]; then
    exec python3 scripts/ci/release_page.py "$@"
fi
if [[ "${1:-}" == patch ]]; then
    export PYTHONOPTIMIZE=0
    [[ $# -eq 1 && "${PROJECT_COPY_ROLE:-active-source}" == active-source ]] || { echo 'Run ./release.sh patch from the editable source.' >&2; exit 2; }
    mkdir -p .agents/runtime
    exec 8>.agents/runtime/preview-prepare.lock
    flock -w 60 8
    # Refuse before changing version files if merged pull requests are not yet local.
    release_checkout="${DPDF_RELEASE_CHECKOUT:-/app/dPDF/source}"
    git -C "$release_checkout" fetch --quiet origin '+refs/heads/main:refs/remotes/origin/main'
    git -C "$release_checkout" merge-base --is-ancestor origin/main HEAD || {
        echo 'GitHub main has merged pull requests; run ./release.sh upstream first.' >&2; exit 2; }
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-release-version.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-private-display.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-public-export.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_ci.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_artifact_cache.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_local_copr.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_release_page.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_delivery_checkpoint.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_resolve_ref.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_pr_policy.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-upstream-sync.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-cargo-notices.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-print-dialog.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/test-rpm-publication.py
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-cargo-notices.py --target linux
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-cargo-notices.py --target windows
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-desktop.py
    python3 scripts/release-version.py prepare
    cargo fmt --check
    cargo test --locked --release
    cargo clippy --locked -- -D warnings
    cargo clippy --locked --features check-windows-print -- -D warnings
    ./build --release
    python3 scripts/release-version.py verify --binary target/release/dpdf
    : "${DPDF_PDFIUM_LIBRARY:?Set DPDF_PDFIUM_LIBRARY to the validated PDFium runtime for release checks}"
    : "${DPDF_QPDF_BINARY:?Set DPDF_QPDF_BINARY to the validated qpdf helper for release checks}"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-reader.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-engine-paths.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY"
    benchmark_args=(--geometry-only)
    if [[ -n "${DPDF_TEST_READER_BENCHMARK:-}" ]]; then benchmark_args=(--output docs/validation/reader-benchmark-latest.json); fi
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/benchmark-reader.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" "${benchmark_args[@]}"
    if [[ -n "${DPDF_READER_BASELINE_BINARY:-}" ]]; then
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/benchmark-buffers.py --binary target/release/dpdf --baseline-binary "$DPDF_READER_BASELINE_BINARY" --pdfium "$DPDF_PDFIUM_LIBRARY" --output docs/validation/shared-reader-memory-latest.json
    fi
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-pages.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-marks.py --binary target/release/dpdf
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-overlays.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-page-units.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-review.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY"
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-aliases.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY"
    block_args=()
    if [[ -n "${DPDF_TEST_POPPLER:-}" ]]; then block_args+=(--poppler); fi
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-block-rust.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" "${block_args[@]}" --output docs/validation/rust-block-latest.json
    if [[ -n "${DPDF_TEST_POPPLER:-}" ]]; then
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-poppler.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --output docs/validation/poppler-latest.json
    fi
    if [[ -n "${DPDF_TEST_BLOCK_PROTOTYPE:-}" ]]; then
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/probe-combined-block.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --output docs/validation/combined-block-prototype-latest.json
    fi
    if [[ -n "${DPDF_TEST_BLOCK_GEOMETRY:-}" ]]; then
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/probe-block-geometry.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --output docs/validation/block-geometry-latest.json
    fi
    if [[ -n "${DPDF_TEST_XVFB:-}" ]]; then
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-file-drop.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --xvfb "$DPDF_TEST_XVFB"
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-about.py --binary target/release/dpdf --xvfb "$DPDF_TEST_XVFB"
        for scale in 1 1.5 2; do
            PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-viewport.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --xvfb "$DPDF_TEST_XVFB" --scale "$scale"
            PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-physical-reader.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB" --scale "$scale"
            PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-tabs.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --xvfb "$DPDF_TEST_XVFB" --scale "$scale"
            PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-clipboard.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB" --placement --scale "$scale"
            PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-clipboard.py --binary target/release/dpdf --xvfb "$DPDF_TEST_XVFB" --marks --scale "$scale"
        done
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-review-ui.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB"
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-passwords.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB"
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-recovery.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB"
        PYTHONDONTWRITEBYTECODE=1 python3 scripts/check-native-print.py --binary target/release/dpdf --pdfium "$DPDF_PDFIUM_LIBRARY" --qpdf "$DPDF_QPDF_BINARY" --xvfb "$DPDF_TEST_XVFB"
    fi
    if [[ -n "${DPDF_PDFIUM_DISTRIBUTION:-}" ]]; then ./scripts/stage-linux.sh /app/dPDF; fi
    ./scripts/sync-release.sh
    "${DPDF_RELEASE_CHECKOUT:-/app/dPDF/source}/release.sh" publish-preview
    rm .agents/runtime/preview-pending.json
    exit 0
fi
if [[ "${1:-}" == upstream ]]; then
    # Bring merged GitHub pull requests into the release checkout and editable source.
    shift
    checkout="${DPDF_RELEASE_CHECKOUT:-/app/dPDF/source}"
    [[ "${PROJECT_COPY_ROLE:-active-source}" == active-source && "$PWD" != "$checkout" ]] || {
        echo 'Run ./release.sh upstream from the editable source.' >&2; exit 2; }
    [[ "$(git -C "$checkout" remote get-url origin)" == git@github-w3f:w3fans/dPDF.git ]] || {
        echo 'Unexpected repository origin.' >&2; exit 2; }
    mkdir -p "$checkout/.agents/runtime"
    exec 9>"$checkout/.agents/runtime/release.lock"
    flock -w 60 9
    PYTHONDONTWRITEBYTECODE=1 python3 scripts/upstream-sync.py --source "$PWD" --checkout "$checkout" "$@"
    exit 0
fi
if [[ "${1:-}" == checkpoint || "${1:-}" == publish-preview ]]; then
    operation="$1"
    [[ $# -eq 1 ]] || { echo 'Usage: ./release.sh checkpoint|publish-preview' >&2; exit 2; }
    checkout="${DPDF_RELEASE_CHECKOUT:-/app/dPDF/source}"
    if [[ "${PROJECT_COPY_ROLE:-active-source}" != build-release ]]; then
        ./scripts/sync-release.sh
        exec "$checkout/release.sh" "$operation"
    fi
    [[ "$PWD" == "$checkout" ]] || { echo 'Checkpoint must run in the configured build-release checkout.' >&2; exit 2; }
    [[ "$(git remote get-url origin)" == git@github-w3f:w3fans/dPDF.git ]] || { echo 'Unexpected repository origin.' >&2; exit 2; }
    standard="${ORIG_HOME:-/www/0rig}"
    agent="${AGENT_NAME:-dpdf-checkpoint}"
    mkdir -p .agents/runtime
    exec 9>.agents/runtime/release.lock
    flock -w 60 9
    created_lease=0
    cleanup() {
        if [[ "$created_lease" == 1 ]]; then "$standard/tools/lease-adapter.sh" finish "$agent" "$PWD"; fi
    }
    trap cleanup EXIT
    if [[ ! -f ".agents/runtime/$agent.lease" ]]; then
        "$standard/tools/lease-adapter.sh" start "$agent" 'Checkpoint synchronized dPDF source to owner GitHub repository' "$PWD"
        created_lease=1
        "$standard/tools/lease-adapter.sh" claim-baseline "$agent" 'Owner-authorized source snapshot synchronized through standard rsyn; no competing lease' "$PWD"
    fi
    "$standard/tools/lease-adapter.sh" release-check "$agent" "$PWD"
    git fetch origin
    branch="$(git symbolic-ref --short HEAD)"
    if git show-ref --verify --quiet "refs/remotes/origin/$branch"; then
        git rev-parse --verify HEAD >/dev/null
        git merge-base --is-ancestor "origin/$branch" HEAD || { echo 'Reconcile upstream history before checkpointing.' >&2; exit 2; }
    fi
    ./.agents/validate
    tag=""
    if [[ "$operation" == publish-preview ]]; then
        tag="$(python3 scripts/release-version.py verify --binary "${DPDF_SOURCE_CHECKOUT:-/zDev/dPDF}/target/release/dpdf")"
        [[ "$(git symbolic-ref --short HEAD)" == main ]] || { echo 'Preview releases belong on main.' >&2; exit 2; }
    fi
    git add -- .
    git diff --cached --check
    if ! git diff --cached --quiet; then
        if [[ -n "$tag" ]]; then message="Release $tag development preview";
        else message="$(awk '/^## Unreleased/ {active=1; next} active && /^## / {exit} active && /^- / {sub(/^- /, ""); print; exit}' CHANGELOG.md)"; fi
        [[ -n "$message" ]] || { echo 'Checkpoint needs an Unreleased changelog entry.' >&2; exit 2; }
        git commit -m "$message"
    fi
    export AGENT_NAME="$agent"
    if [[ -n "$tag" ]]; then
        if git show-ref --verify --quiet "refs/tags/$tag"; then
            [[ "$(git rev-parse "$tag^{}")" == "$(git rev-parse HEAD)" ]] || { echo 'Existing tag points to a different commit; tags are never moved.' >&2; exit 2; }
        else
            git tag -a "$tag" --file "docs/releases/$tag.md"
        fi
        git push --atomic --set-upstream origin "$branch" "refs/tags/$tag"
        # The immutable pushed tag is the durable input to the existing local
        # publication timers. Wake the same unit when running on its operator;
        # the release authority may be a different host and needs no API secret.
        if [[ -f "$HOME/.config/dpdf/publication.env" ]]; then
            if ! python3 scripts/ci/release_page.py deliver --tag "$tag"; then
                echo "PENDING_PUBLICATION_HANDOFF: existing operator timers must reconcile $tag." >&2
            fi
        fi
        echo "TAGGED $tag; PUBLICATION_QUEUED through registered local tag watchers. Delivery is incomplete until page, Fedora and Windows receipts verify."
    else
        git push --set-upstream origin "$branch"
        echo "Development checkpoint pushed; no version or tag was changed."
    fi
    exit 0
fi
controller="${ORIG_HOME:-/www/0rig}/tools/release-controller.sh"
if [[ ! -x "$controller" ]] || ! command -v ruby >/dev/null 2>&1; then controller="$PWD/.agents/bin/release-controller"; fi
if [[ ! -x "$controller" ]]; then
    echo 'Install the standard release tools before running release checks.' >&2
    exit 2
fi
case "${1:-}" in
    next|preflight|validate-result|sync|rollback-check) exec "$controller" "$@" ;;
    *)
        echo 'Usage: ./release.sh delivery-status | deliver --tag TAG | upstream [--dry-run] | patch | checkpoint | next | preflight | validate-result | sync | rollback-check' >&2
        echo 'This adapter accepts standard release-controller subcommands; see docs/BUILDING.md.' >&2
        exit 2
        ;;
esac
