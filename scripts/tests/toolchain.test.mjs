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
  assert.equal(packageJson.engines?.node, '>=24.0.0');
  assert.match(nodeVersion, /^24\.\d+\.\d+$/);
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

test('自动发版通过 Cloudflare Workers AI 生成 CHANGELOG', () => {
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const releasePrepareScript = readFileSync('scripts/release/release-prepare.mjs', 'utf8');
  const releaseAiScript = readFileSync('scripts/release/release-changelog-ai.mjs', 'utf8');

  assert.match(releaseWorkflow, /node scripts\/release\/release-prepare\.mjs "\$\{VERSION\}" --no-commit --no-tag/);
  assert.match(releaseWorkflow, /MOTRIX_RELEASE_CHANGELOG_PROVIDER:\s*cloudflare-workers-ai/);
  assert.match(releaseWorkflow, /MOTRIX_RELEASE_ANALYSIS_MODEL:\s*"@cf\/openai\/gpt-oss-120b"/);
  assert.match(releaseWorkflow, /MOTRIX_RELEASE_EDITOR_MODEL:\s*"@cf\/openai\/gpt-oss-120b"/);
  assert.match(releaseWorkflow, /CLOUDFLARE_ACCOUNT_ID:\s*\$\{\{ secrets\.CLOUDFLARE_ACCOUNT_ID \}\}/);
  assert.match(releaseWorkflow, /CLOUDFLARE_API_TOKEN:\s*\$\{\{ secrets\.CLOUDFLARE_API_TOKEN \}\}/);
  assert.match(releaseWorkflow, /CLOUDFLARE_AI_GATEWAY_ID:\s*motrix-fnos-release/);
  assert.doesNotMatch(releaseWorkflow, /github-models|models\.github\.ai|MOTRIX_RELEASE_MODEL_MIN_INTERVAL_MS/);
  assert.doesNotMatch(releaseWorkflow, /^\s*models:\s*read\s*$/m);
  assert.doesNotMatch(`${releasePrepareScript}\n${releaseAiScript}`, /github-models|models\.github\.ai|gpt-4\.1/);
});

test('提交、推送、远端验证和发版使用独立验证层级', () => {
  const prePushHook = readFileSync('.githooks/pre-push', 'utf8');
  const verifyWorkflow = readFileSync('.github/workflows/verify.yml', 'utf8');
  const releaseWorkflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const auditWorkflow = readFileSync('.github/workflows/dependency-audit.yml', 'utf8');

  assert.equal(existsSync('.githooks/pre-push'), true);
  assert.match(prePushHook, /run\(packageManager, \['run', 'verify'\]\)/);


  assert.match(verifyWorkflow, /workflow_dispatch:/);
  assert.match(verifyWorkflow, /^\s+push:\s*$/m);
  assert.match(verifyWorkflow, /^\s+pull_request:\s*$/m);
  assert.match(verifyWorkflow, /schedule:/);
  assert.match(verifyWorkflow, /cron:\s*"17 18 \* \* 6"/);
  assert.match(verifyWorkflow, /pnpm exec playwright install --with-deps chromium/);
  assert.match(verifyWorkflow, /run:\s+pnpm run verify:extended/);
  assert.ok(
    verifyWorkflow.indexOf('pnpm exec playwright install --with-deps chromium')
      < verifyWorkflow.indexOf('pnpm run verify:extended'),
    '扩展验证前必须安装浏览器',
  );
  assert.match(verifyWorkflow, /run:\s+pnpm run verify/);

  assert.match(releaseWorkflow, /git commit --no-verify/);
  assert.match(releaseWorkflow, /git push --no-verify --atomic/);
  assert.match(releaseWorkflow, /git push --no-verify origin/);
  assert.match(releaseWorkflow, /pnpm run build:fpk:artifacts/);
  assert.match(releaseWorkflow, /pnpm run verify:fpk/);
  assert.doesNotMatch(releaseWorkflow, /^\s*actions:\s*read\s*$/m);
  assert.doesNotMatch(releaseWorkflow, /Require successful main verification|gh run list|source_sha/);
  assert.doesNotMatch(releaseWorkflow, /cargo install cargo-audit|pnpm run audit:deps|pnpm run verify(?::(?!fpk\b)|\s)|pnpm run test:|playwright|cargo test/);

  assert.doesNotMatch(verifyWorkflow, /cargo install cargo-audit|pnpm run audit:deps/);
  assert.match(auditWorkflow, /schedule:/);
  assert.match(auditWorkflow, /cron:\s*"23 19 \* \* 0"/);
  assert.match(auditWorkflow, /cargo install cargo-audit --version 0\.22\.2 --locked/);
  assert.match(auditWorkflow, /pnpm run audit:deps/);
});

test('本地完整打包与 Release 产物构建复用明确的验证层级', () => {
  const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));

  assert.equal(packageJson.scripts['build:fpk'], 'node scripts/build/package-local.mjs');
  assert.equal(packageJson.scripts['verify:extended'], 'node scripts/verify/verify-extended.mjs');
  assert.equal(packageJson.scripts['build:fpk:release'], 'node scripts/build/package-release.mjs');
  assert.match(packageJson.scripts['test:e2e'], /playwright test/);
  assert.match(packageJson.scripts['test:aria2'], /scripts\/extended\/aria2-sidecar\.test\.mjs/);
  assert.match(packageJson.scripts['test:filesystem'], /run-filesystem-tests\.mjs/);
  assert.equal(packageJson.scripts['build:fpk:artifacts'], 'node scripts/build/build-fpk-all.mjs');
  assert.equal(packageJson.scripts['verify:fpk'], 'node scripts/verify/verify-fpk-artifacts.mjs');
  assert.match(packageJson.scripts['test:scripts'], /--test-reporter=\.\/scripts\/verify\/test-duration-reporter\.mjs/);
  assert.match(packageJson.scripts['test:unit'], /vitest-progress-reporter\.mjs/);
  assert.equal(packageJson.scripts.build, 'node scripts/build/build-web.mjs');
});
