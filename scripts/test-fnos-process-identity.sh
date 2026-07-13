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
: > "${SERVER_FIXTURE}"
: > "${OTHER_FIXTURE}"
chmod +x "${SERVER_FIXTURE}" "${OTHER_FIXTURE}"
ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
printf '%s\n' "$$ (motrix-fnos-server) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4242" > "${PROC_FIXTURE}/$$/stat"

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_PROC_ROOT TRIM_PKGVAR

. "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"

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

echo "FPK 进程身份校验测试通过。"
