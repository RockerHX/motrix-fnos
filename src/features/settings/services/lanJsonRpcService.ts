import { httpGet, httpPost, httpPut } from "../../../services/http";
import type { LanJsonRpcMutationResponse, LanJsonRpcStatus } from "../../../types/settings";

export function getLanJsonRpcStatus() {
  return httpGet<LanJsonRpcStatus>("/api/settings/lan-jsonrpc");
}

export function updateLanJsonRpcConfig(enabled: boolean, allowSharedAddressSpace: boolean) {
  return httpPut<LanJsonRpcMutationResponse>("/api/settings/lan-jsonrpc", {
    enabled,
    allowSharedAddressSpace,
  });
}

export function rotateLanJsonRpcToken() {
  return httpPost<LanJsonRpcMutationResponse>("/api/settings/lan-jsonrpc/token", {});
}
