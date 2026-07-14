#!/bin/sh

set -eu

TEST_ROOT=$(mktemp -d)
cleanup() {
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

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
MOTRIX_FNOS_TEST_SERVER_CALLS="${SERVER_CALLS}"
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_PROC_ROOT TRIM_PKGVAR MOTRIX_FNOS_TEST_SERVER_CALLS

. "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"

test "${HTTP_ADDR}" = "0.0.0.0:17080"
test "${JSONRPC_ADDR}" = "127.0.0.1:17081"
export_runtime_env
test "${MOTRIX_FNOS_HTTP_ADDR}" = "0.0.0.0:17080"
test "${MOTRIX_FNOS_JSONRPC_ADDR}" = "127.0.0.1:17081"

(
  MOTRIX_FNOS_HTTP_ADDR="127.0.0.1:27080"
  MOTRIX_FNOS_JSONRPC_ADDR="127.0.0.1:27081"
  export MOTRIX_FNOS_HTTP_ADDR MOTRIX_FNOS_JSONRPC_ADDR
  . "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"
  export_runtime_env
  test "${HTTP_ADDR}" = "127.0.0.1:27080"
  test "${JSONRPC_ADDR}" = "127.0.0.1:27081"
  test "${MOTRIX_FNOS_HTTP_ADDR}" = "127.0.0.1:27080"
  test "${MOTRIX_FNOS_JSONRPC_ADDR}" = "127.0.0.1:27081"
)

prepare_runtime_dirs
write_pid_record "$$"
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

printf '%s\n' "9999" > "${PID_START_FILE}"
clear_stale_pid
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"

write_pid_record "$$"
if "$(dirname -- "$0")/../packaging/fnos/cmd/reset-web-auth" >/dev/null 2>&1; then
  echo "server 运行时不应允许重置 Web 鉴权" >&2
  exit 1
fi
test ! -e "${SERVER_CALLS}"

printf '%s\n' "9999" > "${PID_START_FILE}"
"$(dirname -- "$0")/../packaging/fnos/cmd/reset-web-auth" >/dev/null
grep -qx 'reset-web-auth' "${SERVER_CALLS}"
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"

echo "FPK 进程身份校验测试通过。"
