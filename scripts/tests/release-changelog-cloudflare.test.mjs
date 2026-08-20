import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createCloudflareWorkersAICompletion,
  DEFAULT_CLOUDFLARE_ANALYSIS_MODEL,
  DEFAULT_CLOUDFLARE_EDITOR_MODEL,
} from '../release/release-changelog-cloudflare.mjs';

const accountId = '0123456789abcdef0123456789abcdef';

function responseWithOutputText(text, { incompleteDetails } = {}) {
  return Response.json({
    object: 'response',
    output: [{
      type: 'message',
      content: text ? [{ type: 'output_text', text }] : [],
    }],
    ...(incompleteDetails ? { incomplete_details: incompleteDetails } : {}),
  });
}

test('Cloudflare Workers AI 通过 OpenAI SDK 调用 Responses API', async () => {
  const requests = [];
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    analysisModel: '@cf/openai/gpt-oss-120b',
    editorModel: '@cf/qwen/editor',
    gatewayId: 'motrix-fnos-release',
    metadata: { repository: 'RockerHX/motrix-fnos', run_id: '123', version: '1.9.2' },
    fetchImpl: async (url, init) => {
      requests.push({ url, init });
      return responseWithOutputText('[{"ok":true}]');
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
    String(requests[0].url),
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1/responses`,
  );
  const headers = new Headers(requests[0].init.headers);
  assert.equal(headers.get('authorization'), 'Bearer test-token');
  assert.equal(headers.get('cf-aig-gateway-id'), 'motrix-fnos-release');
  assert.equal(headers.get('cf-aig-collect-log'), 'true');
  assert.equal(headers.get('cf-aig-collect-log-payload'), 'false');
  assert.equal(headers.get('cf-aig-skip-cache'), 'true');
  assert.deepEqual(JSON.parse(headers.get('cf-aig-metadata')), {
    repository: 'RockerHX/motrix-fnos',
    run_id: '123',
    version: '1.9.2',
    stage: '提交分析',
  });
  assert.deepEqual(JSON.parse(requests[0].init.body), {
    model: '@cf/openai/gpt-oss-120b',
    instructions: 'system',
    input: 'user',
    temperature: 0.2,
    max_output_tokens: 321,
    reasoning: { effort: 'low' },
  });

  await complete({
    modelRole: 'editor',
    systemPrompt: 'system',
    userPrompt: 'user',
    maxTokens: 123,
    label: '日志编辑',
  });
  assert.equal(JSON.parse(requests[1].init.body).model, '@cf/qwen/editor');
  assert.equal(JSON.parse(requests[1].init.body).reasoning, undefined);
});

test('Cloudflare Workers AI 由 OpenAI SDK 重试临时错误', async () => {
  let attempts = 0;
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    fetchImpl: async () => {
      attempts += 1;
      if (attempts === 1) {
        return new Response('rate limited', { status: 429, headers: { 'Retry-After': '0' } });
      }
      return responseWithOutputText('[]');
    },
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

test('Cloudflare Workers AI 响应缺少输出时报告原因', async () => {
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    fetchImpl: async () => responseWithOutputText('', {
      incompleteDetails: { reason: 'max_output_tokens' },
    }),
  });

  await assert.rejects(
    () => complete({
      modelRole: 'analysis',
      systemPrompt: 'system',
      userPrompt: 'user',
      maxTokens: 2_400,
      label: '提交分析',
    }),
    /缺少 output_text.*max_output_tokens/,
  );
});
