import { httpGet } from "../../../services/http";
import type { AppUpdateCheck } from "../../../types/app";

export function checkAppUpdate(): Promise<AppUpdateCheck> {
  return httpGet<AppUpdateCheck>("/api/app/update-check");
}
