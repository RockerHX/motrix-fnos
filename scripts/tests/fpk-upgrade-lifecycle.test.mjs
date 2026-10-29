import assert from 'node:assert/strict';
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const repoRoot = process.cwd();
const requireFromNaiveUi = createRequire(import.meta.resolve('naive-ui'));
const { default: Schema } = requireFromNaiveUi('async-validator');

test('FPK 升级确认按 checkbox 数组校验，勾选通过且取消后再次拒绝', async () => {
  const steps = JSON.parse(readFileSync('packaging/fnos/wizard/upgrade', 'utf8'));
  const confirmation = steps.flatMap((step) => step.items)
    .find((item) => item.field === 'wizard_upgrade_acknowledged');
  const schema = new Schema({ [confirmation.field]: confirmation.rules });
  await assert.rejects(schema.validate({ [confirmation.field]: [] }));
  await schema.validate({ [confirmation.field]: ['true'] });
  await assert.rejects(schema.validate({ [confirmation.field]: [] }));
  for (const value of [undefined, false, true, 'true']) {
    await assert.rejects(schema.validate({ [confirmation.field]: value }));
  }
  assert.equal(confirmation.type, 'checkbox');
  assert.equal(Object.hasOwn(confirmation, 'initValue'), false);
  assert.ok(confirmation.rules.some((rule) => rule.required === true));
  assert.deepEqual(confirmation.options.map((option) => option.value), ['true']);
});

test('FPK 升级前拒绝缺失或无效确认，保留原有应用与数据', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-consent-'));
  try {
    const layout = createLayout(root);
    const dataFile = path.join(layout.data, 'keep.db');
    writeFileSync(dataFile, 'original-data');
    for (const value of [undefined, '', 'false', '0', 'yes', 'invalid', '[]', '["false"]', '["true","false"]', '["true","true"]']) {
      const result = runScript(layout, 'upgrade_init', { wizard_upgrade_acknowledged: value });
      assert.notEqual(result.status, 0, `unexpected confirmation: ${value}`);
      assert.match(readFileSync(layout.errorFile, 'utf8'), /请先在升级向导中阅读并同意/);
      assert.equal(readFileSync(dataFile, 'utf8'), 'original-data');
      assert.equal(readFileSync(layout.server, 'utf8'), 'placeholder');
    }
    for (const value of ['true', '1', JSON.stringify(['true'])]) {
      const result = runScript(layout, 'upgrade_init', { wizard_upgrade_acknowledged: value });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(readFileSync(dataFile, 'utf8'), 'original-data');
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('FPK 升级无需检查器或迁移标记，重复执行保留旧任务状态和下载文件', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-'));
  try {
    const layout = createLayout(root);
    for (const file of ['keep.db', 'old.session', 'unfinished.bin']) {
      writeFileSync(path.join(layout.data, file), `preserved-${file}`);
    }
    assert.equal(existsSync(path.join(layout.cmd, 'upgrade-checker')), false);
    for (let attempt = 0; attempt < 2; attempt += 1) {
      for (const script of ['upgrade_init', 'upgrade_callback']) {
        const result = runScript(layout, script);
        assert.equal(result.status, 0, result.stderr);
      }
      for (const file of ['keep.db', 'old.session', 'unfinished.bin']) {
        assert.equal(readFileSync(path.join(layout.data, file), 'utf8'), `preserved-${file}`);
      }
      assert.equal(existsSync(path.join(layout.data, 'engine-upgrade-pending.json')), false);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('FPK 升级脚本在服务或 sidecar 存活时拒绝且不发送信号', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-upgrade-running-'));
  try {
    const active = createLayout(root);
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
    }
    rmSync(active.pid);
    writeFileSync(path.join(active.data, 'aria2-runtime.json'), JSON.stringify({ pid }, null, 2));
    for (const script of ['upgrade_init', 'upgrade_callback']) {
      assert.notEqual(runScript(active, script).status, 0);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function createLayout(root) {
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
  const server = path.join(bin, 'motrix-fnos-server');
  writeFileSync(server, 'placeholder');
  chmodSync(server, 0o755);
  writeFileSync(path.join(bin, 'aria2-next'), 'placeholder');
  return {
    cmd,
    app,
    data,
    pid: path.join(varDir, 'run/motrix-fnos-server.pid'),
    start: path.join(varDir, 'run/motrix-fnos-server.starttime'),
    server,
    procRoot: path.join(root, 'proc'),
    errorFile: path.join(root, 'upgrade-error'),
  };
}

function runScript(layout, script, envOverrides = {}) {
  const env = {
    ...process.env,
    TRIM_APPDEST: layout.app,
    TRIM_PKGVAR: path.dirname(path.dirname(layout.pid)),
    TRIM_TEMP_LOGFILE: layout.errorFile,
    MOTRIX_FNOS_APP_DATA_DIR: layout.data,
    MOTRIX_FNOS_ARIA2_PATH: path.join(layout.app, 'bin/aria2-next'),
    MOTRIX_FNOS_SERVER_BIN: layout.server,
    MOTRIX_FNOS_PROC_ROOT: layout.procRoot,
    MOTRIX_FNOS_STOP_RETRY_SECONDS: '0',
    wizard_upgrade_acknowledged: JSON.stringify(['true']),
    ...envOverrides,
  };
  const result = spawnSync('/bin/sh', [path.join(layout.cmd, script)], { env, encoding: 'utf8', timeout: 5000 });
  assert.ifError(result.error);
  return result;
}
