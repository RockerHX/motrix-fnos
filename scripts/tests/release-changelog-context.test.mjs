import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {
  collectReleaseCommitContext,
  readReleaseCommits,
} from '../release-changelog-context.mjs';
import {
  buildReleaseAnalysisPrompts,
  countChatInputTokens,
  generateChangelogWithHierarchicalSummary,
  MODEL_INPUT_TOKEN_BUDGET,
  NoReleaseFactsError,
} from '../release-changelog-ai.mjs';

test('发布上下文只提供 commit 信息和本地统计，不包含源码 Diff', () => {
  const repoRoot = createFixtureRepository();
  try {
    const commits = readReleaseCommits(repoRoot, 'v1.0.0');
    const context = collectReleaseCommitContext({ repoRoot, baseRef: 'v1.0.0', commits });
    const prompts = buildReleaseAnalysisPrompts({ baseRef: 'v1.0.0', changeContext: context });
    const promptText = prompts.map((item) => item.prompt).join('\n');

    assert.equal(commits.length, 1);
    assert.equal(commits[0].subject, 'feat: 调整可见行为');
    assert.equal(commits[0].body, '让发布说明读取真实提交信息。');
    assert.match(context.fileStatus, /M\s+src\/feature\.txt/);
    assert.match(context.numstat, /src\/feature\.txt/);
    assert.match(context.diffStat, /files? changed/);
    assert.match(promptText, /commit [0-9a-f]+/);
    assert.match(promptText, /让发布说明读取真实提交信息/);
    assert.match(promptText, /evidenceCommits/);
    assert.doesNotMatch(promptText, /diff --git|<diff>|新的用户可见行为/);
    assert.equal('patch' in context, false);
    assert.equal('patchFiles' in context, false);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test('大型 commit 信息使用分批事实合并、编辑和独立审稿', async () => {
  const commits = Array.from({ length: 32 }, (_, index) => ({
    hash: `commit${String(index + 1).padStart(3, '0')}`,
    subject: index % 2 === 0 ? `feat: 增加能力 ${index + 1}` : `fix: 修正能力 ${index + 1}`,
    body: '用户可见变化和最终行为说明。'.repeat(90),
  }));
  const changeContext = { commits };
  const calls = [];
  let analysisSequence = 0;
  let consolidationAttempts = 0;
  const result = await generateChangelogWithHierarchicalSummary({
    version: '1.1.0',
    baseRef: 'v1.0.0',
    changeContext,
    complete: async (request) => {
      calls.push(request);
      if (request.label === 'Release 日志编辑') {
        return JSON.stringify([
          { category: '新增', text: '增加最终能力。', factIds: ['final-feature'] },
        ]);
      }
      if (request.label === 'Release 日志审稿') {
        return JSON.stringify([
          { category: '新增', text: '增加经过多次修正后的最终能力。', factIds: ['final-feature'] },
        ]);
      }
      if (request.label.startsWith('结构化事实合并')) {
        consolidationAttempts += 1;
        if (consolidationAttempts === 1) return '[{"factId":"final-feature"';
        return JSON.stringify([
          {
            factId: 'final-feature',
            category: '新增',
            summary: '增加经过多次修正后的最终能力',
            releaseRelevant: true,
            evidenceCommits: ['commit001'],
            confidence: 'high',
          },
        ]);
      }
      analysisSequence += 1;
      return JSON.stringify(
        Array.from({ length: 8 }, (_, index) => ({
          factId: `fragment-${analysisSequence}-${index + 1}`,
          category: '新增',
          summary: '增加经过多次修正后的最终能力。'.repeat(8),
          releaseRelevant: true,
          evidenceCommits: ['commit001'],
          confidence: 'high',
        })),
      );
    },
  });

  assert.ok(calls.filter((call) => call.label.startsWith('提交信息')).length > 1);
  assert.ok(calls.some((call) => call.label.startsWith('结构化事实合并')));
  assert.ok(consolidationAttempts >= 2);
  const consolidationCalls = calls.filter((call) => call.label.startsWith('结构化事实合并'));
  assert.ok(consolidationCalls[1].maxTokens > consolidationCalls[0].maxTokens);
  assert.equal(calls.find((call) => call.label === 'Release 日志编辑').modelRole, 'editor');
  assert.equal(calls.find((call) => call.label === 'Release 日志审稿').modelRole, 'editor');
  for (const call of calls) {
    assert.ok(countChatInputTokens(call.systemPrompt, call.userPrompt) <= MODEL_INPUT_TOKEN_BUDGET);
    assert.doesNotMatch(call.userPrompt, /diff --git|<diff>/);
  }
  assert.equal(result, '### 新增\n\n- 增加经过多次修正后的最终能力。');
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
          if (request.label.startsWith('提交信息')) {
            return JSON.stringify([
              {
                factId: 'auth-feedback',
                category: '修复',
                summary: '优化鉴权启动反馈',
                releaseRelevant: true,
                evidenceCommits: ['aaaaaaa'],
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

test('模型没有可发布事实时返回可识别的降级错误', async () => {
  const changeContext = smallChangeContext();

  await assert.rejects(
    () =>
      generateChangelogWithHierarchicalSummary({
        version: '1.1.0',
        baseRef: 'v1.0.0',
        changeContext,
        complete: async () =>
          JSON.stringify([
            {
              factId: 'internal-refactor',
              category: '内部',
              summary: '内部实现调整',
              releaseRelevant: false,
              evidenceCommits: ['aaaaaaa'],
              confidence: 'high',
            },
          ]),
      }),
    (error) => error instanceof NoReleaseFactsError,
  );
});

function smallChangeContext() {
  return {
    commits: [{ hash: 'aaaaaaa', subject: 'fix: 修复行为', body: '修复任务状态显示。' }],
  };
}

function createFixtureRepository() {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'motrix-release-context-'));
  git(repoRoot, ['init', '--quiet']);
  git(repoRoot, ['config', 'user.name', 'Test User']);
  git(repoRoot, ['config', 'user.email', 'test@example.com']);

  mkdirAndWrite(repoRoot, 'src/feature.txt', '旧行为\n');
  git(repoRoot, ['add', '.']);
  git(repoRoot, ['commit', '--quiet', '-m', 'chore: 初始化']);
  git(repoRoot, ['tag', 'v1.0.0']);

  mkdirAndWrite(repoRoot, 'src/feature.txt', '新的用户可见行为\n');
  git(repoRoot, ['add', '.']);
  git(repoRoot, ['commit', '--quiet', '-m', 'feat: 调整可见行为', '-m', '让发布说明读取真实提交信息。']);
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
