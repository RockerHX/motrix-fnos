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
READY_STATUS_FILE="${TEST_ROOT}/ready-status"
READY_FILE="${TEST_ROOT}/ready"
mkdir -p "${TEST_ROOT}/data" "${PROC_FIXTURE}"
: > "${SERVER_FIXTURE}"
: > "${ARIA2_FIXTURE}"
: > "${OTHER_FIXTURE}"
chmod +x "${SERVER_FIXTURE}" "${ARIA2_FIXTURE}" "${OTHER_FIXTURE}"
printf '%s\n' "503" > "${READY_STATUS_FILE}"

cat > "${TEST_ROOT}/curl" <<'EOF'
#!/bin/sh
cat "${MOTRIX_FNOS_TEST_READY_STATUS_FILE}"
EOF
chmod +x "${TEST_ROOT}/curl"

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_ARIA2_PATH="${ARIA2_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
MOTRIX_FNOS_CURL_BIN="${TEST_ROOT}/curl"
MOTRIX_FNOS_TEST_READY_STATUS_FILE="${READY_STATUS_FILE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
MOTRIX_FNOS_STOP_INT_ATTEMPTS=1
MOTRIX_FNOS_STOP_TERM_ATTEMPTS=1
MOTRIX_FNOS_STOP_KILL_ATTEMPTS=2
MOTRIX_FNOS_STOP_RETRY_SECONDS=0.05
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_ARIA2_PATH MOTRIX_FNOS_PROC_ROOT
export MOTRIX_FNOS_CURL_BIN MOTRIX_FNOS_TEST_READY_STATUS_FILE TRIM_PKGVAR
export MOTRIX_FNOS_STOP_INT_ATTEMPTS MOTRIX_FNOS_STOP_TERM_ATTEMPTS
export MOTRIX_FNOS_STOP_KILL_ATTEMPTS MOTRIX_FNOS_STOP_RETRY_SECONDS

. "$(dirname -- "$0")/../../packaging/fnos/cmd/common.sh"

spawn_signal_ignoring_process() {
  ready_file="$1"
  MOTRIX_FNOS_TEST_READY_FILE="${ready_file}" \
    node -e '
const fs = require("node:fs");
process.on("SIGINT", () => {});
process.on("SIGTERM", () => {});
fs.writeFileSync(process.env.MOTRIX_FNOS_TEST_READY_FILE, "ready");
setInterval(() => {}, 1_000);
' >/dev/null 2>&1 &
  process_pid=$!
  attempt=1
  while [ ! -f "${ready_file}" ] && [ "${attempt}" -le 40 ]; do
    sleep 0.05
    attempt=$((attempt + 1))
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
  stat_line="${process_pid} (motrix-fnos-server) S"
  stat_field=4
  while [ "${stat_field}" -lt 22 ]; do
    stat_line="${stat_line} 0"
    stat_field=$((stat_field + 1))
  done
  printf '%s %s\n' "${stat_line}" "${start_time}" > "${PROC_FIXTURE}/${process_pid}/stat"
  printf 'Name:\tmotrix-fnos\nUid:\t%s\t%s\t%s\t%s\n' "$(id -u)" "$(id -u)" "$(id -u)" "$(id -u)" > "${PROC_FIXTURE}/${process_pid}/status"
  if [ -n "${command_line}" ]; then
    printf '%s\000' ${command_line} > "${PROC_FIXTURE}/${process_pid}/cmdline"
  fi
}

prepare_runtime_dirs

rm -f "${READY_FILE}"
SERVER_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture "${SERVER_PID}" "${SERVER_FIXTURE}" "4242"
printf '%s\n' "${SERVER_PID}" > "${PID_FILE}"
printf '%s\n' "4242" > "${PID_START_FILE}"

reconcile_startup_orphans
if kill -0 "${SERVER_PID}" 2>/dev/null; then
  echo "启动对账必须清理已确认归属且未就绪的 server" >&2
  exit 1
fi
wait "${SERVER_PID}" 2>/dev/null || true
SERVER_PID=""
test ! -e "${PID_FILE}"
test ! -e "${PID_START_FILE}"

rm -f "${READY_FILE}"
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
  "rpcSecret": "test-secret",
  "processStartTime": 5252,
  "processUid": $(id -u)
}
EOF

reconcile_startup_orphans
if kill -0 "${SIDECAR_PID}" 2>/dev/null; then
  echo "启动对账必须清理已确认归属的 Aria2 孤儿进程" >&2
  exit 1
fi
wait "${SIDECAR_PID}" 2>/dev/null || true
SIDECAR_PID=""
test ! -e "${ARIA2_RUNTIME_FILE}"

rm -f "${READY_FILE}"
SIDECAR_PID=$(spawn_signal_ignoring_process "${READY_FILE}")
write_proc_fixture \
  "${SIDECAR_PID}" \
  "${OTHER_FIXTURE}" \
  "6262" \
  "${OTHER_FIXTURE} --rpc-listen-port=16800 --rpc-secret=test-secret"
cat > "${ARIA2_RUNTIME_FILE}" <<EOF
{
  "pid": ${SIDECAR_PID},
  "actualPort": 16800,
  "rpcSecret": "test-secret",
  "processStartTime": 6262,
  "processUid": $(id -u)
}
EOF

if reconcile_startup_orphans; then
  echo "无法证明归属的 Aria2 孤儿必须拒绝启动" >&2
  exit 1
fi
if ! kill -0 "${SIDECAR_PID}" 2>/dev/null; then
  echo "无法证明归属时不得误杀 Aria2 进程" >&2
  exit 1
fi
test -e "${ARIA2_RUNTIME_FILE}"

echo "FPK 启动孤儿进程对账测试通过。"
