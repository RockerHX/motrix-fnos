import { httpGet } from "./http";
import type { AppInfo, BackendPing } from "../types/app";

export function getAppInfo(): Promise<AppInfo> {
  return httpGet<AppInfo>("/api/app/info");
}

export function pingBackend(): Promise<BackendPing> {
  return httpGet<BackendPing>("/api/app/ping");
}
