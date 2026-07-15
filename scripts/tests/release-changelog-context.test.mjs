import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  collectReleaseChangeContext,
  readReleaseCommits,
} from '../release-changelog-context.mjs';
import {
  buildReleaseAnalysisPrompts,
  countChatInputTokens,
  generateChangelogWithHierarchicalSummary,
  MODEL_INPUT_TOKEN_BUDGET,
} from '../release-changelog-ai.mjs';

test('发布上下文包含 commit body、文件统计和最终净文本补丁', () => {
  const repoRoot = createFixtureRepository();
  try {
    const commits = readReleaseCommits(repoRoot, 'v1.0.0');
    const context = collectReleaseChangeContext({ repoRoot, baseRef: 'v1.0.0', commits });
    const prompts = buildReleaseAnalysisPrompts({ baseRef: 'v1.0.0', changeContext: context });

    assert.equal(commits.length, 1);
    assert.equal(commits[0].subject, 'feat: 调整可见行为');
    assert.equal(commits[0].body, '让发布说明读取真实补丁。');
    assert.match(context.fileStatus, /M\s+src\/feature\.txt/);
    assert.match(context.numstat, /src\/feature\.txt/);
    assert.match(context.diffStat, /files? changed/);
    assert.match(context.patch, /新的用户可见行为/);
    assert.equal(context.patchFiles.length, 1);
    assert.equal(context.patchFiles[0].path, 'src/feature.txt');
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
    const promptText = prompts.map((item) => item.prompt).join('\n');
    assert.match(promptText, /让发布说明读取真实补丁/);
    assert.match(promptText, /新的用户可见行为/);
    assert.match(promptText, /pnpm-lock\.yaml（锁文件）/);
    assert.match(promptText, /实际结果以最终净 Diff 为准/);
    assert.match(promptText, /releaseRelevant/);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('大型最终净 Diff 使用结构化事实、双模型编辑和独立审稿', async () => {
  const largePatch = `diff --git a/src/large.ts b/src/large.ts\n${'+const value = "用户可见变化";\n'.repeat(12_000)}`;
  const changeContext = {
    commits: [
      { hash: 'aaaaaaa', subject: 'feat: 增加能力', body: '' },
      { hash: 'bbbbbbb', subject: 'fix: 反复修正能力', body: '' },
    ],
    fileStatus: 'M\tsrc/large.ts',
    numstat: '12000\t0\tsrc/large.ts',
    diffStat: '1 file changed, 12000 insertions(+)',
    patch: largePatch,
    patchFiles: [{ path: 'src/large.ts', patch: largePatch }],
    omittedPatchFiles: [],
  };
  const calls = [];
  let analysisSequence = 0;
  const result = await generateChangelogWithHierarchicalSummary({
    version: '1.1.0',
    baseRef: 'v1.0.0',
    changeContext,
    complete: async (request) => {
      calls.push(request);
      if (request.label === 'Release 日志编辑') {
        return JSON.stringify([
          {
            category: '新增',
            text: '增加最终能力。',
            factIds: ['final-feature'],
          },
        ]);
      }
      if (request.label === 'Release 日志审稿') {
        return JSON.stringify([
          {
            category: '新增',
            text: '增加经过多次修正后的最终能力。',
            factIds: ['final-feature'],
          },
        ]);
      }
      if (request.label.startsWith('结构化事实合并')) {
        return JSON.stringify([
          {
            factId: 'final-feature',
            category: '新增',
            summary: '增加经过多次修正后的最终能力',
            releaseRelevant: true,
            evidencePaths: ['src/large.ts'],
            confidence: 'high',
          },
        ]);
      }
      analysisSequence += 1;
      return JSON.stringify(
        Array.from({ length: 8 }, (_, index) => ({
          factId: `fragment-${analysisSequence}-${index + 1}`,
          category: '新增',
          summary: '增加经过多次修正后的最终能力',
          releaseRelevant: true,
          evidencePaths: ['src/large.ts'],
          confidence: 'high',
        })),
      );
    },
  });

  assert.ok(calls.length > 3);
  assert.ok(calls.some((call) => call.label.startsWith('结构化事实合并')));
  assert.equal(calls.find((call) => call.label === 'Release 日志编辑').modelRole, 'editor');
  assert.equal(calls.find((call) => call.label === 'Release 日志审稿').modelRole, 'editor');
  for (const call of calls) {
    assert.ok(countChatInputTokens(call.systemPrompt, call.userPrompt) <= MODEL_INPUT_TOKEN_BUDGET);
  }
  assert.equal(result, '### 新增\n\n- 增加经过多次修正后的最终能力。');
  assert.doesNotMatch(calls.at(-1).userPrompt, /const value/);
  assert.match(calls.at(-1).userPrompt, /候选日志/);
});

test('审稿结果引用重复事实或测试噪声时阻止发布', async () => {
  const changeContext = smallChangeContext();

  await assert.rejects(
    () =>
      generateChangelogWithHierarchicalSummary({
        version: '1.1.0',
        baseRef: 'v1.0.0',
        changeContext,
        complete: async (request) => {
          if (request.label.startsWith('发布变更分块')) {
            return JSON.stringify([
              {
                factId: 'auth-feedback',
                category: '修复',
                summary: '优化鉴权启动反馈',
                releaseRelevant: true,
                evidencePaths: ['src/feature.ts'],
                confidence: 'high',
              },
            ]);
          }
          return JSON.stringify([
            { category: '修复', text: '增加鉴权反馈的单元测试。', factIds: ['auth-feedback'] },
          ]);
        },
      }),
    /包含测试、内部实现或空泛描述/,
  );
});

test('任一分块模型调用失败会阻止发布', async () => {
  const changeContext = smallChangeContext();

  await assert.rejects(
    () =>
      generateChangelogWithHierarchicalSummary({
        version: '1.1.0',
        baseRef: 'v1.0.0',
        changeContext,
        complete: async () => {
          throw new Error('模型不可用');
        },
      }),
    /模型不可用/,
  );
});

function smallChangeContext() {
  return {
    commits: [{ hash: 'aaaaaaa', subject: 'fix: 修复行为', body: '' }],
    fileStatus: 'M\tsrc/feature.ts',
    numstat: '1\t1\tsrc/feature.ts',
    diffStat: '1 file changed',
    patch: 'diff --git a/src/feature.ts b/src/feature.ts\n-old\n+new',
    patchFiles: [{ path: 'src/feature.ts', patch: 'diff --git a/src/feature.ts b/src/feature.ts\n-old\n+new' }],
    omittedPatchFiles: [],
  };
}

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
