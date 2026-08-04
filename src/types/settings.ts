import type { AppLanguage } from "../i18n";

export interface AppConfig {
  defaultDownloadDir: string;
  maxConcurrentDownloads: number;
  downloadLimit: number;
  uploadLimit: number;
  language: AppLanguage;
}

export interface JsonRpcTokenStatus {
  configured: boolean;
  maskedToken: string | null;
}

export interface LanJsonRpcStatus {
  enabled: boolean;
  configured: boolean;
  maskedToken: string | null;
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
