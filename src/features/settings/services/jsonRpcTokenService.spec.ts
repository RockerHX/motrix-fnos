import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPut } from "../../../services/http";
import { getJsonRpcTokenStatus, updateJsonRpcToken } from "./jsonRpcTokenService";

vi.mock("../../../services/http", () => ({ httpGet: vi.fn(), httpPut: vi.fn() }));

describe("jsonRpcTokenService", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the dedicated masked-token endpoints", () => {
    getJsonRpcTokenStatus();
    updateJsonRpcToken("new-token");
    updateJsonRpcToken("");

    expect(httpGet).toHaveBeenCalledWith("/api/settings/jsonrpc-token");
    expect(httpPut).toHaveBeenNthCalledWith(1, "/api/settings/jsonrpc-token", { token: "new-token" });
    expect(httpPut).toHaveBeenNthCalledWith(2, "/api/settings/jsonrpc-token", { token: "" });
  });
});
