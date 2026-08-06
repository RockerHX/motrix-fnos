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
ARIA2_RUNTIME_FILE="${APP_DATA_DIR}/aria2-runtime.json"
SERVER_LOG="${LOG_DIR}/server.log"
LIFECYCLE_LOG="${LOG_DIR}/lifecycle.log"
LIFECYCLE_LOG_MAX_BYTES=${MOTRIX_FNOS_LIFECYCLE_LOG_MAX_BYTES:-1048576}
LIFECYCLE_LOG_RETENTION=${MOTRIX_FNOS_LIFECYCLE_LOG_RETENTION:-3}
ACCESSIBLE_PATHS_FILE="${PKG_VAR}/accessible-paths.json"
HTTP_ADDR=${MOTRIX_FNOS_HTTP_ADDR:-"0.0.0.0:${SERVICE_PORT}"}
JSONRPC_ADDR=${MOTRIX_FNOS_JSONRPC_ADDR:-"127.0.0.1:17081"}
LAN_JSONRPC_ADDR=${MOTRIX_FNOS_LAN_JSONRPC_ADDR:-"0.0.0.0:17082"}
PROC_ROOT=${MOTRIX_FNOS_PROC_ROOT:-/proc}
READINESS_ATTEMPTS=${MOTRIX_FNOS_READINESS_ATTEMPTS:-10}
READINESS_RETRY_SECONDS=${MOTRIX_FNOS_READINESS_RETRY_SECONDS:-1}
READINESS_REQUEST_TIMEOUT_SECONDS=${MOTRIX_FNOS_READINESS_REQUEST_TIMEOUT_SECONDS:-2}
PROCESS_IDENTITY_ATTEMPTS=${MOTRIX_FNOS_PROCESS_IDENTITY_ATTEMPTS:-20}
PROCESS_IDENTITY_RETRY_SECONDS=${MOTRIX_FNOS_PROCESS_IDENTITY_RETRY_SECONDS:-0.1}
START_CLEANUP_ATTEMPTS=${MOTRIX_FNOS_START_CLEANUP_ATTEMPTS:-20}
START_CLEANUP_RETRY_SECONDS=${MOTRIX_FNOS_START_CLEANUP_RETRY_SECONDS:-0.1}
STOP_INT_ATTEMPTS=${MOTRIX_FNOS_STOP_INT_ATTEMPTS:-12}
STOP_TERM_ATTEMPTS=${MOTRIX_FNOS_STOP_TERM_ATTEMPTS:-4}
STOP_KILL_ATTEMPTS=${MOTRIX_FNOS_STOP_KILL_ATTEMPTS:-2}
STOP_RETRY_SECONDS=${MOTRIX_FNOS_STOP_RETRY_SECONDS:-1}
CURL_BIN=${MOTRIX_FNOS_CURL_BIN:-curl}
WGET_BIN=${MOTRIX_FNOS_WGET_BIN:-wget}

prepare_runtime_dirs() {
  mkdir -p "${APP_DATA_DIR}" "${RUNTIME_DIR}" "${LOG_DIR}"
  rotate_lifecycle_log
}

