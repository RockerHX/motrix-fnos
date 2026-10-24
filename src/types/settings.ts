import type { AppLanguage } from "../i18n";

export const DEFAULT_MAX_CONCURRENT_DOWNLOADS = 5;
export const MAX_CONCURRENT_DOWNLOADS_LIMIT = 128;
export const DEFAULT_HTTP_MAX_CONNECTIONS = 1;
export const HTTP_MAX_CONNECTIONS_LIMIT = 64;
export const DEFAULT_CONNECT_TIMEOUT = 60;
export const MAX_CONNECT_TIMEOUT_LIMIT = 300;
export const DEFAULT_MAX_TRIES = 5;
export const MAX_TRIES_LIMIT = 10;
export type RuntimeApplyStatus = "applied" | "deferred" | "failed";

export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  httpMaxConnections?: number;
  connectTimeout?: number;
  maxTries?: number;
  downloadLimit: number;
  uploadLimit: number;
  language: AppLanguage;
  runtimeApply?: RuntimeApplyStatus;
}

export interface JsonRpcTokenStatus {
  configured: boolean;
  maskedToken: string | null;
}

export interface LanJsonRpcStatus {
  enabled: boolean;
  available?: boolean;
  configured: boolean;
  maskedToken: string | null;
  allowSharedAddressSpace: boolean;
  port: number;
}

export interface LanJsonRpcMutationResponse {
  status: LanJsonRpcStatus;
  issuedToken: string | null;
}

export interface DownloadProxyStatus {
  configured: boolean;
  maskedProxyUrl: string | null;
  revision: number;
  btUseProxy: boolean;
  btRuntimeApply: "applied" | "deferred" | "blocked";
}

export interface DownloadProxyApplyFailure {
  taskId: number;
  code: string;
  message: string;
}

export interface DownloadProxyMutationResponse {
  status: DownloadProxyStatus;
  appliedTaskIds: number[];
  deferredTaskIds: number[];
  failed: DownloadProxyApplyFailure[];
}
