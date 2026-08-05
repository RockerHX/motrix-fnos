#!/bin/sh

set -eu

TEST_ROOT=$(mktemp -d)
SERVER_PID=""
SIDECAR_PID=""
cleanup() {
  for process_pid in "${SERVER_PID}" "${SIDECAR_PID}"; do
    if [ -n "${process_pid}" ]; then
      kill -KILL "${process_pid}" 2>/dev/null || true
      wait "${process_pid}" 2>/dev/null || true
    fi
  done
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT INT TERM

SERVER_FIXTURE="${TEST_ROOT}/motrix-fnos-server"
ARIA2_FIXTURE="${TEST_ROOT}/aria2-next"
OTHER_FIXTURE="${TEST_ROOT}/other-process"
PROC_FIXTURE="${TEST_ROOT}/proc"
READY_FILE="${TEST_ROOT}/ready"
UNINSTALL_LOG="${TEST_ROOT}/uninstall.log"
mkdir -p "${TEST_ROOT}/data" "${PROC_FIXTURE}"
: > "${SERVER_FIXTURE}"
: > "${ARIA2_FIXTURE}"
: > "${OTHER_FIXTURE}"
chmod +x "${SERVER_FIXTURE}" "${ARIA2_FIXTURE}" "${OTHER_FIXTURE}"

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_ARIA2_PATH="${ARIA2_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
MOTRIX_FNOS_STOP_INT_ATTEMPTS=1
MOTRIX_FNOS_STOP_TERM_ATTEMPTS=1
MOTRIX_FNOS_STOP_KILL_ATTEMPTS=2
MOTRIX_FNOS_STOP_RETRY_SECONDS=0.05
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_ARIA2_PATH MOTRIX_FNOS_PROC_ROOT TRIM_PKGVAR
export MOTRIX_FNOS_STOP_INT_ATTEMPTS MOTRIX_FNOS_STOP_TERM_ATTEMPTS MOTRIX_FNOS_STOP_KILL_ATTEMPTS
export MOTRIX_FNOS_STOP_RETRY_SECONDS

. "$(dirname -- "$0")/../../packaging/fnos/cmd/common.sh"

spawn_signal_ignoring_process() {
  ready_file="$1"
  node -e '
const fs = require("node:fs");
process.on("SIGINT", () => {});
process.on("SIGTERM", () => {});
fs.writeFileSync(process.env.MOTRIX_FNOS_TEST_READY_FILE, "ready");
setInterval(() => {}, 1_000);
' >/dev/null 2>&1 &
  process_pid=$!
  wait_attempt=1
  while [ ! -f "${ready_file}" ] && [ "${wait_attempt}" -le 40 ]; do
    sleep 0.05
    wait_attempt=$((wait_attempt + 1))
  done
  test -f "${ready_file}"
  printf '%s\n' "${process_pid}"
}

write_proc_fixture() {
  process_pid="$1"
  executable="$2"
  start_time="$3"
  command_line="${4:-}"
  mkdir -p "${PROC_FIXTURE}/${process_pid}"
  ln -s "${executable}" "${PROC_FIXTURE}/${process_pid}/exe"
  printf '%s\n' "${process_pid} (motrix-fnos-server) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ${start_time}" > "${PROC_FIXTURE}/${process_pid}/stat"
  if [ -n "${command_line}" ]; then
    printf '%s\000' ${command_line} > "${PROC_FIXTURE}/${process_pid}/cmdline"
  fi
}

export MOTRIX_FNOS_TEST_READY_FILE="${READY_FILE}"
SERVER_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture "${SERVER_PID}" "${SERVER_FIXTURE}" "4242"
prepare_runtime_dirs
printf '%s\n' "${SERVER_PID}" > "${PID_FILE}"
printf '%s\n' "4242" > "${PID_START_FILE}"

"$(dirname -- "$0")/../../packaging/fnos/cmd/stop" > "${TEST_ROOT}/stop-output" 2>&1
if kill -0 "${SERVER_PID}" 2>/dev/null; then
  echo "SIGINT 和 SIGTERM 被忽略时 stop 必须升级到 SIGKILL" >&2
  exit 1
fi
wait "${SERVER_PID}" 2>/dev/null || true
SERVER_PID=""
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"
grep -q "SIGINT" "${LIFECYCLE_LOG}"
grep -q "SIGTERM" "${LIFECYCLE_LOG}"
grep -q "SIGKILL" "${LIFECYCLE_LOG}"

rm -f "${READY_FILE}"
export MOTRIX_FNOS_TEST_READY_FILE="${READY_FILE}"
SERVER_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture "${SERVER_PID}" "${SERVER_FIXTURE}" "4343"
printf '%s\n' "${SERVER_PID}" > "${PID_FILE}"
printf '%s\n' "4343" > "${PID_START_FILE}"
rm -f "${READY_FILE}"
export MOTRIX_FNOS_TEST_READY_FILE="${READY_FILE}"
SIDECAR_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture \
  "${SIDECAR_PID}" \
  "${ARIA2_FIXTURE}" \
  "5252" \
  "${ARIA2_FIXTURE} --rpc-listen-port=16800 --rpc-secret=test-secret"
cat > "${ARIA2_RUNTIME_FILE}" <<EOF
{
  "pid": ${SIDECAR_PID},
  "actualPort": 16800,
  "rpcSecret": "test-secret"
}
EOF
"$(dirname -- "$0")/../../packaging/fnos/cmd/stop" > "${TEST_ROOT}/sidecar-stop-output" 2>&1
if kill -0 "${SERVER_PID}" 2>/dev/null || kill -0 "${SIDECAR_PID}" 2>/dev/null; then
  echo "stop 必须收敛可证明归属的 server 与 Aria2 sidecar" >&2
  exit 1
fi
wait "${SERVER_PID}" 2>/dev/null || true
wait "${SIDECAR_PID}" 2>/dev/null || true
SERVER_PID=""
SIDECAR_PID=""
test ! -e "${ARIA2_RUNTIME_FILE}"

rm -f "${READY_FILE}"
export MOTRIX_FNOS_TEST_READY_FILE="${READY_FILE}"
SERVER_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture "${SERVER_PID}" "${SERVER_FIXTURE}" "5151"
printf '%s\n' "${SERVER_PID}" > "${PID_FILE}"
printf '%s\n' "9999" > "${PID_START_FILE}"
"$(dirname -- "$0")/../../packaging/fnos/cmd/stop" > "${TEST_ROOT}/stale-output" 2>&1
if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
  echo "启动时间不匹配时 stop 不得终止 PID 复用进程" >&2
  exit 1
fi
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"
kill -KILL "${SERVER_PID}"
wait "${SERVER_PID}" 2>/dev/null || true
SERVER_PID=""

rm -f "${READY_FILE}"
export MOTRIX_FNOS_TEST_READY_FILE="${READY_FILE}"
SERVER_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture "${SERVER_PID}" "${SERVER_FIXTURE}" "7171"
printf '%s\n' "${SERVER_PID}" > "${PID_FILE}"
if "$(dirname -- "$0")/../../packaging/fnos/cmd/stop" > "${TEST_ROOT}/missing-start-output" 2>&1; then
  echo "缺少启动时间时 stop 必须失败关闭" >&2
  exit 1
fi
if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
  echo "缺少启动时间时 stop 不得终止进程" >&2
  exit 1
fi
test -e "${PID_FILE}"
if TRIM_TEMP_LOGFILE="${UNINSTALL_LOG}" "$(dirname -- "$0")/../../packaging/fnos/cmd/uninstall_init"; then
  echo "stop 失败时 uninstall_init 必须拒绝继续" >&2
  exit 1
fi
grep -q "拒绝继续卸载" "${UNINSTALL_LOG}"

mkdir -p "${TRIM_PKGVAR}/keep"
printf '%s\n' "keep" > "${TRIM_PKGVAR}/keep/value"
printf '%s\n' "7171" > "${PID_START_FILE}"
if TRIM_TEMP_LOGFILE="${UNINSTALL_LOG}" MOTRIX_FNOS_DELETE_APP_DATA=true \
  "$(dirname -- "$0")/../../packaging/fnos/cmd/uninstall_callback"; then
  echo "运行态 PID 存活时 uninstall_callback 必须拒绝清理数据" >&2
  exit 1
fi
test -f "${TRIM_PKGVAR}/keep/value"
kill -KILL "${SERVER_PID}"
wait "${SERVER_PID}" 2>/dev/null || true
SERVER_PID=""

echo "FPK 停止卸载收敛测试通过。"
