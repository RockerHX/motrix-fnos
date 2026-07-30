#!/bin/sh

set -eu

TEST_ROOT=$(mktemp -d)
cleanup() {
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT INT TERM

SERVER_FIXTURE="${TEST_ROOT}/motrix-fnos-server"
OTHER_FIXTURE="${TEST_ROOT}/other-process"
ARIA2_FIXTURE="${TEST_ROOT}/aria2-next"
ARIA2_CALLS="${TEST_ROOT}/aria2-calls"
PROC_FIXTURE="${TEST_ROOT}/proc"
READY_STATUS_FILE="${TEST_ROOT}/ready-status"
CURL_FIXTURE="${TEST_ROOT}/curl"
WGET_FIXTURE="${TEST_ROOT}/wget"
OUTPUT_FILE="${TEST_ROOT}/output"
SNAPSHOT_BEFORE="${TEST_ROOT}/snapshot-before"
SNAPSHOT_AFTER="${TEST_ROOT}/snapshot-after"
mkdir -p "${TEST_ROOT}/data" "${PROC_FIXTURE}"
: > "${SERVER_FIXTURE}"
: > "${OTHER_FIXTURE}"
cat > "${ARIA2_FIXTURE}" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "${MOTRIX_FNOS_TEST_ARIA2_CALLS}"
EOF
chmod +x "${SERVER_FIXTURE}" "${OTHER_FIXTURE}"
chmod +x "${ARIA2_FIXTURE}"

cat > "${CURL_FIXTURE}" <<'EOF'
#!/bin/sh
cat "${MOTRIX_FNOS_TEST_READY_STATUS_FILE}"
EOF
chmod +x "${CURL_FIXTURE}"

cat > "${WGET_FIXTURE}" <<'EOF'
#!/bin/sh
printf '  HTTP/1.1 %s Ready\n' "$(cat "${MOTRIX_FNOS_TEST_READY_STATUS_FILE}")" >&2
EOF
chmod +x "${WGET_FIXTURE}"

MOTRIX_FNOS_SERVER_BIN="${SERVER_FIXTURE}"
MOTRIX_FNOS_ARIA2_PATH="${ARIA2_FIXTURE}"
MOTRIX_FNOS_PROC_ROOT="${PROC_FIXTURE}"
TRIM_PKGVAR="${TEST_ROOT}/data"
MOTRIX_FNOS_CURL_BIN="${CURL_FIXTURE}"
MOTRIX_FNOS_WGET_BIN="${WGET_FIXTURE}"
MOTRIX_FNOS_READINESS_ATTEMPTS=2
MOTRIX_FNOS_READINESS_RETRY_SECONDS=0
MOTRIX_FNOS_READINESS_REQUEST_TIMEOUT_SECONDS=1
MOTRIX_FNOS_TEST_READY_STATUS_FILE="${READY_STATUS_FILE}"
MOTRIX_FNOS_TEST_ARIA2_CALLS="${ARIA2_CALLS}"
export MOTRIX_FNOS_SERVER_BIN MOTRIX_FNOS_ARIA2_PATH MOTRIX_FNOS_PROC_ROOT TRIM_PKGVAR
export MOTRIX_FNOS_CURL_BIN MOTRIX_FNOS_WGET_BIN MOTRIX_FNOS_READINESS_ATTEMPTS
export MOTRIX_FNOS_READINESS_RETRY_SECONDS MOTRIX_FNOS_READINESS_REQUEST_TIMEOUT_SECONDS
export MOTRIX_FNOS_TEST_READY_STATUS_FILE MOTRIX_FNOS_TEST_ARIA2_CALLS

mkdir -p "${PROC_FIXTURE}/$$"
ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
printf '%s\n' "$$ (motrix-fnos-server) S 1 1 1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4242" > "${PROC_FIXTURE}/$$/stat"

. "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"

run_script() {
  if "$@" > "${OUTPUT_FILE}" 2>&1; then
    script_status=0
  else
    script_status=$?
  fi
}

file_mtime() {
  if stat -c '%Y' "$1" >/dev/null 2>&1; then
    stat -c '%Y' "$1"
  else
    stat -f '%m' "$1"
  fi
}

snapshot_data_tree() {
  snapshot_file="$1"
  : > "${snapshot_file}"
  if [ ! -e "${TRIM_PKGVAR}" ]; then
    printf 'absent\n' >> "${snapshot_file}"
    return
  fi

  find "${TRIM_PKGVAR}" -print | LC_ALL=C sort | while IFS= read -r path; do
    relative_path=${path#"${TRIM_PKGVAR}"}
    if [ -f "${path}" ]; then
      checksum=$(cksum < "${path}")
      printf 'file|%s|%s|%s\n' "${relative_path}" "${checksum}" "$(file_mtime "${path}")"
    elif [ -d "${path}" ]; then
      printf 'dir|%s|%s\n' "${relative_path}" "$(file_mtime "${path}")"
    elif [ -L "${path}" ]; then
      printf 'link|%s|%s|%s\n' "${relative_path}" "$(readlink "${path}")" "$(file_mtime "${path}")"
    fi
  done >> "${snapshot_file}"
}

assert_status_keeps_data_tree_unchanged() {
  snapshot_data_tree "${SNAPSHOT_BEFORE}"
  "$@"
  snapshot_data_tree "${SNAPSHOT_AFTER}"
  cmp -s "${SNAPSHOT_BEFORE}" "${SNAPSHOT_AFTER}"
}

test "$(readiness_url)" = "http://127.0.0.1:17080/api/app/ready"
(
  MOTRIX_FNOS_HTTP_ADDR="[::]:27080"
  export MOTRIX_FNOS_HTTP_ADDR
  . "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"
  test "$(readiness_url)" = "http://[::1]:27080/api/app/ready"
)

printf '%s\n' "200" > "${READY_STATUS_FILE}"
readiness_request
(
  MOTRIX_FNOS_CURL_BIN="${TEST_ROOT}/missing-curl"
  export MOTRIX_FNOS_CURL_BIN
  . "$(dirname -- "$0")/../packaging/fnos/cmd/common.sh"
  readiness_request
)

test ! -e "${RUNTIME_DIR}"
test ! -e "${LOG_DIR}"
run_script "$(dirname -- "$0")/../packaging/fnos/cmd/status"
test "${script_status}" -eq 3
grep -q "未运行" "${OUTPUT_FILE}"
test ! -e "${RUNTIME_DIR}"
test ! -e "${LOG_DIR}"

prepare_runtime_dirs
write_pid_record "$$"
printf '%s\n' "lifecycle baseline" > "${LIFECYCLE_LOG}"
printf '%s\n' "server baseline" > "${SERVER_LOG}"

run_script "$(dirname -- "$0")/../packaging/fnos/cmd/start"
test "${script_status}" -eq 0
grep -q "已在运行且服务就绪" "${OUTPUT_FILE}"

snapshot_data_tree "${SNAPSHOT_BEFORE}"
for _ in 1 2 3; do
  run_script "$(dirname -- "$0")/../packaging/fnos/cmd/status"
  test "${script_status}" -eq 0
done
snapshot_data_tree "${SNAPSHOT_AFTER}"
cmp -s "${SNAPSHOT_BEFORE}" "${SNAPSHOT_AFTER}"
grep -q "运行中且服务就绪" "${OUTPUT_FILE}"

printf '%s\n' "503" > "${READY_STATUS_FILE}"
assert_status_keeps_data_tree_unchanged run_script "$(dirname -- "$0")/../packaging/fnos/cmd/status"
test "${script_status}" -eq 1
grep -q "进程运行但服务未就绪" "${OUTPUT_FILE}"
test ! -e "${ARIA2_CALLS}"

rm "${PROC_FIXTURE}/$$/exe"
ln -s "${OTHER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
assert_status_keeps_data_tree_unchanged run_script "$(dirname -- "$0")/../packaging/fnos/cmd/status"
test "${script_status}" -eq 3
grep -q "未运行" "${OUTPUT_FILE}"
test -e "${PID_FILE}"
test -e "${PID_START_FILE}"

rm "${PROC_FIXTURE}/$$/exe"
ln -s "${SERVER_FIXTURE}" "${PROC_FIXTURE}/$$/exe"
write_pid_record "$$"
run_script "$(dirname -- "$0")/../packaging/fnos/cmd/start"
test "${script_status}" -eq 1
grep -q "进程存在但服务未就绪" "${OUTPUT_FILE}"

echo "FPK 服务就绪脚本测试通过。"
