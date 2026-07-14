import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  buildChangelogPrompt,
  collectReleaseChangeContext,
  readReleaseCommits,
} from '../release-changelog-context.mjs';

test('发布上下文包含 commit body、文件统计和最终净文本补丁', () => {
  const repoRoot = createFixtureRepository();
  try {
    const commits = readReleaseCommits(repoRoot, 'v1.0.0');
    const context = collectReleaseChangeContext({ repoRoot, baseRef: 'v1.0.0', commits });
    const prompt = buildChangelogPrompt({ version: '1.1.0', baseRef: 'v1.0.0', changeContext: context });

    assert.equal(commits.length, 1);
    assert.equal(commits[0].subject, 'feat: 调整可见行为');
    assert.equal(commits[0].body, '让发布说明读取真实补丁。');
    assert.match(context.fileStatus, /M\s+src\/feature\.txt/);
    assert.match(context.numstat, /src\/feature\.txt/);
    assert.match(context.diffStat, /files? changed/);
    assert.match(context.patch, /新的用户可见行为/);
    assert.doesNotMatch(context.patch, /lockfileVersion: 2/);
    assert.doesNotMatch(context.patch, /generated bundle/);
    assert.deepEqual(
      context.omittedPatchFiles,
      [
        { path: 'asset.bin', reason: '二进制文件' },
        { path: 'dist/bundle.js', reason: '生成目录' },
        { path: 'pnpm-lock.yaml', reason: '锁文件' },
      ],
    );
    assert.match(prompt, /让发布说明读取真实补丁/);
    assert.match(prompt, /新的用户可见行为/);
    assert.match(prompt, /pnpm-lock\.yaml（锁文件）/);
    assert.match(prompt, /不要只改写 commit 标题/);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('发布文本补丁超过上限时拒绝截断生成', () => {
  const repoRoot = createFixtureRepository();
  try {
    assert.throws(
      () => collectReleaseChangeContext({ repoRoot, baseRef: 'v1.0.0', maxPatchBytes: 16 }),
      /超过 16 字节上限；请人工预写目标版本 CHANGELOG/,
    );
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

function createFixtureRepository() {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-context-'));
  git(repoRoot, ['init', '--quiet']);
  git(repoRoot, ['config', 'user.name', 'Test User']);
  git(repoRoot, ['config', 'user.email', 'test@example.com']);

  writeFileSync(path.join(repoRoot, 'pnpm-lock.yaml'), 'lockfileVersion: 1\n');
  writeFileSync(path.join(repoRoot, 'asset.bin'), Buffer.from([0, 1, 2, 3]));
  mkdirAndWrite(repoRoot, 'src/feature.txt', '旧行为\n');
  git(repoRoot, ['add', '.']);
  git(repoRoot, ['commit', '--quiet', '-m', 'chore: 初始化']);
  git(repoRoot, ['tag', 'v1.0.0']);

  writeFileSync(path.join(repoRoot, 'pnpm-lock.yaml'), 'lockfileVersion: 2\n');
  writeFileSync(path.join(repoRoot, 'asset.bin'), Buffer.from([0, 255, 2, 3]));
  mkdirAndWrite(repoRoot, 'dist/bundle.js', 'generated bundle\n');
  mkdirAndWrite(repoRoot, 'src/feature.txt', '新的用户可见行为\n');
  git(repoRoot, ['add', '.']);
  git(repoRoot, ['commit', '--quiet', '-m', 'feat: 调整可见行为', '-m', '让发布说明读取真实补丁。']);
  return repoRoot;
}

function mkdirAndWrite(repoRoot, relativePath, content) {
  const directory = path.dirname(path.join(repoRoot, relativePath));
  mkdirSync(directory, { recursive: true });
  writeFileSync(path.join(repoRoot, relativePath), content);
}

function git(repoRoot, args) {
  return execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8' });
}
