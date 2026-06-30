import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, UiPreferences } from "../types/settings";

export function getAppConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_app_config");
}

export function saveAppConfig(payload: AppConfig): Promise<AppConfig> {
  return invoke<AppConfig>("save_app_config", { payload });
}

export function getUiPreferences(): Promise<UiPreferences> {
  return invoke<UiPreferences>("get_ui_preferences");
}

export function saveUiPreferences(payload: UiPreferences): Promise<UiPreferences> {
  return invoke<UiPreferences>("save_ui_preferences", { payload });
}
