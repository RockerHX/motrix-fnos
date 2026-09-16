import { httpGet, httpPut } from "./http";
import {
  DEFAULT_CONNECT_TIMEOUT,
  DEFAULT_MAX_CONNECTION_PER_SERVER,
  DEFAULT_MAX_TRIES,
  DEFAULT_MIN_SPLIT_SIZE,
  DEFAULT_SPLIT,
  type AppConfig,
} from "../types/settings";

export function getAppConfig(): Promise<AppConfig> {
  return httpGet<AppConfig>("/api/settings");
}

export function saveAppConfig(payload: AppConfig): Promise<AppConfig> {
  return httpPut<AppConfig>("/api/settings", {
    defaultDownloadDir: payload.defaultDownloadDir,
    maxConcurrentDownloads: payload.maxConcurrentDownloads,
    maxConnectionPerServer: payload.maxConnectionPerServer ?? DEFAULT_MAX_CONNECTION_PER_SERVER,
    split: payload.split ?? DEFAULT_SPLIT,
    minSplitSize: payload.minSplitSize ?? DEFAULT_MIN_SPLIT_SIZE,
    connectTimeout: payload.connectTimeout ?? DEFAULT_CONNECT_TIMEOUT,
    maxTries: payload.maxTries ?? DEFAULT_MAX_TRIES,
    downloadLimit: payload.downloadLimit,
    uploadLimit: payload.uploadLimit,
    language: payload.language,
  });
}
