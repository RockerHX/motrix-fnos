import { httpGet } from "../../../services/http";
import type { DiagnosticsStorageUsage } from "../types";

const STORAGE_USAGE_PATH = "/api/diagnostics/storage";

export function getStorageUsage(): Promise<DiagnosticsStorageUsage> {
  return httpGet<DiagnosticsStorageUsage>(STORAGE_USAGE_PATH);
}
