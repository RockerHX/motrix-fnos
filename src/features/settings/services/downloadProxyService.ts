import { httpDelete, httpGet, httpPut } from "../../../services/http";
import type { DownloadProxyMutationResponse, DownloadProxyStatus } from "../../../types/settings";

export function getDownloadProxyStatus() {
  return httpGet<DownloadProxyStatus>("/api/settings/proxy");
}

export function updateDownloadProxy(proxyUrl?: string, btUseProxy?: boolean) {
  const payload: { proxyUrl?: string; btUseProxy?: boolean } = {};
  if (proxyUrl !== undefined) payload.proxyUrl = proxyUrl;
  if (btUseProxy !== undefined) payload.btUseProxy = btUseProxy;
  return httpPut<DownloadProxyMutationResponse>("/api/settings/proxy", payload);
}

export function deleteDownloadProxy() {
  return httpDelete<void>("/api/settings/proxy");
}
