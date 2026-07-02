import { httpDelete, httpGet } from "../../../services/http";
import type { DebugLogEntry } from "../types";

export function listDebugLogs(): Promise<DebugLogEntry[]> {
  return httpGet<DebugLogEntry[]>("/api/debug-logs");
}

export function clearDebugLogs(): Promise<void> {
  return httpDelete<void>("/api/debug-logs");
}
