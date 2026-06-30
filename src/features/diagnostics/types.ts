export type DebugLogLevel = "info" | "warn" | "error";

export interface DebugLogEntry {
  id: number;
  timestampMs: number;
  level: DebugLogLevel;
  module: string;
  message: string;
}
