#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/raytrifeno/scraks.git"
REPO_REF="main"
MAX_DISPLAY=10
MAX_ACTIVE=3
UI_MODE_INPUT="auto"
DRY_RUN=0

WORK_DIR=""
RENDER_MODE="compact"
STAGE_TOTAL=0
STAGE_DONE=0
STAGE_TOTAL_UNITS=0
STAGE_UNIT_LABEL="count"
LAST_FRAME_LINES=0
LAST_COMPLETED_TASK=""
ACTIVE_TASKS=()
PENDING_TASKS=()
ANSI_RESET=$'\033[0m'
ANSI_GREEN=$'\033[32m'
ANSI_ORANGE=$'\033[38;5;208m'

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --ui-mode)
            if [[ $# -lt 2 ]]; then
                echo "--ui-mode requires a value: auto|interactive|compact" >&2
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

ensure_cargo_bin_on_path() {
    local cargo_bin="${HOME}/.cargo/bin"
    local export_line='export PATH="$HOME/.cargo/bin:$PATH"'
    local added_profile=0
    local added_session=0

    if [[ ! -d "$cargo_bin" ]]; then
        return 0
    fi

    if [[ ":$PATH:" != *":$cargo_bin:"* ]]; then
        export PATH="$cargo_bin:$PATH"
        hash -r 2>/dev/null || true
        added_session=1
    fi

    local target_profiles=("${HOME}/.profile")
    case "$(basename "${SHELL:-}")" in
        bash)
            target_profiles+=("${HOME}/.bashrc")
            ;;
        zsh)
            target_profiles+=("${HOME}/.zshrc")
            ;;
    esac

    local profile
    for profile in "${target_profiles[@]}"; do
        [[ -f "$profile" ]] || touch "$profile"
        if ! grep -Fqs "$export_line" "$profile"; then
            printf '\n# Added by multus installer\n%s\n' "$export_line" >> "$profile"
            added_profile=1
        fi
    done

    if [[ "$added_profile" -eq 1 ]]; then
        log "Added ~/.cargo/bin to PATH profile."
    fi
    if [[ "$added_session" -eq 1 ]]; then
        log "Updated PATH for current session."
    fi
}

resolve_ui_mode() {
    case "$UI_MODE_INPUT" in
        auto)
            if [[ -t 1 ]]; then
                echo "interactive"
            else
                echo "compact"
            fi
            ;;
        interactive|compact)
            echo "$UI_MODE_INPUT"
            ;;
        *)
            log "Invalid --ui-mode value: $UI_MODE_INPUT"
            exit 1
            ;;
    esac
}

ensure_prerequisites() {
    if ! has_cmd git; then
        log "git is required. Install git first, then run this installer again."
        exit 1
    fi

    if ! has_cmd cargo; then
        install_rust_toolchain
    fi

    if ! has_cmd cargo; then
        log "Rust/Cargo installation failed. Install manually at: https://www.rust-lang.org/tools/install"
        exit 1
    fi
}

install_rust_toolchain() {
    log "Rust/Cargo not found. Installing Rust toolchain..."

    if ! has_cmd curl; then
        log "curl is required to install Rust/Cargo automatically. Install curl first, then run this installer again."
        exit 1
    fi

    local rustup_script
    rustup_script="$(mktemp "${TMPDIR:-/tmp}/multus-rustup-XXXXXX.sh")"

    run_simulated_stage "Downloading" "count" 0 "rustup-init"
    curl -fsSL https://sh.rustup.rs -o "$rustup_script"

    local rust_tasks=("channel" "cargo" "clippy" "rust-docs" "rust-std" "rustc" "rustfmt")
    init_stage_state "count" 0 "${rust_tasks[@]}"
    run_stage \
        "Compiling" \
        'downloading component|installing component|syncing channel updates|default toolchain set to' \
        sh "$rustup_script" -y --profile default --default-toolchain stable

    rm -f "$rustup_script"
    ensure_cargo_bin_on_path
}

cleanup() {
    if [[ -n "${WORK_DIR}" && -d "${WORK_DIR}" ]]; then
        rm -rf "${WORK_DIR}"
    fi
}
trap cleanup EXIT

