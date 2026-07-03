import { httpGet } from "./http";
import type { AccessiblePathsResponse } from "../types/storage";

export function getAccessiblePaths(): Promise<AccessiblePathsResponse> {
  return httpGet<AccessiblePathsResponse>("/api/storage/accessible-paths");
}
