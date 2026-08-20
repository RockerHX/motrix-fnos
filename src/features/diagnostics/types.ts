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

export interface LogFileUsage {
  currentBytes: number;
  historyBytes: number;
  totalBytes: number;
  currentFileCount: number;
  historyFileCount: number;
  totalFileCount: number;
}

export interface DiagnosticsLogUsage {
  aria2: LogFileUsage;
  server: LogFileUsage;
  lifecycle: LogFileUsage;
  totalBytes: number;
  totalFileCount: number;
  aria2LogMode: Aria2LogModeStatus;
}

export interface Aria2LogCleanupResponse {
  reclaimedBytes: number;
  usage: DiagnosticsLogUsage;
}
