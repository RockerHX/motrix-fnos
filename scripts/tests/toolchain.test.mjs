import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
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

test('自动发版使用确定性 CHANGELOG，不依赖已退役的 GitHub Models', () => {
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(releaseWorkflow, /node scripts\/release-prepare\.mjs "\$\{VERSION\}" --no-verify --no-commit --no-tag/);
  assert.doesNotMatch(releaseWorkflow, /github-models|MOTRIX_RELEASE_(?:CHANGELOG_PROVIDER|ANALYSIS_MODEL|EDITOR_MODEL|MODEL_MIN_INTERVAL_MS)/);
  assert.doesNotMatch(releaseWorkflow, /^\s*models:\s*read\s*$/m);
});

test('提交、推送、远端验证和发版使用独立验证层级', () => {
  const prePushHook = readFileSync('.githooks/pre-push', 'utf8');
  const verifyScript = readFileSync('scripts/verify.mjs', 'utf8');
  const verifyWorkflow = readFileSync('.github/workflows/verify.yml', 'utf8');
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const quickSteps = verifyScript.match(/const steps = quick\s*\?\s*\[([\s\S]*?)\]\s*:\s*\[/)?.[1] ?? '';

  assert.equal(existsSync('.githooks/pre-push'), true);
  assert.match(prePushHook, /pnpm run verify/);

  assert.match(quickSteps, /version-check\.mjs/);
  assert.match(quickSteps, /"fmt"/);
  assert.match(quickSteps, /"typecheck"/);
  assert.doesNotMatch(quickSteps, /test:scripts|test-fnos|"test"|test:unit|"build"/);

  assert.match(verifyWorkflow, /workflow_dispatch:/);
  assert.doesNotMatch(verifyWorkflow, /^\s+push:\s*$/m);
  assert.doesNotMatch(verifyWorkflow, /^\s+pull_request:\s*$/m);
  assert.match(verifyWorkflow, /run:\s+pnpm run verify/);

  assert.match(releaseWorkflow, /git commit --no-verify/);
  assert.doesNotMatch(releaseWorkflow, /^\s*actions:\s*read\s*$/m);
  assert.doesNotMatch(releaseWorkflow, /Require successful main verification|gh run list|source_sha/);
  assert.doesNotMatch(releaseWorkflow, /cargo install cargo-audit|pnpm run audit:deps|pnpm run verify(?::pre-commit)?/);
});
