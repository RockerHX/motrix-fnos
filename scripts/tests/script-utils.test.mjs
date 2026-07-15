import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  classifyCommit,
  cleanupCommitSubject,
  compareReleaseVersions,
  normalizeGeneratedChangelog,
  parseChecksums,
  parseManifest,
  platformForTarget,
  removeManifestField,
  sha256,
  upsertManifestField,
  validateFpkPortIsolation,
  validateFpkRuntimeEnvScript,
  validateChangelogBody,
  validatePortEntry,
} from '../script-utils.mjs';
import {
  assertProjectVersion,
  assertReleaseVersion,
  findVersionMismatches,
  nextTestVersion,
  readProjectVersions,
  setProjectVersion,
} from '../version-utils.mjs';

test('版本号校验与比较使用语义化数字段', () => {
  assert.doesNotThrow(() => assertReleaseVersion('1.10.0'));
  assert.throws(() => assertReleaseVersion('v1.10.0'), /x\.y\.z/);
  assert.throws(() => assertReleaseVersion('1.10.1-test.1'), /x\.y\.z/);
  assert.doesNotThrow(() => assertProjectVersion('1.10.1-test.1'));
  assert.throws(() => assertProjectVersion('1.10.1-test.01'), /x\.y\.z-test\.N/);
  assert.throws(() => assertProjectVersion('1.10.1-beta.1'), /x\.y\.z-test\.N/);
  assert.ok(compareReleaseVersions('1.10.0', '1.9.9') > 0);
  assert.ok(compareReleaseVersions('2.0.0', '2.0.1') < 0);
  assert.equal(compareReleaseVersions('2.0.0', '2.0.0'), 0);
  assert.ok(compareReleaseVersions('1.7.4-test.1', '1.7.3') > 0);
  assert.ok(compareReleaseVersions('1.7.4-test.2', '1.7.4-test.1') > 0);
  assert.ok(compareReleaseVersions('1.7.4', '1.7.4-test.2') > 0);
  assert.throws(() => compareReleaseVersions('1.7.4-beta.1', '1.7.3'), /无法比较版本号/);
});

test('测试版本从下一补丁开始并依次递增序号', () => {
  assert.equal(nextTestVersion('1.7.3'), '1.7.4-test.1');
  assert.equal(nextTestVersion('1.7.4-test.1'), '1.7.4-test.2');
  assert.equal(nextTestVersion('1.7.4-test.9'), '1.7.4-test.10');
  assert.throws(() => nextTestVersion('1.7.4-rc.1'), /x\.y\.z-test\.N/);
});

test('版本一致性检查列出所有偏离 package.json 的来源', () => {
  assert.deepEqual(
    findVersionMismatches({
      packageJson: '1.7.0',
      cargoToml: '1.7.0',
      manifestTemplate: '1.6.0',
      uiConfig: '1.6.1',
    }),
    [
      { source: 'manifestTemplate', version: '1.6.0', expected: '1.7.0' },
      { source: 'uiConfig', version: '1.6.1', expected: '1.7.0' },
    ],
  );
});

