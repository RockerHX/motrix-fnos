import { httpGet, httpPut } from "./http";
import type { AppConfig, UiPreferences } from "../types/settings";

export function getAppConfig(): Promise<AppConfig> {
  return httpGet<AppConfig>("/api/settings");
}

export function saveAppConfig(payload: AppConfig): Promise<AppConfig> {
  return httpPut<AppConfig>("/api/settings", payload);
}

export function getUiPreferences(): Promise<UiPreferences> {
  return httpGet<UiPreferences>("/api/ui-preferences");
}

export function saveUiPreferences(payload: UiPreferences): Promise<UiPreferences> {
  return httpPut<UiPreferences>("/api/ui-preferences", payload);
}
