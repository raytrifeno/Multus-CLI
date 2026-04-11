#!/usr/bin/env bash
set -euo pipefail

log() {
    printf '%s\n' "[multus-uninstall] $1"
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
}

confirm_yes() {
    local prompt="$1"
    printf '%s [y/N] ' "$prompt"
    read -r answer
    [[ "$answer" =~ ^([yY]|[yY][eE][sS])$ ]]
}

CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME_DIR="${RUSTUP_HOME:-$HOME/.rustup}"

if has_cmd cargo; then
    log "Removing multus binary with cargo uninstall..."
    if ! cargo uninstall multus; then
        log "cargo uninstall returned non-zero status (continuing cleanup)."
    fi
else
    log "cargo not found. Skipping cargo uninstall step."
fi

if confirm_yes "Remove downloaded Cargo package cache used by Multus (registry + git cache)?"; then
    rm -rf "${CARGO_HOME_DIR}/registry" "${CARGO_HOME_DIR}/git"
    log "Cargo cache removed."
else
    log "Cargo cache kept."
fi

if confirm_yes "Also remove Rust installation (~/.rustup and ~/.cargo)?"; then
    rm -rf "${RUSTUP_HOME_DIR}" "${CARGO_HOME_DIR}"
    log "Rust toolchain and Cargo home removed."
else
    log "Rust installation kept."
fi

log "Uninstall complete."
