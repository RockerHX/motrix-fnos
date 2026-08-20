#!/bin/sh

set -eu

TEST_ROOT=$(mktemp -d)
ORPHAN_PID=""
cleanup() {
  if [ -n "${ORPHAN_PID}" ]; then
    kill -KILL "${ORPHAN_PID}" 2>/dev/null || true
    wait "${ORPHAN_PID}" 2>/dev/null || true
  fi
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT INT TERM

SERVER_FIXTURE="${TEST_ROOT}/motrix-fnos-server"
OTHER_FIXTURE="${TEST_ROOT}/other-process"
PROC_FIXTURE="${TEST_ROOT}/proc"
mkdir -p "${PROC_FIXTURE}/$$" "${TEST_ROOT}/data"
SERVER_CALLS="${TEST_ROOT}/server-calls"
cat > "${SERVER_FIXTURE}" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "${MOTRIX_FNOS_TEST_SERVER_CALLS}"
EOF
: > "${OTHER_FIXTURE}"
chmod +x "${SERVER_FIXTURE}" "${OTHER_FIXTURE}"
ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
printf '%s\n' "$$ (motrix-fnos-server) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4242" > "${PROC_FIXTURE}/$$/stat"
printf 'Name:\tmotrix-fnos\nUid:\t%s\t%s\t%s\t%s\n' "$(id -u)" "$(id -u)" "$(id -u)" "$(id -u)" > "${PROC_FIXTURE}/$$/status"

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
MOTRIX_FNOS_TEST_SERVER_CALLS="${SERVER_CALLS}"
MOTRIX_FNOS_PROCESS_IDENTITY_ATTEMPTS=10
MOTRIX_FNOS_PROCESS_IDENTITY_RETRY_SECONDS=0.05
MOTRIX_FNOS_START_CLEANUP_ATTEMPTS=10
MOTRIX_FNOS_START_CLEANUP_RETRY_SECONDS=0.05
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_PROC_ROOT TRIM_PKGVAR MOTRIX_FNOS_TEST_SERVER_CALLS
export MOTRIX_FNOS_PROCESS_IDENTITY_ATTEMPTS MOTRIX_FNOS_PROCESS_IDENTITY_RETRY_SECONDS
export MOTRIX_FNOS_START_CLEANUP_ATTEMPTS MOTRIX_FNOS_START_CLEANUP_RETRY_SECONDS

. "$(dirname -- "$0")/../../packaging/fnos/cmd/common.sh"

test "${HTTP_ADDR}" = "0.0.0.0:17080"
test "${JSONRPC_ADDR}" = "127.0.0.1:17081"
test "${LAN_JSONRPC_ADDR}" = "0.0.0.0:17082"
export_runtime_env
test "${MOTRIX_FNOS_HTTP_ADDR}" = "0.0.0.0:17080"
test "${MOTRIX_FNOS_JSONRPC_ADDR}" = "127.0.0.1:17081"
test "${MOTRIX_FNOS_LAN_JSONRPC_ADDR}" = "0.0.0.0:17082"

(
  MOTRIX_FNOS_HTTP_ADDR="127.0.0.1:27080"
  MOTRIX_FNOS_JSONRPC_ADDR="127.0.0.1:27081"
  MOTRIX_FNOS_LAN_JSONRPC_ADDR="127.0.0.1:27082"
  export MOTRIX_FNOS_HTTP_ADDR MOTRIX_FNOS_JSONRPC_ADDR MOTRIX_FNOS_LAN_JSONRPC_ADDR
  . "$(dirname -- "$0")/../../packaging/fnos/cmd/common.sh"
  export_runtime_env
  test "${HTTP_ADDR}" = "127.0.0.1:27080"
  test "${JSONRPC_ADDR}" = "127.0.0.1:27081"
  test "${LAN_JSONRPC_ADDR}" = "0.0.0.0:17082"
  test "${MOTRIX_FNOS_HTTP_ADDR}" = "127.0.0.1:27080"
  test "${MOTRIX_FNOS_JSONRPC_ADDR}" = "127.0.0.1:27081"
  test "${MOTRIX_FNOS_LAN_JSONRPC_ADDR}" = "0.0.0.0:17082"
)

prepare_runtime_dirs
write_pid_record "$$"
is_running_pid "$$"

rm "${PROC_FIXTURE}/$$/status"
if is_running_pid "$$"; then
  echo "无法读取进程 UID 时不应识别为 Motrix 进程" >&2
  exit 1
fi
test_process_uid=$(id -u)
foreign_uid=$((test_process_uid + 1))
printf 'Name:\tmotrix-fnos\nUid:\t%s\t%s\t%s\t%s\n' "${foreign_uid}" "${foreign_uid}" "${foreign_uid}" "${foreign_uid}" > "${PROC_FIXTURE}/$$/status"
if is_running_pid "$$"; then
  echo "进程 UID 不匹配时不应识别为 Motrix 进程" >&2
  exit 1
fi
printf 'Name:\tmotrix-fnos\nUid:\t%s\t%s\t%s\t%s\n' "$(id -u)" "$(id -u)" "$(id -u)" "$(id -u)" > "${PROC_FIXTURE}/$$/status"
is_running_pid "$$"

rm "${PROC_FIXTURE}/$$/exe"
ln -s "${OTHER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
is_recorded_process_instance "$$"
(
  sleep 0.1
  rm "${PROC_FIXTURE}/$$/exe"
  ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
) &
identity_transition_pid=$!
wait_for_server_identity "$$"
wait "${identity_transition_pid}"
is_running_pid "$$"

printf '%s\n' "9999" > "${PID_START_FILE}"
if is_running_pid "$$"; then
  echo "启动时间不匹配时不应识别为 Motrix 进程" >&2
  exit 1
fi

printf '%s\n' "4242" > "${PID_START_FILE}"
rm "${PROC_FIXTURE}/$$/exe"
ln -s "${OTHER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
if is_running_pid "$$"; then
  echo "可执行文件不匹配时不应识别为 Motrix 进程" >&2
  exit 1
fi

rm "${PROC_FIXTURE}/$$/exe"
ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
rm "${PID_START_FILE}"
is_running_pid "$$"

sleep 30 &
ORPHAN_PID=$!
mkdir -p "${PROC_FIXTURE}/${ORPHAN_PID}"
ln -s "${OTHER_FIXTURE}" "${PROC_FIXTURE}/${ORPHAN_PID}/exe"
printf '%s\n' "${ORPHAN_PID} (nohup) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 5151" > "${PROC_FIXTURE}/${ORPHAN_PID}/stat"
printf 'Name:\tnohup\nUid:\t%s\t%s\t%s\t%s\n' "$(id -u)" "$(id -u)" "$(id -u)" "$(id -u)" > "${PROC_FIXTURE}/${ORPHAN_PID}/status"
printf '%s\n' "${ORPHAN_PID}" > "${PID_FILE}"
printf '%s\n' "5151" > "${PID_START_FILE}"
if is_running_pid "${ORPHAN_PID}"; then
  echo "nohup exec 前不应通过严格可执行文件校验" >&2
  exit 1
fi
is_recorded_process_instance "${ORPHAN_PID}"
terminate_recorded_process "${ORPHAN_PID}"
if kill -0 "${ORPHAN_PID}" 2>/dev/null; then
  echo "启动失败清理必须终止仍处于 nohup exec 窗口的同一进程" >&2
  exit 1
fi
wait "${ORPHAN_PID}" 2>/dev/null || true
ORPHAN_PID=""
remove_pid_record

write_pid_record "$$"
printf '%s\n' "9999" > "${PID_START_FILE}"
clear_stale_pid
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"

write_pid_record "$$"
if "$(dirname -- "$0")/../../packaging/fnos/cmd/reset-web-auth" >/dev/null 2>&1; then
  echo "server 运行时不应允许重置 Web 鉴权" >&2
  exit 1
fi
test ! -e "${SERVER_CALLS}"

printf '%s\n' "9999" > "${PID_START_FILE}"
"$(dirname -- "$0")/../../packaging/fnos/cmd/reset-web-auth" >/dev/null
grep -qx 'reset-web-auth' "${SERVER_CALLS}"
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"

echo "FPK 进程身份校验测试通过。"
