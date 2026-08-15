import { httpGet, httpPost } from "./http";
import type { AccessiblePathsResponse } from "../types/storage";

export function getAccessiblePaths(): Promise<AccessiblePathsResponse> {
  return httpGet<AccessiblePathsResponse>("/api/storage/accessible-paths");
}

export function refreshAccessiblePaths(): Promise<AccessiblePathsResponse> {
  return httpPost<AccessiblePathsResponse>("/api/storage/accessible-paths/refresh");
}
