#!/usr/bin/env sh
set -eu

REPO_URL="https://github.com/raytrifeno/scraks.git"
REPO_REF="main"

log() {
    printf '%s\n' "[multus-install] $1"
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
}

ensure_cargo() {
    if has_cmd cargo; then
        log "Rust/Cargo detected. Skipping Rust installation."
        return
    fi

    log "Rust/Cargo not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable

    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1090
        . "$HOME/.cargo/env"
    fi

    if ! has_cmd cargo; then
        log "Cargo is not available yet. Open a new terminal, then run the installer again."
        exit 1
    fi
}

ensure_cargo

log "Installing multus from ${REPO_URL} (ref: ${REPO_REF})..."
if ! cargo install --git "$REPO_URL" --branch "$REPO_REF" --force --locked --bin multus; then
    log "cargo install failed. Ensure ${REPO_URL} (ref: ${REPO_REF}) contains Cargo.toml and binary target 'multus'."
    exit 1
fi

if has_cmd multus; then
    log "Installation complete. Run: multus --help"
else
    log "Installed, but 'multus' is not on PATH in this session."
    log "Add this path to your shell profile: \$HOME/.cargo/bin"
fi
