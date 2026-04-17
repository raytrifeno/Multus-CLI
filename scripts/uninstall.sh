#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="multus"
PRIMARY_DIR="$HOME/.local/bin"
SYSTEM_DIR="/usr/local/bin"

log() {
    printf '%s\n' "$1"
}

remove_file_if_exists() {
    local path="$1"
    if [[ -f "$path" ]]; then
        rm -f "$path"
        log "Removed: $path"
    fi
}

remove_path_export_if_present() {
    local profile="$1"
    if [[ ! -f "$profile" ]]; then
        return
    fi
    local before
    before="$(cat "$profile")"
    local after
    after="$(printf '%s\n' "$before" | sed '/# Added by Multus installer/,+1d')"
    if [[ "$before" != "$after" ]]; then
        printf '%s' "$after" > "$profile"
        log "Updated PATH profile: $profile"
    fi
}

remove_file_if_exists "${PRIMARY_DIR}/${BINARY_NAME}"
if [[ -w "$SYSTEM_DIR" ]]; then
    remove_file_if_exists "${SYSTEM_DIR}/${BINARY_NAME}"
fi

remove_path_export_if_present "$HOME/.profile"
remove_path_export_if_present "$HOME/.bashrc"
remove_path_export_if_present "$HOME/.zshrc"

log "Uninstall complete."
