import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPost, httpPut } from "../../../services/http";
import {
  getLanJsonRpcStatus,
  rotateLanJsonRpcToken,
  updateLanJsonRpcConfig,
} from "./lanJsonRpcService";

vi.mock("../../../services/http", () => ({ httpGet: vi.fn(), httpPost: vi.fn(), httpPut: vi.fn() }));

describe("lanJsonRpcService", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the dedicated LAN status, switch, and rotation endpoints", () => {
    getLanJsonRpcStatus();
    updateLanJsonRpcConfig(true, true);
    rotateLanJsonRpcToken();

    expect(httpGet).toHaveBeenCalledWith("/api/settings/lan-jsonrpc");
    expect(httpPut).toHaveBeenCalledWith("/api/settings/lan-jsonrpc", {
      enabled: true,
      allowSharedAddressSpace: true,
    });
    expect(httpPost).toHaveBeenCalledWith("/api/settings/lan-jsonrpc/token", {});
  });
});
