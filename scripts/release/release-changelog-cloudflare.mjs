const DEFAULT_MODEL = '@cf/openai/gpt-oss-120b';
const RETRYABLE_STATUS_CODES = new Set([429, 500, 502, 503, 504]);
const MAX_RETRIES = 3;
const MAX_RETRY_DELAY_MS = 60_000;
const REQUEST_TIMEOUT_MS = 120_000;

export const DEFAULT_CLOUDFLARE_ANALYSIS_MODEL = DEFAULT_MODEL;
export const DEFAULT_CLOUDFLARE_EDITOR_MODEL = DEFAULT_MODEL;

export function createCloudflareWorkersAICompletion({
  accountId,
  apiToken,
  analysisModel = DEFAULT_CLOUDFLARE_ANALYSIS_MODEL,
  editorModel = DEFAULT_CLOUDFLARE_EDITOR_MODEL,
  fetchImpl = globalThis.fetch,
  sleep = (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs)),
  onRetry = () => {},
}) {
  if (!/^[a-f0-9]{32}$/i.test(accountId ?? '')) {
    throw new Error('缺少合法的 CLOUDFLARE_ACCOUNT_ID（应为 32 位十六进制 Account ID）');
  }
  if (!apiToken?.trim()) {
    throw new Error('缺少 CLOUDFLARE_API_TOKEN');
  }
  if (typeof fetchImpl !== 'function') {
    throw new Error('当前 Node.js 环境不支持 fetch');
  }

  const endpoint = `https://api.cloudflare.com/client/v4/accounts/${accountId}/ai/v1/chat/completions`;

  return async ({ modelRole, systemPrompt, userPrompt, maxTokens, label }) => {
    const model = modelRole === 'analysis' ? analysisModel : editorModel;
    return requestCloudflareWorkersAI({
      endpoint,
      apiToken,
      model,
      systemPrompt,
      userPrompt,
      maxTokens,
      label,
      fetchImpl,
      sleep,
      onRetry,
    });
  };
}

async function requestCloudflareWorkersAI({
  endpoint,
  apiToken,
  model,
  systemPrompt,
  userPrompt,
  maxTokens,
  label,
  fetchImpl,
  sleep,
  onRetry,
}) {
  for (let attempt = 0; attempt <= MAX_RETRIES; attempt += 1) {
    const response = await fetchImpl(endpoint, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${apiToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model,
        temperature: 0.2,
        max_tokens: maxTokens,
        messages: [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: userPrompt },
        ],
      }),
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
    });

    if (RETRYABLE_STATUS_CODES.has(response.status) && attempt < MAX_RETRIES) {
      const delayMs = retryDelayMs(response.headers, attempt);
      onRetry(
        `${label}调用 Cloudflare Workers AI（${model}）暂时失败（${response.status}），`
          + `${Math.ceil(delayMs / 1_000)} 秒后重试（${attempt + 2}/${MAX_RETRIES + 1}）`,
      );
      await sleep(delayMs);
      continue;
    }

    if (!response.ok) {
      const responseText = (await response.text()).slice(0, 2_000);
      throw new Error(
        `${label}调用 Cloudflare Workers AI（${model}）失败：${response.status} ${responseText}. `
          + '请检查 Cloudflare 凭证和 Workers AI 配额，或提前在 CHANGELOG.md 写入目标版本条目。',
      );
    }

    const data = await response.json();
    const content = data?.choices?.[0]?.message?.content;
    if (typeof content !== 'string' || !content.trim()) {
      throw new Error(
        `${label}调用 Cloudflare Workers AI（${model}）的响应缺少 choices[0].message.content`,
      );
    }
    return content;
  }

  throw new Error(`${label}调用 Cloudflare Workers AI 失败：超过重试次数`);
}

function retryDelayMs(headers, attempt) {
  const retryAfter = headers.get('retry-after');
  if (retryAfter) {
    const seconds = Number(retryAfter);
    if (Number.isFinite(seconds)) {
      return Math.min(MAX_RETRY_DELAY_MS, Math.max(1_000, seconds * 1_000));
    }
    const retryAt = Date.parse(retryAfter);
    if (Number.isFinite(retryAt)) {
      return Math.min(MAX_RETRY_DELAY_MS, Math.max(1_000, retryAt - Date.now()));
    }
  }
  return Math.min(MAX_RETRY_DELAY_MS, 2_000 * 2 ** attempt);
}
