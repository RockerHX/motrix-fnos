import OpenAI from 'openai';

const DEFAULT_MODEL = '@cf/openai/gpt-oss-120b';
const MAX_RETRIES = 3;
const REQUEST_TIMEOUT_MS = 120_000;
const GATEWAY_LOG_RETRIES = 3;
const GATEWAY_LOG_RETRY_DELAY_MS = 2_000;
// Cloudflare AI Gateway Logs API rejects values above 50.
const GATEWAY_LOG_PAGE_SIZE = 50;
const GATEWAY_LOG_MAX_PAGES = 100;
const FREE_DAILY_NEURONS = 10_000;
const GPT_OSS_120B_INPUT_NEURONS_PER_MILLION_TOKENS = 31_818;
const GPT_OSS_120B_OUTPUT_NEURONS_PER_MILLION_TOKENS = 68_182;
const MODEL_NEURON_RATES = new Map([
  [DEFAULT_MODEL, {
    input: GPT_OSS_120B_INPUT_NEURONS_PER_MILLION_TOKENS,
    output: GPT_OSS_120B_OUTPUT_NEURONS_PER_MILLION_TOKENS,
  }],
]);

export const DEFAULT_CLOUDFLARE_ANALYSIS_MODEL = DEFAULT_MODEL;
export const DEFAULT_CLOUDFLARE_EDITOR_MODEL = DEFAULT_MODEL;

export function createCloudflareWorkersAICompletion({
  accountId,
  apiToken,
  analysisModel = DEFAULT_CLOUDFLARE_ANALYSIS_MODEL,
  editorModel = DEFAULT_CLOUDFLARE_EDITOR_MODEL,
  gatewayId,
  metadata = {},
  fetchImpl = globalThis.fetch,
}) {
  if (!/^[a-f0-9]{32}$/i.test(accountId ?? '')) {
    throw new Error('缺少合法的 CLOUDFLARE_ACCOUNT_ID（应为 32 位十六进制 Account ID）');
  }
  if (!apiToken?.trim()) {
    throw new Error('缺少 CLOUDFLARE_API_TOKEN');
  }
  if (gatewayId !== undefined && !/^[a-z0-9][a-z0-9_-]*$/i.test(gatewayId)) {
    throw new Error('CLOUDFLARE_AI_GATEWAY_ID 只能包含字母、数字、下划线和连字符');
  }
  if (typeof fetchImpl !== 'function') {
    throw new Error('当前 Node.js 环境不支持 fetch');
  }

  const client = new OpenAI({
    apiKey: apiToken,
    baseURL: `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1`,
    timeout: REQUEST_TIMEOUT_MS,
    maxRetries: MAX_RETRIES,
    defaultHeaders: gatewayId ? {
      'cf-aig-gateway-id': gatewayId,
      'cf-aig-collect-log': 'true',
      'cf-aig-collect-log-payload': 'false',
      'cf-aig-skip-cache': 'true',
    } : undefined,
    fetch: fetchImpl,
  });
  const usage = createUsageSummary();

  const complete = async ({ modelRole, systemPrompt, userPrompt, maxTokens, label }) => {
    const model = modelRole === 'analysis' ? analysisModel : editorModel;
    return requestCloudflareWorkersAI({
      client,
      model,
      systemPrompt,
      userPrompt,
      maxTokens,
      label,
      gatewayId,
      metadata: { ...metadata, stage: label },
      usage,
    });
  };
  complete.usage = usage;
  return complete;
}

export function formatCloudflareWorkersAIUsage(usage) {
  if (!usage.neuronEstimateComplete) {
    return [
      'Cloudflare Workers AI 本次用量：',
      `- 请求：${usage.requests} 次`,
      `- 输入 token：${usage.inputTokens}`,
      `- 输出 token：${usage.outputTokens}`,
      `- 合计 token：${usage.inputTokens + usage.outputTokens}`,
      '- 当前模型未配置神经元费率，无法估算免费额度占用。',
    ].join('\n');
  }
  const remaining = Math.max(0, FREE_DAILY_NEURONS - usage.neurons);
  return [
    'Cloudflare Workers AI 本次用量：',
    `- 请求：${usage.requests} 次`,
    `- 输入 token：${usage.inputTokens}`,
    `- 输出 token：${usage.outputTokens}`,
    `- 合计 token：${usage.inputTokens + usage.outputTokens}`,
    `- 折算神经元：${usage.neurons.toFixed(2)}`,
    `- 占每日 10,000 免费神经元：${(usage.neurons / FREE_DAILY_NEURONS * 100).toFixed(2)}%`,
    `- 若今日只有本次调用，预计剩余：${remaining.toFixed(2)} 神经元`,
    '- 账户实际剩余以 Cloudflare Workers AI Dashboard 为准。',
  ].join('\n');
}

