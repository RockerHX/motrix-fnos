import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('验证工具链版本和 CI 安装方式固定', () => {
  const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
  const nodeVersion = readFileSync('.node-version', 'utf8').trim();
  const rustToolchain = readFileSync('rust-toolchain.toml', 'utf8');
  const verifyWorkflow = readFileSync('.github/workflows/verify.yml', 'utf8');
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const buildScript = readFileSync('scripts/build-fpk-all.mjs', 'utf8');

  assert.equal(packageJson.packageManager, 'pnpm@11.17.0');
  assert.match(nodeVersion, /^22\.\d+\.\d+$/);
  assert.match(rustToolchain, /channel\s*=\s*"1\.97\.1"/);
  for (const workflow of [verifyWorkflow, releaseWorkflow]) {
    assert.match(workflow, /version:\s+11\.17\.0/);
    assert.match(workflow, /toolchain:\s+1\.97\.1/);
    assert.match(workflow, /node-version-file:\s+\.node-version/);
    assert.match(workflow, /pnpm install --frozen-lockfile/);
  }
  assert.match(buildScript, /x86_64-unknown-linux-gnu/);
  assert.match(buildScript, /aarch64-unknown-linux-gnu/);
});