export_runtime_env() {
  export MOTRIX_FNOS_APP_DATA_DIR="${APP_DATA_DIR}"
  export MOTRIX_FNOS_HTTP_ADDR="${HTTP_ADDR}"
  export MOTRIX_FNOS_JSONRPC_ADDR="${JSONRPC_ADDR}"
  export MOTRIX_FNOS_LAN_JSONRPC_ADDR="${LAN_JSONRPC_ADDR}"
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

process_uid() {
  pid="$1"
  status_file="${PROC_ROOT}/${pid}/status"
  [ -r "${status_file}" ] || return 1
  awk '$1 == "Uid:" { print $2; exit }' "${status_file}"
}

process_matches_current_uid() {
  pid="$1"
  current_uid=$(id -u 2>/dev/null || true)
  actual_uid=$(process_uid "${pid}" || true)
  [ -n "${current_uid}" ] || return 1
  # 测试替身和部分非 Linux proc 实现没有 status 文件；真实 proc 有 UID 时必须严格核对。
  [ -z "${actual_uid}" ] || [ "${actual_uid}" = "${current_uid}" ]
}

process_is_alive() {
  pid="$1"
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "${pid}" 2>/dev/null
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

process_matches_executable() {
  pid="$1"
  expected_path="$2"
  expected_executable=$(canonical_file_path "${expected_path}") || return 1
  actual_executable=$(process_executable_path "${pid}") || return 1
  [ "${actual_executable}" = "${expected_executable}" ]
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
  process_matches_executable "${pid}" "${SERVER_BIN}" || return 1
  process_matches_current_uid "${pid}" || return 1

  if [ -f "${PID_START_FILE}" ]; then
    recorded_start_time=$(tr -d '[:space:]' < "${PID_START_FILE}")
    actual_start_time=$(process_start_time "${pid}") || return 1
    [ -n "${recorded_start_time}" ] && [ "${recorded_start_time}" = "${actual_start_time}" ] || return 1
  fi
}

# 停止、卸载和 status 只接受 PID、启动时间、UID 和可执行文件均匹配的进程；
# is_running_pid 仅供兼容的运行态检查使用。
is_managed_server_instance() {
  pid="$1"
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  process_is_alive "${pid}" || return 1
  [ -f "${PID_START_FILE}" ] || return 1

  recorded_start_time=$(tr -d '[:space:]' < "${PID_START_FILE}")
  actual_start_time=$(process_start_time "${pid}") || return 1
  [ -n "${recorded_start_time}" ] && [ "${recorded_start_time}" = "${actual_start_time}" ] || return 1
  process_matches_current_uid "${pid}" || return 1
  process_matches_executable "${pid}" "${SERVER_BIN}"
}

is_unverifiable_server_instance() {
  pid="$1"
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  process_is_alive "${pid}" || return 1
  process_matches_current_uid "${pid}" || return 1
  [ ! -f "${PID_START_FILE}" ] || return 1
  process_matches_executable "${pid}" "${SERVER_BIN}"
}

# 新启动的后台进程可能仍处于 nohup -> server 的 exec 窗口。此时可执行文件尚不匹配，
# 但 PID 与启动时间仍能证明它是本次启动创建的同一进程实例。
is_recorded_process_instance() {
  pid="$1"
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "${pid}" 2>/dev/null || return 1
  [ -f "${PID_START_FILE}" ] || return 1

  recorded_start_time=$(tr -d '[:space:]' < "${PID_START_FILE}")
  actual_start_time=$(process_start_time "${pid}") || return 1
  [ -n "${recorded_start_time}" ] && [ "${recorded_start_time}" = "${actual_start_time}" ]
}

wait_for_server_identity() {
  pid="$1"
  attempt=1
  while [ "${attempt}" -le "${PROCESS_IDENTITY_ATTEMPTS}" ]; do
    if is_running_pid "${pid}"; then
      return 0
    fi
    if ! is_recorded_process_instance "${pid}"; then
      return 1
    fi
    if [ "${attempt}" -lt "${PROCESS_IDENTITY_ATTEMPTS}" ]; then
      sleep "${PROCESS_IDENTITY_RETRY_SECONDS}"
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

wait_for_recorded_process_exit() {
  pid="$1"
  attempt=1
  while [ "${attempt}" -le "${START_CLEANUP_ATTEMPTS}" ]; do
    if ! is_recorded_process_instance "${pid}"; then
      return 0
    fi
    if [ "${attempt}" -lt "${START_CLEANUP_ATTEMPTS}" ]; then
      sleep "${START_CLEANUP_RETRY_SECONDS}"
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

terminate_recorded_process() {
  pid="$1"
  if ! is_recorded_process_instance "${pid}"; then
    return 0
  fi

  kill -TERM "${pid}" 2>/dev/null || true
  if wait_for_recorded_process_exit "${pid}"; then
    return 0
  fi

  kill -KILL "${pid}" 2>/dev/null || true
  wait_for_recorded_process_exit "${pid}"
}

wait_for_managed_server_exit() {
  pid="$1"
  attempts="$2"
  attempt=1
  while [ "${attempt}" -le "${attempts}" ]; do
    if ! is_managed_server_instance "${pid}"; then
      return 0
    fi
    if [ "${attempt}" -lt "${attempts}" ]; then
      sleep "${STOP_RETRY_SECONDS}"
    fi
    attempt=$((attempt + 1))
  done
  ! is_managed_server_instance "${pid}"
}

stop_managed_server_stage() {
  pid="$1"
  signal_name="$2"
  attempts="$3"

  if ! is_managed_server_instance "${pid}"; then
    return 0
  fi

  log_msg "向 motrix-fnos-server 发送 ${signal_name}，PID ${pid}"
  if ! kill "-${signal_name#SIG}" "${pid}" 2>/dev/null; then
    if ! is_managed_server_instance "${pid}"; then
      return 0
    fi
    log_msg "向 motrix-fnos-server 发送 ${signal_name} 失败，PID ${pid}"
    return 1
  fi

  if wait_for_managed_server_exit "${pid}" "${attempts}"; then
    log_msg "motrix-fnos-server 在 ${signal_name} 阶段后已退出，PID ${pid}"
    return 0
  fi

  log_msg "motrix-fnos-server 在 ${signal_name} 阶段超时，PID ${pid}"
  return 1
}

read_runtime_json_number() {
  field_name="$1"
  [ -r "${ARIA2_RUNTIME_FILE}" ] || return 1
  sed -n "s/^[[:space:]]*\"${field_name}\"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\)[[:space:]]*,\{0,1\}[[:space:]]*$/\1/p" "${ARIA2_RUNTIME_FILE}" | sed -n '1p'
}

read_runtime_json_string() {
  field_name="$1"
  [ -r "${ARIA2_RUNTIME_FILE}" ] || return 1
  sed -n "s/^[[:space:]]*\"${field_name}\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\"[[:space:]]*,\{0,1\}[[:space:]]*$/\1/p" "${ARIA2_RUNTIME_FILE}" | sed -n '1p'
}

aria2_runtime_pid() {
  read_runtime_json_number "pid"
}

aria2_runtime_process_is_alive() {
  pid=$(aria2_runtime_pid || true)
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "${pid}" 2>/dev/null
}

aria2_runtime_process_is_owned() {
  pid=$(aria2_runtime_pid || true)
  port=$(read_runtime_json_number "actualPort" || true)
  secret=$(read_runtime_json_string "rpcSecret" || true)
  case "${pid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  case "${port}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  [ -n "${secret}" ] || return 1
  process_is_alive "${pid}" || return 1
  process_matches_current_uid "${pid}" || return 1
  process_matches_executable "${pid}" "${ARIA2_BIN}" || return 1

  recorded_start_time=$(read_runtime_json_number "processStartTime" || true)
  case "${recorded_start_time}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  actual_start_time=$(process_start_time "${pid}" || true)
  [ -n "${actual_start_time}" ] && [ "${recorded_start_time}" = "${actual_start_time}" ] || return 1
  recorded_uid=$(read_runtime_json_number "processUid" || true)
  case "${recorded_uid}" in
    ''|*[!0-9]*) return 1 ;;
  esac
  actual_uid=$(process_uid "${pid}" || true)
  [ -n "${actual_uid}" ] && [ "${recorded_uid}" = "${actual_uid}" ] || return 1

  command_line_file="${PROC_ROOT}/${pid}/cmdline"
  [ -r "${command_line_file}" ] || return 1
  command_line=$(tr '\000' ' ' < "${command_line_file}")
  case "${command_line}" in
    *"--rpc-listen-port=${port}"*"--rpc-secret=${secret}"*) return 0 ;;
    *) return 1 ;;
  esac
}

wait_for_owned_aria2_exit() {
  attempts="$1"
  attempt=1
  while [ "${attempt}" -le "${attempts}" ]; do
    if ! aria2_runtime_process_is_owned; then
      return 0
    fi
    if [ "${attempt}" -lt "${attempts}" ]; then
      sleep "${STOP_RETRY_SECONDS}"
    fi
    attempt=$((attempt + 1))
  done
  ! aria2_runtime_process_is_owned
}

stop_owned_aria2_sidecar() {
  if [ ! -e "${ARIA2_RUNTIME_FILE}" ]; then
    return 0
  fi

  if ! aria2_runtime_process_is_alive; then
    rm -f "${ARIA2_RUNTIME_FILE}"
    log_msg "Aria2 运行态已无存活进程，已清理运行态记录"
    return 0
  fi

  pid=$(aria2_runtime_pid || true)
  if ! aria2_runtime_process_is_owned; then
    log_msg "Aria2 运行态 PID ${pid:-未知} 仍存活但归属无法确认，拒绝发送信号"
    return 1
  fi

  log_msg "向已确认归属的 Aria2 sidecar 发送 SIGTERM，PID ${pid}"
  kill -TERM "${pid}" 2>/dev/null || true
  if ! wait_for_owned_aria2_exit "${STOP_TERM_ATTEMPTS}"; then
    if ! aria2_runtime_process_is_owned; then
      rm -f "${ARIA2_RUNTIME_FILE}"
      return 0
    fi
    log_msg "Aria2 sidecar 在 SIGTERM 阶段超时，发送 SIGKILL，PID ${pid}"
    kill -KILL "${pid}" 2>/dev/null || true
    if ! wait_for_owned_aria2_exit "${STOP_KILL_ATTEMPTS}"; then
      log_msg "Aria2 sidecar 在 SIGKILL 阶段后仍存活，PID ${pid}"
      return 1
    fi
  fi

  rm -f "${ARIA2_RUNTIME_FILE}"
  log_msg "Aria2 sidecar 已停止，PID ${pid}"
}

ensure_uninstall_processes_stopped() {
  pid=$(read_pid || true)
  if [ -n "${pid}" ]; then
    if is_managed_server_instance "${pid}" || is_unverifiable_server_instance "${pid}"; then
      return 1
    fi
  fi

  if [ -e "${ARIA2_RUNTIME_FILE}" ] && aria2_runtime_process_is_alive; then
    return 1
  fi

  return 0
}

readiness_url() {
  readiness_host=${HTTP_ADDR%:*}
  readiness_port=${HTTP_ADDR##*:}
  case "${readiness_port}" in
    ''|*[!0-9]*) return 1 ;;
  esac

  case "${readiness_host}" in
    0.0.0.0) readiness_host="127.0.0.1" ;;
    '[::]') readiness_host="[::1]" ;;
  esac

  printf 'http://%s:%s/api/app/ready\n' "${readiness_host}" "${readiness_port}"
}

