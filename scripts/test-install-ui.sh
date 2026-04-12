#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SCRIPT="${SCRIPT_DIR}/install.sh"

if [[ ! -f "$INSTALL_SCRIPT" ]]; then
    echo "[multus-test] install.sh not found at: $INSTALL_SCRIPT" >&2
    exit 1
fi

OUTPUT="$(bash "$INSTALL_SCRIPT" --dry-run --ui-mode compact)"

if ! grep -q "Downloading crates (parallel task runner)" <<<"$OUTPUT"; then
    echo "[multus-test] download stage title missing" >&2
    exit 1
fi

if ! grep -q "Compiling crates (parallel task runner)" <<<"$OUTPUT"; then
    echo "[multus-test] compile stage title missing" >&2
    exit 1
fi

if ! grep -Eq '\[[#-]+\][[:space:]]+[0-9]+/[0-9]+' <<<"$OUTPUT"; then
    echo "[multus-test] progress bar output missing" >&2
    exit 1
fi

if grep -q "Visible:" <<<"$OUTPUT"; then
    echo "[multus-test] unexpected 'Visible:' text found" >&2
    exit 1
fi

if ! grep -q "done: crate-01" <<<"$OUTPUT"; then
    echo "[multus-test] expected first completed crate event missing" >&2
    exit 1
fi

if ! grep -q "Dry-run finished. No installation was performed." <<<"$OUTPUT"; then
    echo "[multus-test] dry-run completion message missing" >&2
    exit 1
fi

echo "[multus-test] install UI dry-run test passed."
