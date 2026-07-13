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
PID_START_FILE="${RUNTIME_DIR}/motrix-fnos-server.starttime"
SERVER_LOG="${LOG_DIR}/server.log"
ACCESSIBLE_PATHS_FILE="${PKG_VAR}/accessible-paths.json"
HTTP_ADDR=${MOTRIX_FNOS_HTTP_ADDR:-"0.0.0.0:${SERVICE_PORT}"}
PROC_ROOT=${MOTRIX_FNOS_PROC_ROOT:-/proc}

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
    sed -n '1{s/[[:space:]]//g;p;}' "${PID_FILE}"
  fi
}

process_start_time() {
  pid="$1"
  stat_file="${PROC_ROOT}/${pid}/stat"
  [ -r "${stat_file}" ] || return 1
  awk '{print $22}' "${stat_file}"
}

canonical_file_path() {
  path="$1"
  directory=$(dirname -- "${path}")
  file_name=$(basename -- "${path}")
  [ -d "${directory}" ] || return 1
  directory=$(CDPATH= cd -- "${directory}" && pwd -P) || return 1
  printf '%s/%s\n' "${directory}" "${file_name}"
}

process_executable_path() {
  pid="$1"
  executable_link="${PROC_ROOT}/${pid}/exe"
  [ -L "${executable_link}" ] || return 1
  executable_path=$(readlink "${executable_link}") || return 1
  canonical_file_path "${executable_path}"
}

write_pid_record() {
  pid="$1"
  start_time=$(process_start_time "${pid}") || return 1
  printf '%s\n' "${pid}" > "${PID_FILE}"
  printf '%s\n' "${start_time}" > "${PID_START_FILE}"
}

remove_pid_record() {
  rm -f "${PID_FILE}" "${PID_START_FILE}"
}

is_running_pid() {
  pid="$1"
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "${pid}" 2>/dev/null || return 1

  # PID 可能被系统复用；停止或报告运行中前，必须同时确认可执行文件，并在有记录时核对进程启动时间。
  expected_executable=$(canonical_file_path "${SERVER_BIN}") || return 1
  actual_executable=$(process_executable_path "${pid}") || return 1
  [ "${actual_executable}" = "${expected_executable}" ] || return 1

  if [ -f "${PID_START_FILE}" ]; then
    recorded_start_time=$(tr -d '[:space:]' < "${PID_START_FILE}")
    actual_start_time=$(process_start_time "${pid}") || return 1
    [ -n "${recorded_start_time}" ] && [ "${recorded_start_time}" = "${actual_start_time}" ] || return 1
  fi
}

clear_stale_pid() {
  pid=$(read_pid || true)
  if [ -f "${PID_FILE}" ] && { [ -z "${pid}" ] || ! is_running_pid "${pid}"; }; then
    remove_pid_record
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

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

write_accessible_paths_file() {
  mkdir -p "${PKG_VAR}"
  # fnOS 以冒号分隔 TRIM_DATA_ACCESSIBLE_PATHS；先完整写入临时文件再原子替换，避免 server 读到半截授权列表 JSON。
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
