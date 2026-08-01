import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { validateFpkAppIdentity } from '../lib/script-utils.mjs';

test('仓库 FPK 身份与 Release 产物名保持一致', () => {
  const manifestContent = readFileSync('packaging/fnos/manifest.template', 'utf8');
  const uiConfig = JSON.parse(readFileSync('packaging/fnos/app/ui/config', 'utf8'));
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
});

test('双架构 FPK 预组装脚本保留生命周期和静态产物契约', () => {
  const buildAll = readFileSync('scripts/build/build-fpk-all.mjs', 'utf8');
  const build = readFileSync('scripts/build/build-fpk.mjs', 'utf8');
  const start = readFileSync('packaging/fnos/cmd/start', 'utf8');
  const status = readFileSync('packaging/fnos/cmd/status', 'utf8');

  assert.match(buildAll, /x86_64-unknown-linux-gnu/);
  assert.match(buildAll, /aarch64-unknown-linux-gnu/);
  assert.match(buildAll, /--prepare-only/);
  assert.match(buildAll, /已跳过 fnpack build/);
  assert.match(build, /MotrixFNOS\.sc/);
  assert.match(build, /['"]app['"], ['"]bin['"]|['"]app\/bin['"]/);
  assert.match(build, /['"]app['"], ['"]ui['"]|['"]app\/ui['"]/);
  assert.match(build, /['"]cmd['"]|['"]cmd\//);
  assert.match(start, /wait_for_server_ready/);
  assert.match(start, /JSONRPC_ADDR/);
  assert.match(start, /LAN_JSONRPC_ADDR/);
  assert.match(status, /readiness_request/);
});
