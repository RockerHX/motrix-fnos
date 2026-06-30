import { invoke } from "@tauri-apps/api/core";
import type { DebugLogEntry } from "../types";

export function listDebugLogs(): Promise<DebugLogEntry[]> {
  return invoke<DebugLogEntry[]>("list_debug_logs");
}

export function clearDebugLogs(): Promise<void> {
  return invoke<void>("clear_debug_logs");
}
