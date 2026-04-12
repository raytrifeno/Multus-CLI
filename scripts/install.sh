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
LAST_FRAME_LINES=0
LAST_COMPLETED_TASK=""
ACTIVE_TASKS=()
PENDING_TASKS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --ui-mode)
            if [[ $# -lt 2 ]]; then
                echo "[multus-install] --ui-mode requires a value: auto|interactive|compact" >&2
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
            echo "[multus-install] Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

log() {
    printf '%s\n' "[multus-install] $1"
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
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
        log "Rust/Cargo not found."
        log "Install Rust first at: https://www.rust-lang.org/tools/install"
        exit 1
    fi
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
    local tasks=("$@")
    if [[ "${#tasks[@]}" -eq 0 ]]; then
        tasks=("multus")
    fi

    STAGE_TOTAL="${#tasks[@]}"
    STAGE_DONE=0
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
    local filled=$((done * width / total))
    local empty=$((width - filled))
    printf '['
    printf '%*s' "$filled" '' | tr ' ' '#'
    printf '%*s' "$empty" '' | tr ' ' '-'
    printf ']'
}

render_stage_interactive() {
    local title="$1"
    local final="${2:-0}"
    local bar
    bar="$(new_progress_bar "$STAGE_DONE" "$STAGE_TOTAL")"

    local lines=()
    lines+=("[multus-install] ${title}")
    lines+=("Progress ${bar} ${STAGE_DONE}/${STAGE_TOTAL} | Active: ${#ACTIVE_TASKS[@]}/${MAX_ACTIVE}")
    if [[ -n "$LAST_COMPLETED_TASK" ]]; then
        lines+=("Completed: ${LAST_COMPLETED_TASK}")
    fi

    local shown=0
    local item=""
    for item in "${ACTIVE_TASKS[@]}"; do
        [[ "$shown" -ge "$MAX_DISPLAY" ]] && break
        lines+=("  [RUNNING] ${item}")
        shown=$((shown + 1))
    done
    for item in "${PENDING_TASKS[@]}"; do
        [[ "$shown" -ge "$MAX_DISPLAY" ]] && break
        lines+=("  [QUEUED ] ${item}")
        shown=$((shown + 1))
    done
    if [[ "$shown" -eq 0 ]]; then
        lines+=("  waiting for events...")
    fi

    if [[ "$LAST_FRAME_LINES" -gt 0 ]]; then
        printf '\033[%sA' "$LAST_FRAME_LINES"
    fi

    local line
    for line in "${lines[@]}"; do
        printf '\033[2K%s\n' "$line"
    done
    LAST_FRAME_LINES="${#lines[@]}"

    if [[ "$final" -eq 1 ]]; then
        printf '\n'
        LAST_FRAME_LINES=0
    fi
}

render_stage_compact() {
    local title="$1"
    local bar
    bar="$(new_progress_bar "$STAGE_DONE" "$STAGE_TOTAL")"

    local active_preview="none"
    if [[ "${#ACTIVE_TASKS[@]}" -gt 0 ]]; then
        active_preview="$(printf '%s, ' "${ACTIVE_TASKS[@]:0:${MAX_ACTIVE}}" | sed 's/, $//')"
    fi

    local next_count=$((MAX_DISPLAY - ${#ACTIVE_TASKS[@]}))
    if [[ "$next_count" -lt 0 ]]; then
        next_count=0
    fi

    local next_preview="none"
    if [[ "${#PENDING_TASKS[@]}" -gt 0 && "$next_count" -gt 0 ]]; then
        next_preview="$(printf '%s, ' "${PENDING_TASKS[@]:0:${next_count}}" | sed 's/, $//')"
    fi

    local done_label="-"
    if [[ -n "$LAST_COMPLETED_TASK" ]]; then
        done_label="$LAST_COMPLETED_TASK"
    fi

    printf '[multus-install] %s | %s %s/%s | done: %s | active: %s | next: %s\n' \
        "$title" "$bar" "$STAGE_DONE" "$STAGE_TOTAL" "$done_label" "$active_preview" "$next_preview"
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
    shift
    local tasks=("$@")

    init_stage_state "${tasks[@]}"
    render_stage "$title" 0
    while [[ "$STAGE_DONE" -lt "$STAGE_TOTAL" ]]; do
        advance_stage_state
        render_stage "$title" 0
    done
    render_stage "$title" 1
    log "${title} complete."
}

run_stage() {
    local title="$1"
    local event_regex="$2"
    shift 2
    local cmd=("$@")

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
    render_stage "$title" 1
    log "${title} complete."
}

RENDER_MODE="$(resolve_ui_mode)"

if [[ "$DRY_RUN" -eq 1 ]]; then
    DRY_TASKS=()
    for i in $(seq 1 12); do
        DRY_TASKS+=("crate-$(printf '%02d' "$i")")
    done
    run_simulated_stage "Downloading crates (parallel task runner)" "${DRY_TASKS[@]}"
    COMPILE_TASKS=("${DRY_TASKS[@]}" "multus")
    run_simulated_stage "Compiling crates (parallel task runner)" "${COMPILE_TASKS[@]}"
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

init_stage_state "${PACKAGES[@]}"
run_stage \
    "Downloading crates (parallel task runner)" \
    'Downloaded[[:space:]]+[^[:space:]]+' \
    cargo fetch --locked --manifest-path "$MANIFEST_PATH" -vv

COMPILE_PACKAGES=("${PACKAGES[@]}" "multus")
init_stage_state "${COMPILE_PACKAGES[@]}"
run_stage \
    "Compiling crates (parallel task runner)" \
    'Compiling[[:space:]]+[^[:space:]]+' \
    cargo install --path "$WORK_DIR" --locked --force --bin multus -j "$MAX_ACTIVE"

if has_cmd multus; then
    log "Installation complete."
    multus --help
else
    log "Installed, but 'multus' is not on PATH in this session."
    log "Open a new terminal or add this path to your shell profile: \$HOME/.cargo/bin"
fi
