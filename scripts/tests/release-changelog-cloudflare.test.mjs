import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createCloudflareWorkersAICompletion,
  DEFAULT_CLOUDFLARE_ANALYSIS_MODEL,
  DEFAULT_CLOUDFLARE_EDITOR_MODEL,
} from '../release/release-changelog-cloudflare.mjs';

const accountId = '0123456789abcdef0123456789abcdef';

test('Cloudflare Workers AI 使用 OpenAI-compatible endpoint 和对应角色模型', async () => {
  const requests = [];
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    analysisModel: '@cf/qwen/analysis',
    editorModel: '@cf/openai/editor',
    fetchImpl: async (url, init) => {
      requests.push({ url, init });
      return Response.json({ choices: [{ message: { content: '[{"ok":true}]' } }] });
    },
  });

  const content = await complete({
    modelRole: 'analysis',
    systemPrompt: 'system',
    userPrompt: 'user',
    maxTokens: 321,
    label: '提交分析',
  });

  assert.equal(content, '[{"ok":true}]');
  assert.equal(
    requests[0].url,
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1/chat/completions`,
  );
  assert.equal(requests[0].init.headers.Authorization, 'Bearer test-token');
  assert.deepEqual(JSON.parse(requests[0].init.body), {
    model: '@cf/qwen/analysis',
    temperature: 0.2,
    max_tokens: 321,
    messages: [
      { role: 'system', content: 'system' },
      { role: 'user', content: 'user' },
    ],
  });

  await complete({
    modelRole: 'editor',
    systemPrompt: 'system',
    userPrompt: 'user',
    maxTokens: 123,
    label: '日志编辑',
  });
  assert.equal(JSON.parse(requests[1].init.body).model, '@cf/openai/editor');
});

test('Cloudflare Workers AI 对临时错误按 Retry-After 重试', async () => {
  const delays = [];
  const retryMessages = [];
  let attempts = 0;
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    fetchImpl: async () => {
      attempts += 1;
      if (attempts === 1) {
        return new Response('rate limited', { status: 429, headers: { 'Retry-After': '2' } });
      }
      return Response.json({ choices: [{ message: { content: '[]' } }] });
    },
    sleep: async (delayMs) => delays.push(delayMs),
    onRetry: (message) => retryMessages.push(message),
  });

  const content = await complete({
    modelRole: 'editor',
    systemPrompt: 'system',
    userPrompt: 'user',
    maxTokens: 100,
    label: '日志审稿',
  });

  assert.equal(content, '[]');
  assert.equal(attempts, 2);
  assert.deepEqual(delays, [2_000]);
  assert.match(retryMessages[0], /429.*2 秒后重试/);
});

test('Cloudflare Workers AI 校验凭证并报告不可重试错误', async () => {
  assert.throws(
    () => createCloudflareWorkersAICompletion({ accountId: '', apiToken: 'test-token' }),
    /CLOUDFLARE_ACCOUNT_ID/,
  );
  assert.throws(
    () => createCloudflareWorkersAICompletion({ accountId, apiToken: '' }),
    /CLOUDFLARE_API_TOKEN/,
  );

  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    fetchImpl: async () => new Response('{"errors":[{"message":"unauthorized"}]}', { status: 401 }),
  });
  await assert.rejects(
    () => complete({
      modelRole: 'analysis',
      systemPrompt: 'system',
      userPrompt: 'user',
      maxTokens: 100,
      label: '提交分析',
    }),
    /401.*检查 Cloudflare 凭证和 Workers AI 配额/,
  );
});

test('Cloudflare Workers AI 默认使用 gpt-oss-120b', () => {
  assert.equal(DEFAULT_CLOUDFLARE_ANALYSIS_MODEL, '@cf/openai/gpt-oss-120b');
  assert.equal(DEFAULT_CLOUDFLARE_EDITOR_MODEL, '@cf/openai/gpt-oss-120b');
});