export async function readCloudflareGatewayDailyUsage({
  accountId,
  apiToken,
  gatewayId,
  expectedMetadata = {},
  minimumExpectedRequests = 0,
  fetchImpl = globalThis.fetch,
  sleep = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
}) {
  if (!gatewayId) return null;
  const startDate = new Date();
  startDate.setUTCHours(0, 0, 0, 0);
  const endpoint = new URL(
    `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai-gateway/gateways/${encodeURIComponent(gatewayId)}/logs`,
  );
  endpoint.searchParams.set('start_date', startDate.toISOString());
  endpoint.searchParams.set('per_page', String(GATEWAY_LOG_PAGE_SIZE));
  endpoint.searchParams.set('order_by', 'created_at');
  endpoint.searchParams.set('order_by_direction', 'desc');

  await sleep(GATEWAY_LOG_RETRY_DELAY_MS);
  let logs;
  let matchingLogCount = 0;
  for (let attempt = 0; attempt < GATEWAY_LOG_RETRIES; attempt += 1) {
    logs = await readGatewayLogPages({ endpoint, apiToken, fetchImpl });
    matchingLogCount = logs.filter((log) => (
      log.success === true && gatewayMetadataMatches(log.metadata, expectedMetadata)
    )).length;
    if (matchingLogCount >= minimumExpectedRequests || attempt === GATEWAY_LOG_RETRIES - 1) break;
    await sleep(GATEWAY_LOG_RETRY_DELAY_MS);
  }

  const usage = createUsageSummary();
  for (const log of logs) {
    recordUsage(usage, { input_tokens: log.tokens_in, output_tokens: log.tokens_out }, log.model);
  }
  return {
    ...usage,
    gatewayId,
    logCount: logs.length,
    observedReleaseLogCount: matchingLogCount,
    expectedReleaseLogCount: minimumExpectedRequests,
    startDate: startDate.toISOString(),
  };
}

async function readGatewayLogPages({ endpoint, apiToken, fetchImpl }) {
  const firstPage = await requestGatewayLogPage({ endpoint, page: 1, apiToken, fetchImpl });
  const logs = [...firstPage.logs];
  const totalCount = nonNegativeNumber(firstPage.totalCount);
  const pageSize = nonNegativeNumber(firstPage.perPage) || GATEWAY_LOG_PAGE_SIZE;
  const totalPages = Math.min(
    GATEWAY_LOG_MAX_PAGES,
    Math.max(1, Math.ceil(totalCount / pageSize)),
  );
  for (let page = 2; page <= totalPages; page += 1) {
    const nextPage = await requestGatewayLogPage({ endpoint, page, apiToken, fetchImpl });
    logs.push(...nextPage.logs);
  }
  return logs;
}