collect_lock_packages() {
    local lock_path="$1"
    awk '
        /^name = "/ {
            gsub(/"/, "", $3);
            if (!seen[$3]++) print $3;
        }
    ' "$lock_path"
}

init_stage_state() {
    local unit_label="${1:-count}"
    local total_units="${2:-0}"
    shift 2
    local tasks=("$@")
    if [[ "${#tasks[@]}" -eq 0 ]]; then
        tasks=("multus")
    fi

    STAGE_TOTAL="${#tasks[@]}"
    STAGE_DONE=0
    STAGE_UNIT_LABEL="$unit_label"
    if [[ "$total_units" -gt 0 ]]; then
        STAGE_TOTAL_UNITS="$total_units"
    else
        STAGE_TOTAL_UNITS="$STAGE_TOTAL"
    fi
    LAST_COMPLETED_TASK=""
    ACTIVE_TASKS=()
    PENDING_TASKS=("${tasks[@]}")

    while [[ "${#ACTIVE_TASKS[@]}" -lt "${MAX_ACTIVE}" && "${#PENDING_TASKS[@]}" -gt 0 ]]; do
        ACTIVE_TASKS+=("${PENDING_TASKS[0]}")
        PENDING_TASKS=("${PENDING_TASKS[@]:1}")
    done
}

advance_stage_state() {
    LAST_COMPLETED_TASK=""
    if [[ "${#ACTIVE_TASKS[@]}" -gt 0 ]]; then
        LAST_COMPLETED_TASK="${ACTIVE_TASKS[0]}"
        ACTIVE_TASKS=("${ACTIVE_TASKS[@]:1}")
    fi

    if [[ "${STAGE_DONE}" -lt "${STAGE_TOTAL}" ]]; then
        STAGE_DONE=$((STAGE_DONE + 1))
    fi

    if [[ "${#ACTIVE_TASKS[@]}" -lt "${MAX_ACTIVE}" && "${#PENDING_TASKS[@]}" -gt 0 ]]; then
        ACTIVE_TASKS+=("${PENDING_TASKS[0]}")
        PENDING_TASKS=("${PENDING_TASKS[@]:1}")
    fi
}

new_progress_bar() {
    local done="$1"
    local total="$2"
    local width=24
    if [[ "$total" -le 0 ]]; then
        total=1
    fi

    if [[ "$done" -le 0 ]]; then
        printf '[>'
        printf '%*s' "$((width - 1))" '' | tr ' ' '-'
        printf ']'
        return
    fi

    if [[ "$done" -ge "$total" ]]; then
        printf '['
        printf '%*s' "$width" '' | tr ' ' '='
        printf ']'
        return
    fi

    local span=$((width - 1))
    local filled=$((done * span / total))
    if [[ "$filled" -lt 2 ]]; then
        filled=2
    fi
    if [[ "$filled" -gt "$span" ]]; then
        filled="$span"
    fi
    local empty=$((width - filled - 1))

    printf '['
    printf '%*s' "$filled" '' | tr ' ' '='
    printf '>'
    printf '%*s' "$empty" '' | tr ' ' '-'
    printf ']'
}

format_stage_title() {
    local title="$1"
    title="${title//Downloading/${ANSI_GREEN}Downloading${ANSI_RESET}}"
    title="${title//Compiling/${ANSI_ORANGE}Compiling${ANSI_RESET}}"
    printf '%s' "$title"
}

format_loading_bar() {
    local bar="$1"
    printf '%s%s%s' "$ANSI_ORANGE" "$bar" "$ANSI_RESET"
}

stage_verb() {
    local title="$1"
    if [[ "$title" == Compiling* ]]; then
        printf '%sCompiling%s' "$ANSI_ORANGE" "$ANSI_RESET"
        return
    fi

    printf '%sDownloading%s' "$ANSI_GREEN" "$ANSI_RESET"
}

stage_summary_verb() {
    local title="$1"
    if [[ "$title" == Compiling* ]]; then
        printf '%s' "compile"
        return
    fi

    printf '%s' "download"
}

