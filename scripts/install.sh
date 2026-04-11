#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/raytrifeno/scraks.git"
REPO_REF="main"
MAX_VISIBLE=10
MAX_ACTIVE=3

WORK_DIR=""
STAGE_TOTAL=0
STAGE_DONE=0
ACTIVE_TASKS=()
PENDING_TASKS=()

log() {
    printf '%s\n' "[multus-install] $1"
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
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
    ACTIVE_TASKS=()
    PENDING_TASKS=("${tasks[@]}")

    while [[ "${#ACTIVE_TASKS[@]}" -lt "${MAX_ACTIVE}" && "${#PENDING_TASKS[@]}" -gt 0 ]]; do
        ACTIVE_TASKS+=("${PENDING_TASKS[0]}")
        PENDING_TASKS=("${PENDING_TASKS[@]:1}")
    done
}

advance_stage_state() {
    if [[ "${#ACTIVE_TASKS[@]}" -gt 0 ]]; then
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

render_parallel_ui() {
    local title="$1"
    printf '\033[2J\033[H'
    printf '[multus-install] %s\n' "$title"
    printf 'Progress: %s/%s | Active: %s/%s | Visible: %s\n\n' \
        "$STAGE_DONE" "$STAGE_TOTAL" "${#ACTIVE_TASKS[@]}" "$MAX_ACTIVE" "$MAX_VISIBLE"

    local shown=0
    local item=""

    for item in "${ACTIVE_TASKS[@]}"; do
        [[ "$shown" -ge "$MAX_VISIBLE" ]] && break
        printf '  [RUNNING] %s\n' "$item"
        shown=$((shown + 1))
    done

    for item in "${PENDING_TASKS[@]}"; do
        [[ "$shown" -ge "$MAX_VISIBLE" ]] && break
        printf '  [QUEUED ] %s\n' "$item"
        shown=$((shown + 1))
    done

    if [[ "$shown" -eq 0 ]]; then
        printf '  waiting for events...\n'
    fi
}

run_stage() {
    local title="$1"
    local event_regex="$2"
    shift 2
    local cmd=("$@")

    render_parallel_ui "$title"

    local fifo
    fifo="$(mktemp -u)"
    mkfifo "$fifo"

    "${cmd[@]}" >"$fifo" 2>&1 &
    local cmd_pid=$!

    while IFS= read -r line; do
        if [[ "$line" =~ $event_regex ]]; then
            advance_stage_state
            render_parallel_ui "$title"
        fi
    done < "$fifo"

    wait "$cmd_pid"
    local cmd_status=$?
    rm -f "$fifo"

    if [[ "$cmd_status" -ne 0 ]]; then
        log "$title failed (exit code: $cmd_status)."
        exit "$cmd_status"
    fi

    while [[ "$STAGE_DONE" -lt "$STAGE_TOTAL" ]]; do
        advance_stage_state
    done
    render_parallel_ui "$title"
    printf '\n'
    log "$title complete."
}

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
