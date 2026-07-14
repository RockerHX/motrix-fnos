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
  rawBody?: BodyInit;
  headers?: HeadersInit;
  handleUnauthorized?: boolean;
}

let csrfTokenProvider: (() => string | null) | null = null;
let unauthorizedHandler: (() => void | Promise<void>) | null = null;
let isHandlingUnauthorized = false;

export function setCsrfTokenProvider(provider: (() => string | null) | null) {
  csrfTokenProvider = provider;
}

export function setUnauthorizedHandler(handler: (() => void | Promise<void>) | null) {
  unauthorizedHandler = handler;
}

async function request<T>(method: string, path: string, options: RequestOptions = {}): Promise<T> {
  const hasJsonBody = options.body !== undefined;
  const csrfToken = isWriteMethod(method) ? csrfTokenProvider?.() : null;
  const response = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: {
      ...(hasJsonBody ? { "content-type": "application/json" } : {}),
      ...(csrfToken ? { "X-CSRF-Token": csrfToken } : {}),
      ...options.headers,
    },
    body: options.rawBody ?? (hasJsonBody ? JSON.stringify(options.body) : undefined),
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
    const error = new ApiError(response.status, errorPayload);
    if (response.status === 401 && options.handleUnauthorized !== false && unauthorizedHandler && !isHandlingUnauthorized) {
      isHandlingUnauthorized = true;
      try {
        await unauthorizedHandler();
      } finally {
        isHandlingUnauthorized = false;
      }
    }
    throw error;
  }

  return payload as T;
}

function isWriteMethod(method: string) {
  return method === "POST" || method === "PUT" || method === "PATCH" || method === "DELETE";
}

function isApiErrorResponse(payload: unknown): payload is ApiErrorResponse {
  if (!payload || typeof payload !== "object") {
    return false;
  }

  const candidate = payload as Record<string, unknown>;
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

export function httpGet<T>(path: string, options: Pick<RequestOptions, "handleUnauthorized"> = {}) {
  return request<T>("GET", path, options);
}

export function httpPost<T>(path: string, body?: unknown, options: Pick<RequestOptions, "handleUnauthorized"> = {}) {
  return request<T>("POST", path, { body, ...options });
}

export function httpPostFormData<T>(path: string, formData: FormData) {
  return request<T>("POST", path, { rawBody: formData });
}

export function httpPut<T>(path: string, body: unknown, options: Pick<RequestOptions, "handleUnauthorized"> = {}) {
  return request<T>("PUT", path, { body, ...options });
}

export function httpDelete<T>(path: string) {
  return request<T>("DELETE", path);
}