print_stage_summary() {
    local title="$1"
    local verb
    verb="$(stage_summary_verb "$title")"
    printf '%s %s/%s\n\n' "$verb" "$STAGE_DONE" "$STAGE_TOTAL"
}

current_stage_task() {
    if [[ -n "$LAST_COMPLETED_TASK" ]]; then
        printf '%s' "$LAST_COMPLETED_TASK"
        return
    fi

    if [[ "${#ACTIVE_TASKS[@]}" -gt 0 ]]; then
        printf '%s' "${ACTIVE_TASKS[0]}"
        return
    fi

    if [[ "${#PENDING_TASKS[@]}" -gt 0 ]]; then
        printf '%s' "${PENDING_TASKS[0]}"
        return
    fi

    printf '%s' "multus"
}

compute_progress_display() {
    if [[ "$STAGE_UNIT_LABEL" == "mb" ]]; then
        local total="${STAGE_TOTAL_UNITS}"
        if [[ "$total" -le 0 ]]; then
            total=1
        fi

        local done
        if [[ "$STAGE_DONE" -ge "$STAGE_TOTAL" ]]; then
            done="$total"
        elif [[ "$STAGE_TOTAL" -le 0 ]]; then
            done=0
        else
            done=$((STAGE_DONE * total / STAGE_TOTAL))
        fi

        PROGRESS_DONE="$done"
        PROGRESS_TOTAL="$total"
        PROGRESS_SUFFIX=" MB"
        return
    fi

    PROGRESS_DONE="$STAGE_DONE"
    PROGRESS_TOTAL="$STAGE_TOTAL"
    PROGRESS_SUFFIX=""
}

render_stage_interactive() {
    local title="$1"
    local final="${2:-0}"
    compute_progress_display
    local bar
    bar="$(new_progress_bar "$PROGRESS_DONE" "$PROGRESS_TOTAL")"
    local verb
    verb="$(stage_verb "$title")"
    local task
    task="$(current_stage_task)"
    local colored_bar
    colored_bar="$(format_loading_bar "$bar")"

    local line
    line="${verb} ${task} | ${colored_bar} ${PROGRESS_DONE}/${PROGRESS_TOTAL}${PROGRESS_SUFFIX}"

    if [[ "$LAST_FRAME_LINES" -gt 0 ]]; then
        printf '\033[%sA' "$LAST_FRAME_LINES"
    fi

    printf '\033[2K%s\n' "$line"
    LAST_FRAME_LINES=1

    if [[ "$final" -eq 1 ]]; then
        printf '\n'
        LAST_FRAME_LINES=0
    fi
}

render_stage_compact() {
    local title="$1"
    compute_progress_display
    local bar
    bar="$(new_progress_bar "$PROGRESS_DONE" "$PROGRESS_TOTAL")"
    local verb
    verb="$(stage_verb "$title")"
    local task
    task="$(current_stage_task)"
    local colored_bar
    colored_bar="$(format_loading_bar "$bar")"

    printf '%s %s | %s %s/%s\n' \
        "$verb" "$task" "$colored_bar" "$PROGRESS_DONE" "$PROGRESS_TOTAL${PROGRESS_SUFFIX}"
}

render_stage() {
    local title="$1"
    local final="${2:-0}"
    if [[ "$RENDER_MODE" == "interactive" ]]; then
        render_stage_interactive "$title" "$final"
    else
        render_stage_compact "$title"
    fi
}

run_simulated_stage() {
    local title="$1"
    local unit_label="${2:-count}"
    local total_units="${3:-0}"
    shift 3
    local tasks=("$@")

    init_stage_state "$unit_label" "$total_units" "${tasks[@]}"
    if [[ "$RENDER_MODE" != "interactive" ]]; then
        print_stage_summary "$title"
    fi
    render_stage "$title" 0
    while [[ "$STAGE_DONE" -lt "$STAGE_TOTAL" ]]; do
        advance_stage_state
        render_stage "$title" 0
    done
    if [[ "$RENDER_MODE" == "interactive" ]]; then
        render_stage "$title" 1
    fi
    log "${title} complete."
}

