import type { AppLanguage } from "../i18n";

export const DEFAULT_MAX_CONCURRENT_DOWNLOADS = 5;
export const MAX_CONCURRENT_DOWNLOADS_LIMIT = 128;
export const DEFAULT_MAX_CONNECTION_PER_SERVER = 1;
export const MAX_CONNECTION_PER_SERVER_LIMIT = 64;
export const DEFAULT_SPLIT = 5;
export const MAX_SPLIT_LIMIT = 64;
export const MIN_SPLIT_SIZE_OPTIONS = ["1M", "5M", "10M", "20M"] as const;
export const DEFAULT_MIN_SPLIT_SIZE = "20M";
export const DEFAULT_CONNECT_TIMEOUT = 60;
export const MAX_CONNECT_TIMEOUT_LIMIT = 300;
export const DEFAULT_MAX_TRIES = 5;
export const MAX_TRIES_LIMIT = 10;
export type RuntimeApplyStatus = "applied" | "deferred" | "failed";

export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  maxConnectionPerServer?: number;
  split?: number;
  minSplitSize?: string;
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
