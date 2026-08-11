import { httpGet, httpPut } from "../../../services/http";
import type { Aria2LogModeStatus } from "../types";

const ARIA2_LOG_MODE_PATH = "/api/diagnostics/aria2-log-mode";

export function getAria2LogMode(): Promise<Aria2LogModeStatus> {
  return httpGet<Aria2LogModeStatus>(ARIA2_LOG_MODE_PATH);
}

export function updateAria2LogMode(detailed: boolean): Promise<Aria2LogModeStatus> {
  return httpPut<Aria2LogModeStatus>(ARIA2_LOG_MODE_PATH, { detailed });
}