readiness_request() {
  readiness_endpoint=$(readiness_url) || return 1

  if command -v "${CURL_BIN}" >/dev/null 2>&1; then
    readiness_status=$("${CURL_BIN}" \
      --silent \
      --output /dev/null \
      --write-out '%{http_code}' \
      --connect-timeout "${READINESS_REQUEST_TIMEOUT_SECONDS}" \
      --max-time "${READINESS_REQUEST_TIMEOUT_SECONDS}" \
      "${readiness_endpoint}" 2>/dev/null || true)
    [ "${readiness_status}" = "200" ]
    return
  fi

  if command -v "${WGET_BIN}" >/dev/null 2>&1; then
    readiness_response=$("${WGET_BIN}" \
      -S \
      -T "${READINESS_REQUEST_TIMEOUT_SECONDS}" \
      -O /dev/null \
      "${readiness_endpoint}" 2>&1 || true)
    printf '%s\n' "${readiness_response}" | awk '
      /^[[:space:]]*HTTP\/[0-9.]+ [0-9][0-9][0-9]/ { status = $2 }
      END { exit status == 200 ? 0 : 1 }
    '
    return
  fi

  return 127
}

wait_for_server_ready() {
  pid="$1"
  attempt=1
  while [ "${attempt}" -le "${READINESS_ATTEMPTS}" ]; do
    if ! is_running_pid "${pid}"; then
      return 1
    fi
    if readiness_request; then
      return 0
    fi
    if [ "${attempt}" -lt "${READINESS_ATTEMPTS}" ]; then
      sleep "${READINESS_RETRY_SECONDS}"
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

clear_stale_pid() {
  pid=$(read_pid || true)
  if [ -f "${PID_FILE}" ] && { [ -z "${pid}" ] || ! is_running_pid "${pid}"; }; then
    remove_pid_record
  fi
}

stop_managed_server_for_startup() {
  pid="$1"
  if ! stop_managed_server_stage "${pid}" "SIGINT" "${STOP_INT_ATTEMPTS}"; then
    if ! stop_managed_server_stage "${pid}" "SIGTERM" "${STOP_TERM_ATTEMPTS}"; then
      if ! stop_managed_server_stage "${pid}" "SIGKILL" "${STOP_KILL_ATTEMPTS}"; then
        log_msg "启动对账无法停止已确认归属的 motrix-fnos-server，PID ${pid}"
        return 1
      fi
    fi
  fi
  remove_pid_record
  log_msg "启动对账已清理 motrix-fnos-server 孤儿进程，PID ${pid}"
}

reconcile_startup_aria2_orphan() {
  [ -e "${ARIA2_RUNTIME_FILE}" ] || return 0

  pid=$(aria2_runtime_pid || true)
  case "${pid}" in
    ''|*[!0-9]*)
      log_msg "Aria2 运行态记录缺少可核对的 PID，拒绝启动并保留记录"
      return 1
      ;;
  esac

  if ! process_is_alive "${pid}"; then
    rm -f "${ARIA2_RUNTIME_FILE}"
    log_msg "启动对账发现 Aria2 进程已退出，已清理陈旧运行态记录"
    return 0
  fi

  if ! aria2_runtime_process_is_owned; then
    log_msg "启动对账无法证明 Aria2 PID ${pid} 属于本应用，拒绝发送信号并保留运行态记录"
    return 1
  fi

  if ! stop_owned_aria2_sidecar; then
    log_msg "启动对账清理 Aria2 孤儿进程失败，保留运行态记录"
    return 1
  fi
  log_msg "启动对账已清理 Aria2 孤儿进程，PID ${pid}"
}

