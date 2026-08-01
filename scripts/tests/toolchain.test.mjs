import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import test from 'node:test';

test('验证工具链版本和 CI 安装方式固定', () => {
  const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
  const nodeVersion = readFileSync('.node-version', 'utf8').trim();
  const rustToolchain = readFileSync('rust-toolchain.toml', 'utf8');
  const verifyWorkflow = readFileSync('.github/workflows/verify.yml', 'utf8');
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const auditWorkflow = readFileSync('.github/workflows/dependency-audit.yml', 'utf8');
  const buildScript = readFileSync('scripts/build/build-fpk-all.mjs', 'utf8');

  assert.equal(packageJson.packageManager, 'pnpm@11.17.0');
  assert.match(nodeVersion, /^22\.\d+\.\d+$/);
  assert.match(rustToolchain, /channel\s*=\s*"1\.97\.1"/);
  for (const workflow of [verifyWorkflow, releaseWorkflow]) {
    assert.match(workflow, /version:\s+11\.17\.0/);
    assert.match(workflow, /toolchain:\s+1\.97\.1/);
    assert.match(workflow, /node-version-file:\s+\.node-version/);
    assert.match(workflow, /pnpm install --frozen-lockfile/);
  }
  assert.match(auditWorkflow, /version:\s+11\.17\.0/);
  assert.match(auditWorkflow, /toolchain:\s+1\.97\.1/);
  assert.match(auditWorkflow, /node-version-file:\s+\.node-version/);
  assert.match(buildScript, /x86_64-unknown-linux-gnu/);
  assert.match(buildScript, /aarch64-unknown-linux-gnu/);
});

test('自动发版使用确定性 CHANGELOG，不依赖已退役的 GitHub Models', () => {
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(releaseWorkflow, /node scripts\/release\/release-prepare\.mjs "\$\{VERSION\}" --no-commit --no-tag/);
  assert.doesNotMatch(releaseWorkflow, /github-models|MOTRIX_RELEASE_(?:CHANGELOG_PROVIDER|ANALYSIS_MODEL|EDITOR_MODEL|MODEL_MIN_INTERVAL_MS)/);
  assert.doesNotMatch(releaseWorkflow, /^\s*models:\s*read\s*$/m);
});

