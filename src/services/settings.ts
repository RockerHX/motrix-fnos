import { httpGet, httpPut } from "./http";
import {
  DEFAULT_CONNECT_TIMEOUT,
  DEFAULT_HTTP_MAX_CONNECTIONS,
  DEFAULT_MAX_TRIES,
  type AppConfig,
} from "../types/settings";

export function getAppConfig(): Promise<AppConfig> {
  return httpGet<AppConfig>("/api/settings");
}

export function saveAppConfig(payload: AppConfig): Promise<AppConfig> {
  return httpPut<AppConfig>("/api/settings", {
    defaultDownloadDir: payload.defaultDownloadDir,
    maxConcurrentDownloads: payload.maxConcurrentDownloads,
    httpMaxConnections: payload.httpMaxConnections ?? DEFAULT_HTTP_MAX_CONNECTIONS,
    connectTimeout: payload.connectTimeout ?? DEFAULT_CONNECT_TIMEOUT,
    maxTries: payload.maxTries ?? DEFAULT_MAX_TRIES,
    downloadLimit: payload.downloadLimit,
    uploadLimit: payload.uploadLimit,
    language: payload.language,
  });
}