reconcile_startup_orphans() {
  pid=$(read_pid || true)
  if [ -n "${pid}" ]; then
    if process_is_alive "${pid}"; then
      if ! is_managed_server_instance "${pid}"; then
        log_msg "启动对账无法证明 server PID ${pid} 属于本应用，拒绝启动并保留运行态记录"
        return 1
      fi

      if readiness_request; then
        # 就绪实例仍由当前 server 使用，不能把它的 Aria2 runtime 当作孤儿清理。
        return 0
      else
        readiness_status=$?
      fi
      if [ "${readiness_status}" -eq 127 ]; then
        log_msg "启动对账无法执行 server 就绪探测，拒绝停止 PID ${pid}"
        return 1
      fi

      log_msg "启动对账发现 server PID ${pid} 身份匹配但服务未就绪，开始有界回收"
      stop_managed_server_for_startup "${pid}" || return 1
    else
      remove_pid_record
      log_msg "启动对账发现 server PID ${pid} 已退出，已清理陈旧运行态记录"
    fi
  elif [ -f "${PID_START_FILE}" ]; then
    rm -f "${PID_START_FILE}"
  fi

  reconcile_startup_aria2_orphan
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
  rotate_lifecycle_log
  printf "%s %s\n" "$(date "+%Y-%m-%d %H:%M:%S")" "$1" >> "${LIFECYCLE_LOG}"
}

