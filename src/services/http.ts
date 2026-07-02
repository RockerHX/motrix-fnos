export interface ApiErrorResponse {
  code: string;
  message: string;
}

export class ApiError extends Error {
  code: string;
  status: number;

  constructor(status: number, payload: ApiErrorResponse) {
    super(payload.message || `请求失败（${status}）`);
    this.name = "ApiError";
    this.code = payload.code;
    this.status = status;
  }
}

interface RequestOptions {
  body?: unknown;
  headers?: HeadersInit;
}

async function request<T>(method: string, path: string, options: RequestOptions = {}): Promise<T> {
  const response = await fetch(path, {
    method,
    headers: {
      ...(options.body === undefined ? {} : { "content-type": "application/json" }),
      ...options.headers,
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });

  if (response.status === 204) {
    return undefined as T;
  }

  const contentType = response.headers.get("content-type") || "";
  const isJson = contentType.includes("application/json");
  const payload = isJson ? ((await response.json()) as unknown) : await response.text();

  if (!response.ok) {
    const errorPayload = isApiErrorResponse(payload)
      ? payload
      : {
          code: "http_error",
          message: typeof payload === "string" && payload ? payload : `请求失败（${response.status}）`,
        };
    throw new ApiError(response.status, errorPayload);
  }

  return payload as T;
}

function isApiErrorResponse(payload: unknown): payload is ApiErrorResponse {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Record<string, unknown>;
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

export function httpGet<T>(path: string) {
  return request<T>("GET", path);
}

export function httpPost<T>(path: string, body?: unknown) {
  return request<T>("POST", path, { body });
}

export function httpPut<T>(path: string, body: unknown) {
  return request<T>("PUT", path, { body });
}

export function httpDelete<T>(path: string) {
  return request<T>("DELETE", path);
}
