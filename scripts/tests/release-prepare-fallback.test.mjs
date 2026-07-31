import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const releasePrepareScript = fileURLToPath(new URL('../release-prepare.mjs', import.meta.url));

test('未配置模型 provider 时 release prepare 使用确定性 commit log', () => {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-prepare-'));
  try {
    writeFileSync(path.join(repoRoot, 'package.json'), '{"version":"1.0.0"}\n');
    writeFileSync(path.join(repoRoot, 'CHANGELOG.md'), '# Changelog\n');
    mkdirSync(path.join(repoRoot, 'src'), { recursive: true });
    mkdirSync(path.join(repoRoot, 'server'), { recursive: true });
    mkdirSync(path.join(repoRoot, 'packaging', 'fnos', 'app', 'ui'), { recursive: true });
    writeFileSync(path.join(repoRoot, 'server', 'Cargo.toml'), '[package]\nversion = "1.0.0"\n');
    writeFileSync(path.join(repoRoot, 'packaging', 'fnos', 'manifest.template'), 'version               = 1.0.0\n');
    writeFileSync(
      path.join(repoRoot, 'packaging', 'fnos', 'app', 'ui', 'config'),
      '{".url":{"motrix.Application":{"url":"/?v=1.0.0"}}}\n',
    );
    writeFileSync(path.join(repoRoot, 'src', 'feature.txt'), '初始内容\n');
    git(repoRoot, ['init', '--quiet']);
    git(repoRoot, ['config', 'user.name', 'Test User']);
    git(repoRoot, ['config', 'user.email', 'test@example.com']);
    git(repoRoot, ['add', '.']);
    git(repoRoot, ['commit', '--quiet', '-m', 'chore: 初始化']);
    git(repoRoot, ['tag', 'v1.0.0']);
    writeFileSync(path.join(repoRoot, 'src', 'feature.txt'), '修复后的内容\n');
    git(repoRoot, ['add', '.']);
    git(repoRoot, ['commit', '--quiet', '-m', 'fix: 修复下载任务创建失败']);

    const env = { ...process.env };
    delete env.MOTRIX_RELEASE_CHANGELOG_PROVIDER;
    delete env.MOTRIX_RELEASE_CHANGELOG_MODEL;
    delete env.MOTRIX_RELEASE_ANALYSIS_MODEL;
    delete env.MOTRIX_RELEASE_EDITOR_MODEL;
    const output = execFileSync(process.execPath, [releasePrepareScript, '1.0.1', '--dry-run'], {
      cwd: repoRoot,
      encoding: 'utf8',
      env,
    });

    assert.match(output, /CHANGELOG：commit log 生成/);
    assert.match(output, /### 修复\n\n- 修复下载任务创建失败/);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

function git(repoRoot, args) {
  execFileSync('git', args, { cwd: repoRoot, stdio: 'pipe' });
}