test('提交、推送、远端验证和发版使用独立验证层级', () => {
  const prePushHook = readFileSync('.githooks/pre-push', 'utf8');
  const verifyScript = readFileSync('scripts/verify/verify.mjs', 'utf8');
  const releasePrepareScript = readFileSync('scripts/release/release-prepare.mjs', 'utf8');
  const rustTestScript = readFileSync('scripts/verify/run-rust-tests.mjs', 'utf8');
  const commandProgressScript = readFileSync('scripts/lib/command-progress.mjs', 'utf8');
  const vitestProgressReporter = readFileSync('scripts/verify/vitest-progress-reporter.mjs', 'utf8');
  const verifyWorkflow = readFileSync('.github/workflows/verify.yml', 'utf8');
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const auditWorkflow = readFileSync('.github/workflows/dependency-audit.yml', 'utf8');
  const quickSteps = verifyScript.match(/const steps = quick\s*\?\s*\[([\s\S]*?)\]\s*:\s*\[/)?.[1] ?? '';
  const fullSteps = verifyScript.match(/\]\s*:\s*\[([\s\S]*?)\];/)?.[1] ?? '';

  assert.equal(existsSync('.githooks/pre-push'), true);
  assert.match(prePushHook, /pnpm run verify/);

  assert.match(quickSteps, /version-check\.mjs/);
  assert.match(quickSteps, /"fmt"/);
  assert.doesNotMatch(quickSteps, /typecheck|test:scripts|test-fnos|"test"|test:unit|"build"/);
  assert.doesNotMatch(releasePrepareScript, /run\('pnpm', \['run', 'verify'\]\)/);
  assert.match(fullSteps, /run-rust-tests\.mjs/);
  assert.match(fullSteps, /\["build", "--manifest-path"/);
  assert.match(rustTestScript, /\['test', '--manifest-path', 'server\/Cargo\.toml'\]/);
  assert.match(verifyScript, /runCommandWithProgress/);
  assert.match(commandProgressScript, /HEARTBEAT_INTERVAL_MS = 30_000/);
  assert.match(commandProgressScript, /PROGRESS_MESSAGE_PREFIX/);
  assert.match(vitestProgressReporter, /onTestCaseReady/);

  assert.match(verifyWorkflow, /workflow_dispatch:/);
  assert.doesNotMatch(verifyWorkflow, /^\s+push:\s*$/m);
  assert.doesNotMatch(verifyWorkflow, /^\s+pull_request:\s*$/m);
  assert.match(verifyWorkflow, /run:\s+pnpm run verify/);

  assert.match(releaseWorkflow, /git commit --no-verify/);
  assert.match(releaseWorkflow, /git push --no-verify --atomic/);
  assert.match(releaseWorkflow, /git push --no-verify origin/);
  assert.match(releaseWorkflow, /pnpm run build:fpk:artifacts/);
  assert.match(releaseWorkflow, /pnpm run verify:fpk/);
  assert.doesNotMatch(releaseWorkflow, /^\s*actions:\s*read\s*$/m);
  assert.doesNotMatch(releaseWorkflow, /Require successful main verification|gh run list|source_sha/);
  assert.doesNotMatch(releaseWorkflow, /cargo install cargo-audit|pnpm run audit:deps|pnpm run verify(?!:fpk)/);

  assert.doesNotMatch(verifyWorkflow, /cargo install cargo-audit|pnpm run audit:deps/);
  assert.match(auditWorkflow, /schedule:/);
  assert.match(auditWorkflow, /cron:\s*"23 19 \* \* 0"/);
  assert.match(auditWorkflow, /cargo install cargo-audit --version 0\.22\.2 --locked/);
  assert.match(auditWorkflow, /pnpm run audit:deps/);
});

test('本地完整打包与 Release 产物构建复用明确的验证层级', () => {
  const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
  const packageLocalScript = readFileSync('scripts/build/package-local.mjs', 'utf8');
  const buildAllScript = readFileSync('scripts/build/build-fpk-all.mjs', 'utf8');
  const buildFpkScript = readFileSync('scripts/build/build-fpk.mjs', 'utf8');
  const buildServerScript = readFileSync('scripts/build/build-server-linux.mjs', 'utf8');
  const buildFpkWebScript = readFileSync('scripts/build/build-web-ui-fpk.mjs', 'utf8');
  const buildWebScript = readFileSync('scripts/build/build-web.mjs', 'utf8');

  assert.equal(packageJson.scripts['build:fpk'], 'node scripts/build/package-local.mjs');
  assert.equal(packageJson.scripts['build:fpk:artifacts'], 'node scripts/build/build-fpk-all.mjs');
  assert.equal(packageJson.scripts['verify:fpk'], 'node scripts/verify/verify-fpk-artifacts.mjs');
  assert.match(packageJson.scripts['test:scripts'], /--test-reporter=\.\/scripts\/verify\/test-duration-reporter\.mjs/);
  assert.match(packageJson.scripts['test:unit'], /vitest-progress-reporter\.mjs/);
  assert.equal(packageJson.scripts.build, 'node scripts/build/build-web.mjs');
  assert.match(buildWebScript, /\['--noEmit'\]/);
  assert.match(buildWebScript, /\['build', '--logLevel', 'warn'\]/);
  assert.match(buildFpkWebScript, /\['exec', 'vite', 'build', '--logLevel', 'warn'\]/);
  assert.doesNotMatch(buildFpkWebScript, /\['run', 'build'\]/);

  assert.match(packageLocalScript, /\['run', 'verify'\]/);
  assert.match(packageLocalScript, /build-fpk-all\.mjs', '--reuse-web-dist'/);
  assert.match(packageLocalScript, /\['run', 'verify:fpk'\]/);

  assert.ok(
    buildAllScript.indexOf("await run('node', webArgs") < buildAllScript.indexOf('for (const target of targets)'),
    '双架构循环前应只准备一次 Web UI',
  );
  assert.match(buildAllScript, /runCommandWithProgress/);
  assert.match(buildServerScript, /cargoProgressDetail/);
  assert.match(buildFpkScript, /reportCommandProgress/);
  assert.match(buildAllScript, /'--reuse-web-ui'/);
  assert.match(buildFpkScript, /if \(!reuseWebUi\)/);
});
