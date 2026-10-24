import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpDelete, httpGet, httpPut } from "../../../services/http";
import { deleteDownloadProxy, getDownloadProxyStatus, updateDownloadProxy } from "./downloadProxyService";

vi.mock("../../../services/http", () => ({ httpDelete: vi.fn(), httpGet: vi.fn(), httpPut: vi.fn() }));

describe("downloadProxyService", () => {
  beforeEach(() => vi.clearAllMocks());

  it("uses the dedicated proxy settings endpoints", () => {
    getDownloadProxyStatus();
    updateDownloadProxy("http://proxy.example.com:7890");
    updateDownloadProxy(undefined, true);
    deleteDownloadProxy();

    expect(httpGet).toHaveBeenCalledWith("/api/settings/proxy");
    expect(httpPut).toHaveBeenCalledWith("/api/settings/proxy", { proxyUrl: "http://proxy.example.com:7890" });
    expect(httpPut).toHaveBeenNthCalledWith(2, "/api/settings/proxy", { btUseProxy: true });
    expect(httpDelete).toHaveBeenCalledWith("/api/settings/proxy");
  });
});