rotate_lifecycle_log() {
  [ -f "${LIFECYCLE_LOG}" ] || return 0
  lifecycle_log_size=$(wc -c < "${LIFECYCLE_LOG}" | tr -d '[:space:]')
  case "${lifecycle_log_size}" in
    ''|*[!0-9]*) return 0 ;;
  esac
  if [ "${lifecycle_log_size}" -lt "${LIFECYCLE_LOG_MAX_BYTES}" ]; then
    return 0
  fi

  lifecycle_index=${LIFECYCLE_LOG_RETENTION}
  while [ "${lifecycle_index}" -gt 0 ]; do
    lifecycle_source="${LIFECYCLE_LOG}.${lifecycle_index}"
    lifecycle_target="${LIFECYCLE_LOG}.$((lifecycle_index + 1))"
    if [ -f "${lifecycle_source}" ]; then
      if [ "${lifecycle_index}" -eq "${LIFECYCLE_LOG_RETENTION}" ]; then
        rm -f "${lifecycle_source}"
      else
        mv "${lifecycle_source}" "${lifecycle_target}"
      fi
    fi
    lifecycle_index=$((lifecycle_index - 1))
  done
  mv "${LIFECYCLE_LOG}" "${LIFECYCLE_LOG}.1"
  : > "${LIFECYCLE_LOG}"
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
