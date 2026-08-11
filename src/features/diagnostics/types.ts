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

export type Aria2LogLevel = "warn" | "debug";

export interface Aria2LogModeStatus {
  mode: Aria2LogLevel;
  detailed: boolean;
  detailedUntilMs: number | null;
  maxFileSizeBytes: number;
  maxFileCount: number;
  appliesOnNextStart: boolean;
}
