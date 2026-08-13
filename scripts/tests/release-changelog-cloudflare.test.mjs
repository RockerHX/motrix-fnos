import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createCloudflareWorkersAICompletion,
  DEFAULT_CLOUDFLARE_ANALYSIS_MODEL,
  DEFAULT_CLOUDFLARE_EDITOR_MODEL,
  formatCloudflareGatewayDailyUsage,
  formatCloudflareWorkersAIUsage,
  readCloudflareGatewayDailyUsage,
} from '../release/release-changelog-cloudflare.mjs';

const accountId = '0123456789abcdef0123456789abcdef';

test('Cloudflare Workers AI 使用 OpenAI-compatible endpoint 和对应角色模型', async () => {
  const requests = [];
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    analysisModel: '@cf/qwen/analysis',
    editorModel: '@cf/openai/editor',
    gatewayId: 'motrix-fnos-release',
    metadata: { repository: 'RockerHX/motrix-fnos', run_id: '123', version: '1.9.2' },
    fetchImpl: async (url, init) => {
      requests.push({ url, init });
      return Response.json({
        choices: [{ message: { content: '[{"ok":true}]' } }],
        usage: { prompt_tokens: 1_000, completion_tokens: 100 },
      });
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
  assert.equal(requests[0].init.headers['cf-aig-gateway-id'], 'motrix-fnos-release');
  assert.equal(requests[0].init.headers['cf-aig-collect-log'], 'true');
  assert.equal(requests[0].init.headers['cf-aig-collect-log-payload'], 'false');
  assert.equal(requests[0].init.headers['cf-aig-skip-cache'], 'true');
  assert.deepEqual(JSON.parse(requests[0].init.headers['cf-aig-metadata']), {
    repository: 'RockerHX/motrix-fnos',
    run_id: '123',
    version: '1.9.2',
    stage: '提交分析',
  });
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
  assert.equal(complete.usage.requests, 2);
  assert.equal(complete.usage.inputTokens, 2_000);
  assert.equal(complete.usage.outputTokens, 200);
  assert.equal(complete.usage.neuronEstimateComplete, false);
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

test('Cloudflare Workers AI 汇总本次 token 和 gpt-oss-120b 神经元', async () => {
  const complete = createCloudflareWorkersAICompletion({
    accountId,
    apiToken: 'test-token',
    fetchImpl: async () => Response.json({
      choices: [{ message: { content: '[]' } }],
      usage: { prompt_tokens: 10_000, completion_tokens: 1_000 },
    }),
  });

  await complete({
    modelRole: 'analysis',
    systemPrompt: 'system',
    userPrompt: 'user',
    maxTokens: 100,
    label: '提交分析',
  });

  assert.equal(complete.usage.requests, 1);
  assert.equal(complete.usage.inputTokens, 10_000);
  assert.equal(complete.usage.outputTokens, 1_000);
  assert.equal(complete.usage.neurons.toFixed(3), '386.362');
  assert.match(formatCloudflareWorkersAIUsage(complete.usage), /折算神经元：386\.36/);
  assert.match(formatCloudflareWorkersAIUsage(complete.usage), /预计剩余：9613\.64/);
});

test('Cloudflare AI Gateway 日志汇总今日用量且不读取 payload', async () => {
  const requests = [];
  const delays = [];
  let attempt = 0;
  const usage = await readCloudflareGatewayDailyUsage({
    accountId,
    apiToken: 'test-token',
    gatewayId: 'motrix-fnos-release',
    expectedMetadata: { run_id: '123', version: '1.9.2' },
    minimumExpectedRequests: 2,
    sleep: async (delayMs) => delays.push(delayMs),
    fetchImpl: async (url, init) => {
      requests.push({ url: String(url), init });
      attempt += 1;
      return Response.json({
        result: [
          {
            success: true,
            model: '@cf/openai/gpt-oss-120b',
            metadata: JSON.stringify({ run_id: '123', version: '1.9.2', stage: '提交分析' }),
            tokens_in: 1_000,
            tokens_out: 100,
          },
          {
            success: true,
            model: '@cf/openai/gpt-oss-120b',
            metadata: JSON.stringify({ run_id: attempt === 1 ? 'older' : '123', version: '1.9.2', stage: '版本编辑' }),
            tokens_in: 2_000,
            tokens_out: 200,
          },
        ],
        result_info: { total_count: 2 },
      });
    },
  });

  assert.equal(requests.length, 2);
  assert.deepEqual(delays, [2_000, 2_000]);
  assert.match(requests[0].url, /ai-gateway\/gateways\/motrix-fnos-release\/logs/);
  assert.doesNotMatch(requests[0].url, /model=/);
  assert.equal(requests[0].init.headers.Authorization, 'Bearer test-token');
  assert.equal(usage.logCount, 2);
  assert.equal(usage.observedReleaseLogCount, 2);
  assert.equal(usage.expectedReleaseLogCount, 2);
  assert.equal(usage.inputTokens, 3_000);
  assert.equal(usage.outputTokens, 300);
  assert.equal(usage.neurons.toFixed(4), '115.9086');
  assert.match(formatCloudflareGatewayDailyUsage(usage), /预计剩余：9884\.09/);
  assert.match(formatCloudflareGatewayDailyUsage(usage), /本次发布日志已落库：2\/2/);
});

test('Cloudflare AI Gateway 日志延迟时明确标注当日估算可能偏小', async () => {
  const usage = await readCloudflareGatewayDailyUsage({
    accountId,
    apiToken: 'test-token',
    gatewayId: 'motrix-fnos-release',
    expectedMetadata: { run_id: '123' },
    minimumExpectedRequests: 1,
    sleep: async () => {},
    fetchImpl: async () => Response.json({ result: [], result_info: { total_count: 0 } }),
  });

  assert.equal(usage.observedReleaseLogCount, 0);
  assert.match(formatCloudflareGatewayDailyUsage(usage), /本次发布日志已落库：0\/1/);
  assert.match(formatCloudflareGatewayDailyUsage(usage), /日志仍可能在异步写入/);
});

test('Cloudflare AI Gateway 遇到未知模型时不套用 gpt-oss-120b 费率', async () => {
  const usage = await readCloudflareGatewayDailyUsage({
    accountId,
    apiToken: 'test-token',
    gatewayId: 'motrix-fnos-release',
    sleep: async () => {},
    fetchImpl: async () => Response.json({
      result: [{
        success: true,
        model: '@cf/example/other-model',
        tokens_in: 1_000,
        tokens_out: 100,
      }],
      result_info: { total_count: 1 },
    }),
  });

  assert.equal(usage.neuronEstimateComplete, false);
  assert.match(formatCloudflareGatewayDailyUsage(usage), /无法估算免费额度占用/);
});
