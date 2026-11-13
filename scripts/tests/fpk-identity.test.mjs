import assert from 'node:assert/strict';
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { execFileSync, spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { validateFpkAppIdentity } from '../lib/script-utils.mjs';

test('仓库 FPK 身份与 Release 产物名保持一致', () => {
  const manifestContent = readFileSync('packaging/fnos/manifest.template', 'utf8');
  const uiConfig = JSON.parse(readFileSync('packaging/fnos/app/ui/config', 'utf8'));
  const resource = JSON.parse(readFileSync('packaging/fnos/config/resource', 'utf8'));
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.doesNotThrow(() =>
    validateFpkAppIdentity({
      manifestContent,
      uiConfig,
      expectedAppName: 'motrix',
      expectedEntryId: 'motrix.Application',
    }),
  );
  assert.match(releaseWorkflow, /motrix_\$\{VERSION\}_x86\.fpk/);
  assert.match(releaseWorkflow, /motrix_\$\{VERSION\}_arm\.fpk/);
  assert.match(releaseWorkflow, /generate-fpk-sbom\.mjs/);
  assert.match(releaseWorkflow, /attest-build-provenance@[0-9a-f]{40}/);
  assert.doesNotMatch(releaseWorkflow, /motrix\.fnos_\$\{VERSION\}/);
  assert.match(manifestContent, /^micro_app\s*=\s*true$/m);
  assert.match(manifestContent, /^os_min_version\s*=\s*1\.2\.0401$/m);
  assert.deepEqual(resource['api-scope'], ['trim.file.sharedAccess', 'trim.file.path']);
  assert.equal(uiConfig['.url']?.['motrix.Application']?.allUsers, false);
  assert.equal(uiConfig['.url']?.['motrix.Application']?.control?.accessPerm, 'editable');
});

test('双架构 FPK 预组装脚本保留生命周期和静态产物契约', () => {
  const build = readFileSync('scripts/build/build-fpk.mjs', 'utf8');
  const start = readFileSync('packaging/fnos/cmd/start', 'utf8');
  const status = readFileSync('packaging/fnos/cmd/status', 'utf8');

  assert.match(build, /MotrixFNOS\.sc/);
  assert.match(build, /['"]app['"], ['"]bin['"]|['"]app\/bin['"]/);
  assert.match(build, /['"]app['"], ['"]ui['"]|['"]app\/ui['"]/);
  assert.match(build, /['"]cmd['"]|['"]cmd\//);
  assert.match(build, /minimumFnosVersion\s*=\s*['"]1\.2\.0401['"]/);
  assert.doesNotMatch(build, /0\.9\.0/);
  assert.match(start, /wait_for_server_ready/);
  assert.match(start, /JSONRPC_ADDR/);
  assert.match(start, /LAN_JSONRPC_ADDR/);
  assert.match(status, /readiness_request/);
});

test('安装向导允许保留密码重装时留空，填写时校验长度且不设置明文默认值', () => {
  const steps = JSON.parse(readFileSync('packaging/fnos/wizard/install', 'utf8'));
  const fields = steps.flatMap((step) => step.items).filter((item) => item.field);
  assert.deepEqual(fields.map((item) => item.field), [
    'wizard_management_password',
    'wizard_management_password_confirm',
  ]);
  for (const item of fields) {
    assert.equal(item.type, 'password');
    assert.equal(Object.hasOwn(item, 'initValue'), false);
    assert.ok(item.rules.every((rule) => !rule.required));
    assert.ok(item.rules.some((rule) => rule.min === 8 && rule.max === 128));
  }
  const start = readFileSync('packaging/fnos/cmd/start', 'utf8');
  assert.ok(start.indexOf('unset wizard_management_password wizard_management_password_confirm') < start.indexOf('nohup'));
});

test('应用元数据及安装、升级和卸载向导均有完整中英字典', () => {
  const dictionaries = ['en', 'en-US', 'zh-CN'].map((locale) => {
    const content = readFileSync(`packaging/fnos/i18n/${locale}`, 'utf8');
    assert.match(content, /^\[wizard\]/);
    if (locale !== 'zh-CN') assert.doesNotMatch(content, /\p{Script=Han}/u);
    let section;
    const dictionary = {};
    for (const line of content.split(/\r?\n/)) {
      const heading = /^\[(\w+)\]$/.exec(line);
      if (heading) section = heading[1];
      const entry = /^(\w+)=(".*")$/.exec(line);
      if (entry) dictionary[`${section}.${entry[1]}`] = JSON.parse(entry[2]);
    }
    return dictionary;
  });
  assert.deepEqual(dictionaries[0], dictionaries[1]);
  assert.deepEqual(Object.keys(dictionaries[0]).sort(), Object.keys(dictionaries[2]).sort());
  const used = new Set();
  function check(value) {
    if (Array.isArray(value)) return value.forEach(check);
    if (!value || typeof value !== 'object') return;
    for (const [key, item] of Object.entries(value)) {
      if (['stepTitle', 'label', 'helpText', 'message'].includes(key)) {
        const match = /^\$\{(\w+\.\w+)\}$/.exec(item);
        assert.ok(match, `向导文案未引用字典：${item}`);
        used.add(match[1]);
        for (const dictionary of dictionaries) assert.ok(dictionary[match[1]], `向导翻译缺失：${match[1]}`);
      } else {
        check(item);
      }
    }
  }
  for (const name of ['install', 'upgrade', 'uninstall']) {
    check(JSON.parse(readFileSync(`packaging/fnos/wizard/${name}`, 'utf8')));
  }
  const manifest = readFileSync('packaging/fnos/manifest.template', 'utf8');
  for (const field of ['desc', 'changelog']) {
    const line = manifest.split('\n').find((item) => item.startsWith(`${field} `));
    const reference = /"\$\{(common\.\w+)\}"/.exec(line);
    assert.ok(reference, `manifest.${field} 必须引用包内字典`);
    used.add(reference[1]);
    for (const dictionary of dictionaries) assert.ok(dictionary[reference[1]]);
  }
  assert.deepEqual([...used].sort(), Object.keys(dictionaries[0]).sort());
});

test('生命周期提示按系统语言选择，未知语言回退英语，缺少变量时保留旧版中文行为', () => {
  const common = path.resolve('packaging/fnos/cmd/common.sh');
  for (const [locale, expected] of [['zh-CN', '中文'], ['ZH_cn', '中文'], ['en-US', 'English'], ['ja-JP', 'English'], ['', '中文']]) {
    const output = execFileSync('sh', ['-c', '. "$1"; ui_message "中文" "English"', 'sh', common], {
      env: { ...process.env, TRIM_SYS_LANGUAGE: locale }, encoding: 'utf8',
    });
    assert.equal(output.trim(), expected);
  }
});

test('英语安装和升级失败提示保留数据，且写入应用中心错误文件', () => {
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'motrix-fnos-i18n-'));
  try {
    const binary = path.join(temporary, 'server');
    const errorFile = path.join(temporary, 'error.log');
    const retainedFile = path.join(temporary, 'retained.txt');
    writeFileSync(binary, '#!/bin/sh\nexit 1\n');
    chmodSync(binary, 0o755);
    writeFileSync(retainedFile, 'keep');
    const env = { ...process.env, TRIM_SYS_LANGUAGE: 'en-US', TRIM_PKGVAR: temporary, TRIM_TEMP_LOGFILE: errorFile, MOTRIX_FNOS_SERVER_BIN: binary, wizard_upgrade_acknowledged: '' };
    for (const [script, expected] of [['install_callback', /Management password setup failed/], ['upgrade_init', /Upgrade cancelled/]]) {
      const result = spawnSync('sh', [path.resolve('packaging/fnos/cmd', script)], { env, encoding: 'utf8' });
      assert.equal(result.status, 1);
      assert.match(result.stderr, expected);
      const message = readFileSync(errorFile, 'utf8');
      assert.match(message, expected);
      assert.doesNotMatch(message, /\p{Script=Han}/u);
      assert.equal(readFileSync(retainedFile, 'utf8'), 'keep');
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
