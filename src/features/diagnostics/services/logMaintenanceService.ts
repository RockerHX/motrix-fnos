import { httpDelete, httpGet } from "../../../services/http";
import type { Aria2LogCleanupResponse, DiagnosticsLogUsage } from "../types";

const LOG_USAGE_PATH = "/api/diagnostics/logs";
const ARIA2_LOGS_PATH = "/api/diagnostics/aria2-logs";

export function getLogUsage(): Promise<DiagnosticsLogUsage> {
  return httpGet<DiagnosticsLogUsage>(LOG_USAGE_PATH);
}

export function clearAria2Logs(): Promise<Aria2LogCleanupResponse> {
  return httpDelete<Aria2LogCleanupResponse>(ARIA2_LOGS_PATH);
}