test('版本同步同时更新 package、Cargo、manifest 与 UI cache', () => {
  const fixtureRoot = mkdtempSync(path.join(tmpdir(), 'motrix-version-'));
  const files = {
    packageJson: path.join(fixtureRoot, 'package.json'),
    cargoToml: path.join(fixtureRoot, 'server', 'Cargo.toml'),
    manifestTemplate: path.join(fixtureRoot, 'packaging', 'fnos', 'manifest.template'),
    uiConfig: path.join(fixtureRoot, 'packaging', 'fnos', 'app', 'ui', 'config'),
  };

  try {
    mkdirSync(path.dirname(files.cargoToml), { recursive: true });
    mkdirSync(path.dirname(files.uiConfig), { recursive: true });
    writeFileSync(files.packageJson, '{\n  "version": "1.7.1"\n}\n');
    writeFileSync(files.cargoToml, '[package]\nversion = "1.7.1"\n');
    writeFileSync(files.manifestTemplate, 'appname               = motrix.fnos\nversion               = 1.7.1\n');
    writeFileSync(
      files.uiConfig,
      `${JSON.stringify({ '.url': { 'motrix.fnos.main': { url: '/?v=1.7.1' } } }, null, 2)}\n`,
    );

    setProjectVersion('1.7.2', files);

    assert.deepEqual(readProjectVersions(files), {
      packageJson: '1.7.2',
      cargoToml: '1.7.2',
      manifestTemplate: '1.7.2',
      uiConfig: '1.7.2',
    });
    assert.equal(JSON.parse(readFileSync(files.uiConfig, 'utf8'))['.url']['motrix.fnos.main'].url, '/?v=1.7.2');
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('版本同步支持测试版本并能从 UI cache 读回', () => {
  const fixtureRoot = mkdtempSync(path.join(tmpdir(), 'motrix-test-version-'));
  const files = {
    packageJson: path.join(fixtureRoot, 'package.json'),
    cargoToml: path.join(fixtureRoot, 'server', 'Cargo.toml'),
    manifestTemplate: path.join(fixtureRoot, 'packaging', 'fnos', 'manifest.template'),
    uiConfig: path.join(fixtureRoot, 'packaging', 'fnos', 'app', 'ui', 'config'),
  };

  try {
    mkdirSync(path.dirname(files.cargoToml), { recursive: true });
    mkdirSync(path.dirname(files.uiConfig), { recursive: true });
    writeFileSync(files.packageJson, '{\n  "version": "1.7.3"\n}\n');
    writeFileSync(files.cargoToml, '[package]\nversion = "1.7.3"\n');
    writeFileSync(files.manifestTemplate, 'version               = 1.7.3\n');
    writeFileSync(
      files.uiConfig,
      `${JSON.stringify({ '.url': { 'motrix.fnos.main': { url: '/?v=1.7.3' } } }, null, 2)}\n`,
    );

    setProjectVersion('1.7.4-test.1', files);

    assert.deepEqual(readProjectVersions(files), {
      packageJson: '1.7.4-test.1',
      cargoToml: '1.7.4-test.1',
      manifestTemplate: '1.7.4-test.1',
      uiConfig: '1.7.4-test.1',
    });
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test('端口入口与 server listener 保持一致并拒绝混入网关字段', () => {
  const expected = {
    entryId: 'motrix.fnos.main',
    port: '17080',
    url: '/?v=1.7.1',
    accessPerm: 'editable',
  };
  const config = {
    '.url': {
      'motrix.fnos.main': {
        type: 'iframe',
        protocol: 'http',
        port: '17080',
        url: '/?v=1.7.1',
        control: { accessPerm: 'editable', portPerm: 'readonly' },
      },
    },
  };

  assert.doesNotThrow(() => validatePortEntry(config, expected));
  assert.throws(
    () =>
      validatePortEntry(
        { '.url': { 'motrix.fnos.main': { ...config['.url']['motrix.fnos.main'], port: '18080' } } },
        expected,
      ),
    /port 必须为 17080/,
  );
  assert.throws(
    () =>
      validatePortEntry(
        {
          '.url': {
            'motrix.fnos.main': {
              ...config['.url']['motrix.fnos.main'],
              gatewayPrefix: '/app/motrix',
              gatewaySocket: 'motrix-fnos.sock',
            },
          },
        },
        expected,
      ),
    /不得声明 gatewayPrefix/,
  );
  assert.throws(
    () =>
      validatePortEntry(
        {
          '.url': {
            'motrix.fnos.main': {
              ...config['.url']['motrix.fnos.main'],
              url: '/',
            },
          },
        },
        expected,
      ),
    /url 必须为/,
  );
  assert.throws(
    () =>
      validatePortEntry(
        {
          '.url': {
            'motrix.fnos.main': {
              ...config['.url']['motrix.fnos.main'],
              control: { accessPerm: 'readonly' },
            },
          },
        },
        expected,
      ),
    /accessPerm 必须为 editable/,
  );
});

test('FPK 端口隔离只允许公开管理端口', () => {
  const fixture = {
    manifestContent: 'appname = motrix.fnos\nversion = 1.7.2\nservice_port = 17080\n',
    uiConfig: {
      '.url': {
        'motrix.fnos.main': {
          type: 'iframe',
          protocol: 'http',
          port: '17080',
          url: '/?v=1.7.2',
          control: { accessPerm: 'editable', portPerm: 'readonly' },
        },
      },
    },
    portConfigContent: '[MotrixFNOS]\nsrc.ports="17080/tcp"\ndst.ports="17080/tcp"\n',
    resourceContent: '{"port-config":{"protocol-file":"MotrixFNOS.sc"}}',
    managementPort: '17080',
    jsonRpcPort: '17081',
  };

  assert.doesNotThrow(() => validateFpkPortIsolation(fixture));
  assert.throws(
    () => validateFpkPortIsolation({ ...fixture, manifestContent: fixture.manifestContent.replace('17080', '17081') }),
    /manifest\.service_port 必须为管理端口 17080/,
  );
  assert.throws(
    () => validateFpkPortIsolation({ ...fixture, uiConfig: JSON.parse(JSON.stringify(fixture.uiConfig).replace('17080', '17081')) }),
    /port 必须为 17080/,
  );
  assert.throws(
    () => validateFpkPortIsolation({ ...fixture, portConfigContent: fixture.portConfigContent.replace('src.ports="17080', 'src.ports="17081') }),
    /MotrixFNOS\.sc 不得声明 JSON-RPC 专用端口 17081/,
  );
  assert.throws(
    () => validateFpkPortIsolation({ ...fixture, resourceContent: '{"ports":[17081]}' }),
    /config\/resource 不得声明 JSON-RPC 专用端口 17081/,
  );
});

test('FPK 运行脚本必须导出 JSON-RPC 回环地址', () => {
  const script = 'JSONRPC_ADDR=${MOTRIX_FNOS_JSONRPC_ADDR:-"127.0.0.1:17081"}\nexport MOTRIX_FNOS_JSONRPC_ADDR="${JSONRPC_ADDR}"\n';

  assert.doesNotThrow(() => validateFpkRuntimeEnvScript(script, '127.0.0.1:17081'));
  assert.throws(() => validateFpkRuntimeEnvScript(script.replace('127.0.0.1', '0.0.0.0'), '127.0.0.1:17081'), /缺少 JSON-RPC 回环默认值/);
  assert.throws(() => validateFpkRuntimeEnvScript(script.replace('export MOTRIX_FNOS_JSONRPC_ADDR', 'export OTHER_ADDR'), '127.0.0.1:17081'), /未导出/);
});

test('CHANGELOG 生成逻辑清理提交前缀并保持中文分组', () => {
  assert.equal(classifyCommit('feat(tasks): 增加批量任务'), '新增');
  assert.equal(classifyCommit('fix: 修复状态'), '修复');
  assert.equal(classifyCommit('docs(api): 更新契约'), '文档');
  assert.equal(classifyCommit('refactor: 拆分模块'), '改进');
  assert.equal(cleanupCommitSubject('test(server): 补充测试'), '补充测试');
  assert.equal(
    normalizeGeneratedChangelog('```md\n## 1.8.0\n\n### 修复\n\n- 修复状态\n```'),
    '### 修复\n\n- 修复状态',
  );
  assert.doesNotThrow(() => validateChangelogBody('### 新增\n\n- 增加任务\n\n### 文档\n\n- 更新说明'));
  assert.throws(() => normalizeGeneratedChangelog(''), /模型返回了空 CHANGELOG/);
  assert.throws(() => normalizeGeneratedChangelog('- 内部整理'), /未归入分类/);
  assert.throws(() => normalizeGeneratedChangelog('### 其他\n\n- 内部整理'), /不允许的分类“其他”/);
  assert.throws(() => normalizeGeneratedChangelog('### 修复\n\n### 文档\n\n- 更新说明'), /“修复”分类没有日志条目/);
  assert.throws(() => normalizeGeneratedChangelog('### 修复'), /“修复”分类没有日志条目/);
  assert.throws(() => normalizeGeneratedChangelog('### 修复\n\n修复状态'), /不支持的内容/);
});

test('FPK manifest 字段转换保持对齐并支持删除', () => {
  const source = 'source                = fnos\nplatform              = x86\narch                  = x86_64\n';
  const updated = upsertManifestField(source, 'platform', 'arm');
  const removed = removeManifestField(updated, 'arch');
  const inserted = upsertManifestField(removed, 'service_port', '17080');

  assert.match(updated, /^platform\s+= arm$/m);
  assert.doesNotMatch(removed, /^arch\s*=/m);
  assert.equal(parseManifest(inserted).service_port, '17080');
  assert.equal(parseManifest(inserted).platform, 'arm');
});

test('构建目标映射到对应 fnOS 平台', () => {
  assert.equal(platformForTarget('x86_64-unknown-linux-gnu'), 'x86');
  assert.equal(platformForTarget('aarch64-unknown-linux-gnu'), 'arm');
});

test('Aria2 checksum 解析忽略无效行并统一哈希大小写', () => {
  const hash = 'A'.repeat(64);
  const checksums = parseChecksums(`${hash}  aria2-next-linux-x86_64\ninvalid\n`);

  assert.equal(checksums.get('aria2-next-linux-x86_64'), hash.toLowerCase());
  assert.equal(checksums.size, 1);
  assert.equal(sha256(Buffer.from('motrix')), '21d18eea6592c920bb403ba3f94c86811ea6fefba161950a2e55a75e888759c5');
});
