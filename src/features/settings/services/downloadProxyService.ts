import { httpDelete, httpGet, httpPut } from "../../../services/http";
import type { DownloadProxyMutationResponse, DownloadProxyStatus } from "../../../types/settings";

export function getDownloadProxyStatus() {
  return httpGet<DownloadProxyStatus>("/api/settings/proxy");
}

export function updateDownloadProxy(proxyUrl: string) {
  return httpPut<DownloadProxyMutationResponse>("/api/settings/proxy", { proxyUrl });
}

export function deleteDownloadProxy() {
  return httpDelete<void>("/api/settings/proxy");
}
