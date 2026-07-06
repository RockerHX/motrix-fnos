#!/bin/sh

set -eu

CMD_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PACKAGE_ROOT=$(CDPATH= cd -- "${CMD_DIR}/.." && pwd)
APP_DEST=${TRIM_APPDEST:-"${PACKAGE_ROOT}/app"}
PKG_VAR=${TRIM_PKGVAR:-"${PACKAGE_ROOT}/app/data"}
SERVICE_PORT=${TRIM_SERVICE_PORT:-${MOTRIX_FNOS_HTTP_PORT:-17080}}
APP_DATA_DIR=${MOTRIX_FNOS_APP_DATA_DIR:-"${PKG_VAR}"}
SERVER_BIN=${MOTRIX_FNOS_SERVER_BIN:-"${APP_DEST}/bin/motrix-fnos-server"}
ARIA2_BIN_DEFAULT="${APP_DEST}/bin/aria2-next"
ARIA2_BIN=${MOTRIX_FNOS_ARIA2_PATH:-"${ARIA2_BIN_DEFAULT}"}
RUNTIME_DIR="${PKG_VAR}/run"
LOG_DIR="${PKG_VAR}/logs"
PID_FILE="${RUNTIME_DIR}/motrix-fnos-server.pid"
SERVER_LOG="${LOG_DIR}/server.log"
ACCESSIBLE_PATHS_FILE="${PKG_VAR}/accessible-paths.json"
HTTP_ADDR=${MOTRIX_FNOS_HTTP_ADDR:-"0.0.0.0:${SERVICE_PORT}"}

prepare_runtime_dirs() {
  mkdir -p "${APP_DATA_DIR}" "${RUNTIME_DIR}" "${LOG_DIR}"
}

export_runtime_env() {
  export MOTRIX_FNOS_APP_DATA_DIR="${APP_DATA_DIR}"
  export MOTRIX_FNOS_HTTP_ADDR="${HTTP_ADDR}"
  export MOTRIX_FNOS_ARIA2_PATH="${ARIA2_BIN}"
  export MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE="${ACCESSIBLE_PATHS_FILE}"
}

read_pid() {
  if [ -f "${PID_FILE}" ]; then
    tr -d '[:space:]' < "${PID_FILE}"
  fi
}

is_running_pid() {
  pid="$1"
  [ -n "${pid}" ] && kill -0 "${pid}" 2>/dev/null
}

clear_stale_pid() {
  pid=$(read_pid || true)
  if [ -n "${pid}" ] && ! is_running_pid "${pid}"; then
    rm -f "${PID_FILE}"
  fi
}

require_file() {
  path="$1"
  name="$2"
  if [ ! -f "${path}" ]; then
    echo "${name} 不存在：${path}" >&2
    exit 1
  fi
  if [ ! -x "${path}" ]; then
    chmod +x "${path}" 2>/dev/null || true
  fi
  if [ ! -x "${path}" ]; then
    echo "${name} 不可执行：${path}" >&2
    exit 1
  fi
}

log_msg() {
  mkdir -p "${LOG_DIR}"
  printf "%s %s\n" "$(date "+%Y-%m-%d %H:%M:%S")" "$1" >> "${SERVER_LOG}"
}

run_required_noop_hook() {
  hook_name="${1:-$(basename "$0")}"
  reason="保留空壳生命周期脚本（当前已验证 fnpack 版本存在文件校验要求）"

  if [ -n "${TRIM_TEMP_LOGFILE:-}" ]; then
    printf '%s %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "${reason}：${hook_name}" >> "${TRIM_TEMP_LOGFILE}"
  fi

  log_msg "${reason}：${hook_name}"
  exit 0
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

write_accessible_paths_file() {
  mkdir -p "${PKG_VAR}"
  tmp_file="${ACCESSIBLE_PATHS_FILE}.tmp"
  printf '{"paths":[' > "${tmp_file}"

  old_ifs="${IFS}"
  IFS=':'
  first=1
  for accessible_path in ${TRIM_DATA_ACCESSIBLE_PATHS:-}; do
    if [ -z "${accessible_path}" ]; then
      continue
    fi
    if [ "${first}" -eq 0 ]; then
      printf ',' >> "${tmp_file}"
    fi
    printf '"%s"' "$(json_escape "${accessible_path}")" >> "${tmp_file}"
    first=0
  done
  IFS="${old_ifs}"

  printf ']}\n' >> "${tmp_file}"
  mv "${tmp_file}" "${ACCESSIBLE_PATHS_FILE}"
  log_msg "已同步授权目录列表到 ${ACCESSIBLE_PATHS_FILE}"
}