async function requestGatewayLogPage({ endpoint, page, apiToken, fetchImpl }) {
  endpoint.searchParams.set('page', String(page));
  const response = await fetchImpl(endpoint, {
    headers: { Authorization: `Bearer ${apiToken}` },
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  if (!response.ok) {
    const responseText = (await response.text()).slice(0, 2_000);
    throw new Error(`读取 Cloudflare AI Gateway 日志失败：${response.status} ${responseText}`);
  }
  const data = await response.json();
  return {
    logs: Array.isArray(data?.result) ? data.result : [],
    totalCount: data?.result_info?.total_count,
    perPage: data?.result_info?.per_page,
  };
}

function gatewayMetadataMatches(rawMetadata, expectedMetadata) {
  const expectedEntries = Object.entries(expectedMetadata);
  if (expectedEntries.length === 0) return true;
  try {
    const metadata = typeof rawMetadata === 'string' ? JSON.parse(rawMetadata) : rawMetadata;
    return expectedEntries.every(([key, value]) => metadata?.[key] === value);
  } catch {
    return false;
  }
}

export function formatCloudflareGatewayDailyUsage(usage) {
  if (!usage) return '';
  const releaseLogStatus = usage.expectedReleaseLogCount > 0
    ? `- 本次发布日志已落库：${usage.observedReleaseLogCount}/${usage.expectedReleaseLogCount} 条`
    : null;
  const persistenceWarning = usage.observedReleaseLogCount < usage.expectedReleaseLogCount
    ? '- Cloudflare 日志仍可能在异步写入；本次精确 token 以上方模型响应统计为准。'
    : null;
  if (!usage.neuronEstimateComplete) {
    return [
      `Cloudflare AI Gateway 今日累计（${usage.gatewayId}，自 00:00 UTC）：`,
      `- 已记录请求：${usage.logCount} 次`,
      releaseLogStatus,
      `- 输入 token：${usage.inputTokens}`,
      `- 输出 token：${usage.outputTokens}`,
      '- 当前模型未配置神经元费率，无法估算免费额度占用。',
      persistenceWarning,
    ].filter(Boolean).join('\n');
  }
  const remaining = Math.max(0, FREE_DAILY_NEURONS - usage.neurons);
  return [
    `Cloudflare AI Gateway 今日累计（${usage.gatewayId}，自 00:00 UTC）：`,
    `- 已记录请求：${usage.logCount} 次`,
    releaseLogStatus,
    `- 输入 token：${usage.inputTokens}`,
    `- 输出 token：${usage.outputTokens}`,
    `- 折算神经元：${usage.neurons.toFixed(2)}`,
    `- Gateway 口径预计剩余：${remaining.toFixed(2)} / ${FREE_DAILY_NEURONS} 神经元`,
    '- 该数字不包含绕过此 Gateway 的 Workers AI 调用，账户实际剩余以 Workers AI Dashboard 为准。',
    persistenceWarning,
  ].filter(Boolean).join('\n');
}

async function requestCloudflareWorkersAI({
  client,
  model,
  systemPrompt,
  userPrompt,
  maxTokens,
  label,
  gatewayId,
  metadata,
  usage,
}) {
  let response;
  try {
    response = await client.responses.create(
      {
        model,
        instructions: systemPrompt,
        input: userPrompt,
        max_output_tokens: maxTokens,
        temperature: 0.2,
        ...(isGptOssModel(model) ? { reasoning: { effort: 'low' } } : {}),
      },
      gatewayId ? { headers: { 'cf-aig-metadata': stringifyHeaderJson(metadata) } } : undefined,
    );
  } catch (error) {
    throw new Error(
      `${label}调用 Cloudflare Workers AI（${model}）失败：${error instanceof Error ? error.message : String(error)}. `
        + '请检查 Cloudflare 凭证和 Workers AI 配额，或提前在 CHANGELOG.md 写入目标版本条目。',
      { cause: error },
    );
  }

  recordUsage(usage, response.usage, model);
  const content = response.output_text;
  if (typeof content !== 'string' || !content.trim()) {
    const incompleteReason = response.incomplete_details?.reason;
    throw new Error(
      `${label}调用 Cloudflare Workers AI（${model}）的响应缺少 output_text`
        + (incompleteReason ? `（未完成原因：${incompleteReason}）` : ''),
    );
  }
  return content;
}

function stringifyHeaderJson(value) {
  return JSON.stringify(value).replace(/[^\x00-\x7F]/g, (character) => (
    `\\u${character.charCodeAt(0).toString(16).padStart(4, '0')}`
  ));
}

function isGptOssModel(model) {
  return /^@cf\/openai\/gpt-oss-/i.test(model);
}

function createUsageSummary() {
  return { requests: 0, inputTokens: 0, outputTokens: 0, neurons: 0, neuronEstimateComplete: true };
}

function recordUsage(summary, responseUsage, model = DEFAULT_MODEL) {
  const inputTokens = nonNegativeNumber(responseUsage?.prompt_tokens ?? responseUsage?.input_tokens);
  const outputTokens = nonNegativeNumber(responseUsage?.completion_tokens ?? responseUsage?.output_tokens);
  const rates = MODEL_NEURON_RATES.get(model);
  summary.requests += 1;
  summary.inputTokens += inputTokens;
  summary.outputTokens += outputTokens;
  if (rates) {
    summary.neurons += (inputTokens * rates.input + outputTokens * rates.output) / 1_000_000;
  } else {
    summary.neuronEstimateComplete = false;
  }
}

function nonNegativeNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : 0;
}
