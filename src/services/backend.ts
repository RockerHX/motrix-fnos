import { invoke } from "@tauri-apps/api/core";
import type { AppInfo, BackendPing } from "../types/app";

export function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export function pingBackend(): Promise<BackendPing> {
  return invoke<BackendPing>("ping_backend");
}
