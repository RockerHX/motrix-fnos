import assert from 'node:assert/strict';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repoRoot = process.cwd();

test('FPK 升级脚本执行检查、迁移并保留失败数据', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-'));
  try {
    const layout = createLayout(root, 'native-v2');
    writeFileSync(path.join(layout.data, 'keep.db'), 'user-state');

    const check = runScript(layout, 'upgrade_init');
    assert.equal(check.status, 0, check.stderr);
    assert.equal(readFileSync(layout.calls, 'utf8'), 'engine-upgrade-check\n');
    assert.equal(readFileSync(path.join(layout.data, 'keep.db'), 'utf8'), 'user-state');

    const migrate = runScript(layout, 'upgrade_callback');
    assert.equal(migrate.status, 0, migrate.stderr);
    assert.equal(readFileSync(layout.calls, 'utf8'), 'engine-upgrade-check\nengine-upgrade-migrate\n');

    writeFileSync(layout.checker, '#!/bin/sh\nexit 17\n');
    chmodSync(layout.checker, 0o755);
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      const failed = runScript(layout, script);
      assert.notEqual(failed.status, 0);
      assert.equal(readFileSync(path.join(layout.data, 'keep.db'), 'utf8'), 'user-state');
      assert.match(readFileSync(layout.errorFile, 'utf8'), /升级/);
    }
    rmSync(layout.checker);
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      assert.notEqual(runScript(layout, script).status, 0);
      assert.match(readFileSync(layout.errorFile, 'utf8'), /工具不可用/);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('FPK 旧代际跳过迁移，缺失或未知代际必须拒绝', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-reject-'));
  try {
    const legacy = createLayout(root, 'legacy-v1');
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      assert.equal(runScript(legacy, script).status, 0);
    }
    assert.equal(existsSync(legacy.calls), false);

    for (const generation of ['future-v3', '']) {
      const unknown = createLayout(path.join(root, generation || 'missing'), generation);
      if (!generation) rmSync(path.join(unknown.app, 'bin/engine-generation'));
      for (const script of ['upgrade_init', 'upgrade_callback']) {
        assert.notEqual(runScript(unknown, script).status, 0);
        assert.match(readFileSync(unknown.errorFile, 'utf8'), /引擎代际/);
      }
      assert.equal(existsSync(unknown.calls), false);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('FPK 升级脚本在服务或 sidecar 存活时拒绝且不发送信号', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-running-'));
  try {
    const active = createLayout(root, 'native-v2');
    // 只读取当前测试进程的存活状态，伪 /proc 提供应用归属，避免启动后台进程。
    const pid = process.pid;
    writeFileSync(active.pid, `${pid}\n`);
    const procDir = path.join(active.procRoot, String(pid));
    mkdirSync(procDir, { recursive: true });
    writeFileSync(path.join(procDir, 'stat'), `${Array(21).fill('0').join(' ')} 12345 0\n`);
    writeFileSync(path.join(procDir, 'status'), `Name: fake-server\nUid:\t${process.getuid?.() ?? 0}\t${process.getuid?.() ?? 0}\t${process.getuid?.() ?? 0}\t${process.getuid?.() ?? 0}\n`);
    symlinkSync(active.server, path.join(procDir, 'exe'));
    writeFileSync(path.join(procDir, 'cmdline'), 'fake-server\0');
    writeFileSync(active.start, '12345\n');
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      assert.notEqual(runScript(active, script).status, 0);
      assert.equal(existsSync(active.calls), false);
    }
    rmSync(active.pid);
    writeFileSync(path.join(active.data, 'aria2-runtime.json'), JSON.stringify({ pid }, null, 2));
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      assert.notEqual(runScript(active, script).status, 0);
      assert.equal(existsSync(active.calls), false);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function createLayout(root, generation) {
  const cmd = path.join(root, 'cmd');
  const app = path.join(root, 'app');
  const bin = path.join(app, 'bin');
  const data = path.join(root, 'var');
  const varDir = path.join(root, 'var');
  mkdirSync(cmd, { recursive: true });
  mkdirSync(bin, { recursive: true });
  mkdirSync(data, { recursive: true });
  mkdirSync(path.join(varDir, 'run'), { recursive: true });
  copyFileSync(path.join(repoRoot, 'packaging/fnos/cmd/common.sh'), path.join(cmd, 'common.sh'));
  for (const script of ['upgrade_init', 'upgrade_callback']) {
    copyFileSync(path.join(repoRoot, `packaging/fnos/cmd/${script}`), path.join(cmd, script));
    chmodSync(path.join(cmd, script), 0o755);
  }
  const checker = path.join(cmd, 'upgrade-checker');
  writeFileSync(checker, '#!/bin/sh\n[ "$MOTRIX_FNOS_APP_DATA_DIR" = "$TRIM_PKGVAR" ] || exit 9\n[ "$MOTRIX_FNOS_ARIA2_PATH" = "$TRIM_APPDEST/bin/aria2-next" ] || exit 10\nprintf "%s\\n" "$1" >> "$MOTRIX_FNOS_UPGRADE_CALLS"\n');
  chmodSync(checker, 0o755);
  writeFileSync(path.join(bin, 'engine-generation'), `${generation}\n`);
  const server = path.join(bin, 'motrix-fnos-server');
  writeFileSync(server, 'placeholder');
  chmodSync(server, 0o755);
  writeFileSync(path.join(bin, 'aria2-next'), 'placeholder');
  return {
    cmd,
    app,
    data,
    checker,
    calls: path.join(root, 'calls.log'),
    pid: path.join(varDir, 'run/motrix-fnos-server.pid'),
    start: path.join(varDir, 'run/motrix-fnos-server.starttime'),
    server,
    procRoot: path.join(root, 'proc'),
    errorFile: path.join(root, 'upgrade-error'),
  };
}

function runScript(layout, script) {
  const env = {
    ...process.env,
    TRIM_APPDEST: layout.app,
    TRIM_PKGVAR: path.dirname(path.dirname(layout.pid)),
    TRIM_TEMP_LOGFILE: layout.errorFile,
    MOTRIX_FNOS_APP_DATA_DIR: layout.data,
    MOTRIX_FNOS_UPGRADE_CALLS: layout.calls,
    MOTRIX_FNOS_ARIA2_PATH: path.join(layout.app, 'bin/aria2-next'),
    MOTRIX_FNOS_SERVER_BIN: layout.server,
    MOTRIX_FNOS_PROC_ROOT: layout.procRoot,
    MOTRIX_FNOS_STOP_RETRY_SECONDS: '0',
  };
  const result = spawnSync('/bin/sh', [path.join(layout.cmd, script)], { env, encoding: 'utf8', timeout: 5000 });
  assert.ifError(result.error);
  return result;
}
