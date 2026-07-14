import { httpGet, httpPut } from "./http";
import type { AppConfig } from "../types/settings";

export function getAppConfig(): Promise<AppConfig> {
  return httpGet<AppConfig>("/api/settings");
}

export function saveAppConfig(payload: AppConfig): Promise<AppConfig> {
  return httpPut<AppConfig>("/api/settings", {
    defaultDownloadDir: payload.defaultDownloadDir,
    maxConcurrentDownloads: payload.maxConcurrentDownloads,
    downloadLimit: payload.downloadLimit,
    uploadLimit: payload.uploadLimit,
    language: payload.language,
  });
}