run_stage() {
    local title="$1"
    local event_regex="$2"
    shift 2
    local cmd=("$@")

    if [[ "$RENDER_MODE" != "interactive" ]]; then
        print_stage_summary "$title"
    fi
    render_stage "$title" 0

    local fifo
    fifo="$(mktemp -u)"
    mkfifo "$fifo"

    "${cmd[@]}" >"$fifo" 2>&1 &
    local cmd_pid=$!

    while IFS= read -r line; do
        if [[ "$line" =~ $event_regex ]]; then
            advance_stage_state
            render_stage "$title" 0
        fi
    done < "$fifo"

    wait "$cmd_pid"
    local cmd_status=$?
    rm -f "$fifo"

    if [[ "$cmd_status" -ne 0 ]]; then
        log "${title} failed (exit code: ${cmd_status})."
        exit "$cmd_status"
    fi

    while [[ "$STAGE_DONE" -lt "$STAGE_TOTAL" ]]; do
        advance_stage_state
        render_stage "$title" 0
    done
    if [[ "$RENDER_MODE" == "interactive" ]]; then
        render_stage "$title" 1
    fi
    log "${title} complete."
}

RENDER_MODE="$(resolve_ui_mode)"

if [[ "$DRY_RUN" -eq 1 ]]; then
    DRY_TASKS=()
    for i in $(seq 1 12); do
        DRY_TASKS+=("package-$(printf '%02d' "$i")")
    done
    DRY_DOWNLOAD_MB=$(( ${#DRY_TASKS[@]} * 12 ))
    run_simulated_stage "Downloading" "mb" "$DRY_DOWNLOAD_MB" "${DRY_TASKS[@]}"
    COMPILE_TASKS=("${DRY_TASKS[@]}" "multus")
    run_simulated_stage "Compiling" "count" 0 "${COMPILE_TASKS[@]}"
    log "Dry-run finished. No installation was performed."
    exit 0
fi

ensure_prerequisites

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/multus-install-XXXXXX")"

log "Cloning ${REPO_URL} (ref: ${REPO_REF})..."
git clone --depth 1 --branch "$REPO_REF" "$REPO_URL" "$WORK_DIR" >/dev/null

MANIFEST_PATH="${WORK_DIR}/Cargo.toml"
LOCK_PATH="${WORK_DIR}/Cargo.lock"

if [[ ! -f "$MANIFEST_PATH" ]]; then
    log "Cargo.toml not found in repository. Check ${REPO_URL} (ref: ${REPO_REF})."
    exit 1
fi

if [[ ! -f "$LOCK_PATH" ]]; then
    log "Cargo.lock not found. Generating lockfile..."
    (
        cd "$WORK_DIR"
        cargo generate-lockfile >/dev/null
    )
fi

PACKAGES=()
while IFS= read -r pkg; do
    [[ -n "$pkg" ]] && PACKAGES+=("$pkg")
done < <(collect_lock_packages "$LOCK_PATH")

if [[ "${#PACKAGES[@]}" -eq 0 ]]; then
    PACKAGES=("dependencies")
fi

DOWNLOAD_TOTAL_MB=$(( ${#PACKAGES[@]} * 6 ))
if [[ "$DOWNLOAD_TOTAL_MB" -lt 64 ]]; then
    DOWNLOAD_TOTAL_MB=64
fi

init_stage_state "mb" "$DOWNLOAD_TOTAL_MB" "${PACKAGES[@]}"
run_stage \
    "Downloading" \
    'Downloaded[[:space:]]+[^[:space:]]+' \
    cargo fetch --locked --manifest-path "$MANIFEST_PATH" -vv

COMPILE_PACKAGES=("${PACKAGES[@]}" "multus")
init_stage_state "count" 0 "${COMPILE_PACKAGES[@]}"
run_stage \
    "Compiling" \
    'Compiling[[:space:]]+[^[:space:]]+' \
    cargo install --path "$WORK_DIR" --locked --force --bin multus -j "$MAX_ACTIVE"

ensure_cargo_bin_on_path

if has_cmd multus; then
    log "Installation complete."
    multus --help
else
    log "Installed, but 'multus' is still not detected in this session."
    log "Try opening a new terminal. Expected binary location: ${HOME}/.cargo/bin/multus"
fi
