#!/usr/bin/env bash
set -euo pipefail

REPO_OWNER="raytrifeno"
REPO_NAME="Multus-CLI"
BINARY_NAME="multus"
INSTALL_DIR_DEFAULT="${HOME}/.local/bin"
UI_MODE_INPUT="auto"
DRY_RUN=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --ui-mode)
            if [[ $# -lt 2 ]]; then
                echo "--ui-mode requires a value" >&2
                exit 1
            fi
            UI_MODE_INPUT="$2"
            shift 2
            ;;
        --help)
            cat <<'EOF'
Usage: install.sh [--dry-run] [--ui-mode auto|interactive|compact]
EOF
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

log() {
    printf '%s\n' "$1"
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
}

resolve_asset_name() {
    local os_name
    local arch_name

    case "$(uname -s)" in
        Linux)
            os_name="linux"
            ;;
        Darwin)
            os_name="macos"
            ;;
        *)
            log "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)
            arch_name="x64"
            ;;
        arm64|aarch64)
            if [[ "$os_name" == "macos" ]]; then
                arch_name="arm64"
            else
                log "Unsupported architecture for Linux releases: $(uname -m)"
                exit 1
            fi
            ;;
        *)
            log "Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac

    if [[ "$os_name" == "linux" && "$arch_name" != "x64" ]]; then
        log "No Linux release asset available for architecture: $arch_name"
        exit 1
    fi

    printf 'multus-%s-%s.tar.gz' "$os_name" "$arch_name"
}

resolve_install_dir() {
    if [[ -w "/usr/local/bin" ]]; then
        printf '/usr/local/bin'
        return
    fi
    printf '%s' "$INSTALL_DIR_DEFAULT"
}

ensure_path_contains_install_dir() {
    local install_dir="$1"
    if [[ "$install_dir" != "$INSTALL_DIR_DEFAULT" ]]; then
        return
    fi

    if [[ ":$PATH:" == *":$install_dir:"* ]]; then
        return
    fi

    export PATH="$install_dir:$PATH"
    local export_line='export PATH="$HOME/.local/bin:$PATH"'
    local profiles=("$HOME/.profile")
    case "$(basename "${SHELL:-}")" in
        bash)
            profiles+=("$HOME/.bashrc")
            ;;
        zsh)
            profiles+=("$HOME/.zshrc")
            ;;
    esac

    local profile
    for profile in "${profiles[@]}"; do
        [[ -f "$profile" ]] || touch "$profile"
        if ! grep -Fqs "$export_line" "$profile"; then
            printf '\n# Added by Multus installer\n%s\n' "$export_line" >> "$profile"
        fi
    done
}

if ! has_cmd curl; then
    log "curl is required."
    exit 1
fi
if ! has_cmd tar; then
    log "tar is required."
    exit 1
fi

asset_name="$(resolve_asset_name)"
download_url="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/latest/download/${asset_name}"
install_dir="$(resolve_install_dir)"

log "Release asset: ${asset_name}"
log "Download URL: ${download_url}"
log "Install directory: ${install_dir}"

if [[ "$DRY_RUN" -eq 1 ]]; then
    log "Dry-run finished. No files were changed."
    exit 0
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/multus-install-XXXXXX")"
archive_path="${tmp_dir}/${asset_name}"
extract_dir="${tmp_dir}/extract"
trap 'rm -rf "$tmp_dir"' EXIT

mkdir -p "$extract_dir"
mkdir -p "$install_dir"

log "Downloading latest release binary..."
curl -fL "$download_url" -o "$archive_path"

log "Extracting archive..."
tar -xzf "$archive_path" -C "$extract_dir"

binary_path="${extract_dir}/${BINARY_NAME}"
if [[ ! -f "$binary_path" ]]; then
    log "Archive did not contain ${BINARY_NAME}."
    exit 1
fi

chmod +x "$binary_path"
cp "$binary_path" "${install_dir}/${BINARY_NAME}"
chmod +x "${install_dir}/${BINARY_NAME}"

ensure_path_contains_install_dir "$install_dir"
hash -r 2>/dev/null || true

log "Installation complete."
if has_cmd multus; then
    multus --help >/dev/null
    log "Command available: multus"
else
    log "Binary installed to ${install_dir}/${BINARY_NAME}."
    log "Open a new terminal if command is not yet in PATH."
fi
