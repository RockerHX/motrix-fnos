import OpenAI from 'openai';

const DEFAULT_MODEL = '@cf/openai/gpt-oss-120b';
const MAX_RETRIES = 3;
const REQUEST_TIMEOUT_MS = 120_000;

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
    });
  };
  return complete;
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
