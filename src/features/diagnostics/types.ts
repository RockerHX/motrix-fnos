export type DebugLogLevel = "info" | "warn" | "error";
export type DebugLogCategory = "app" | "task" | "aria2" | "settings" | "storage" | "api" | "runtime";

export interface DebugLogEntry {
  id: number;
  timestampMs: number;
  lastTimestampMs: number;
  level: DebugLogLevel;
  category: DebugLogCategory;
  module: string;
  message: string;
  repeatCount: number;
}
