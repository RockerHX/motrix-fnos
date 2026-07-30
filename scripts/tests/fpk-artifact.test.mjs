import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parseManifest } from '../script-utils.mjs';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const packageVersion = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')).version;
const packageTargets = [
  { platform: 'x86', machine: /x86-64/ },
  { platform: 'arm', machine: /aarch64/ },
];

test('双架构 FPK 解包后保留完整运行内容且不携带运行态残留', (t) => {
  const artifacts = packageTargets.map(({ platform, machine }) => ({
    platform,
    machine,
    file: path.join(repoRoot, 'packaging', 'fnos', 'dist', `motrix_${packageVersion}_${platform}.fpk`),
  }));
  const existingArtifacts = artifacts.filter(({ file }) => existsSync(file));

  if (existingArtifacts.length === 0) {
    t.skip('未发现已构建的双架构 FPK，跳过解包验收');
    return;
  }

  assert.equal(existingArtifacts.length, artifacts.length, 'x86 与 ARM FPK 必须成对生成');
  const extractionRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-fpk-artifact-'));

  try {
    for (const artifact of artifacts) {
      validateArtifact(artifact, extractionRoot);
    }
  } finally {
    rmSync(extractionRoot, { recursive: true, force: true });
  }
});

function validateArtifact({ file, platform, machine }, extractionRoot) {
  const packageRoot = path.join(extractionRoot, platform);
  mkdirSync(packageRoot, { recursive: true });
  execFileSync('tar', ['-xzf', file, '-C', packageRoot]);

  for (const relativePath of [
    'manifest',
    'MotrixFNOS.sc',
    'app.tgz',
    'config/resource',
    'config/privilege',
    'cmd/common.sh',
    'cmd/start',
    'cmd/stop',
    'cmd/status',
  ]) {
    assertRegularFile(packageRoot, relativePath);
  }

  const manifest = parseManifest(readFileSync(path.join(packageRoot, 'manifest'), 'utf8'));
  assert.equal(manifest.appname, 'motrix');
  assert.equal(manifest.version, packageVersion);
  assert.equal(manifest.platform, platform);
  assert.equal(manifest.service_port, '17080');
  assert.match(readFileSync(path.join(packageRoot, 'MotrixFNOS.sc'), 'utf8'), /src\.ports="17080\/tcp"/);
  assert.match(readFileSync(path.join(packageRoot, 'MotrixFNOS.sc'), 'utf8'), /dst\.ports="17080\/tcp"/);

  const appRoot = path.join(packageRoot, 'app');
  mkdirSync(appRoot, { recursive: true });
  execFileSync('tar', ['-xzf', path.join(packageRoot, 'app.tgz'), '-C', appRoot]);

  for (const relativePath of [
    'bin/motrix-fnos-server',
    'bin/aria2-next',
    'config/resource',
    'config/privilege',
    'ui/config',
    'ui/dist/index.html',
  ]) {
    assertRegularFile(appRoot, relativePath);
  }

  const serverBinary = path.join(appRoot, 'bin/motrix-fnos-server');
  const aria2Binary = path.join(appRoot, 'bin/aria2-next');
  assertExecutable(serverBinary);
  assertExecutable(aria2Binary);
  assert.match(execFileSync('file', [serverBinary], { encoding: 'utf8' }), machine);
  assert.match(execFileSync('file', [aria2Binary], { encoding: 'utf8' }), machine);

  const uiConfig = JSON.parse(readFileSync(path.join(appRoot, 'ui/config'), 'utf8'));
  assert.equal(uiConfig['.url']?.['motrix.Application']?.port, '17080');
  assert.deepEqual(readdirSync(path.join(appRoot, 'data')), [], 'app/data 不得携带运行时残留');
}

function assertRegularFile(root, relativePath) {
  const target = path.join(root, relativePath);
  assert.equal(existsSync(target), true, `FPK 缺少 ${relativePath}`);
  assert.equal(statSync(target).isFile(), true, `FPK 路径不是文件：${relativePath}`);
}

function assertExecutable(file) {
  assert.notEqual(statSync(file).mode & 0o111, 0, `文件不可执行：${file}`);
}
