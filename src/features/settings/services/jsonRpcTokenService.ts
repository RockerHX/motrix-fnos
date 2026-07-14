import { httpGet, httpPut } from "../../../services/http";
import type { JsonRpcTokenStatus } from "../../../types/settings";

export function getJsonRpcTokenStatus() {
  return httpGet<JsonRpcTokenStatus>("/api/settings/jsonrpc-token");
}

export function updateJsonRpcToken(token: string) {
  return httpPut<JsonRpcTokenStatus>("/api/settings/jsonrpc-token", { token });
}
