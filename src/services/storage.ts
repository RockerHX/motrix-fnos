import { httpGet, httpPost } from "./http";
import type { AppLanguage } from "../i18n";
import type { AccessiblePathsResponse, DisplayAccessiblePathsResponse } from "../types/storage";

export function getAccessiblePaths(): Promise<AccessiblePathsResponse> {
  return httpGet<AccessiblePathsResponse>("/api/storage/accessible-paths");
}

export function refreshAccessiblePaths(): Promise<AccessiblePathsResponse> {
  return httpPost<AccessiblePathsResponse>("/api/storage/accessible-paths/refresh");
}

export function getDisplayAccessiblePaths(language: AppLanguage): Promise<DisplayAccessiblePathsResponse> {
  return httpGet<DisplayAccessiblePathsResponse>(
    `/api/storage/accessible-paths/display?language=${encodeURIComponent(language)}`,
  );
}
