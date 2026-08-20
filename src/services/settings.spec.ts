import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPut } from "./http";
import { getAppConfig, saveAppConfig } from "./settings";

vi.mock("./http", () => ({ httpGet: vi.fn(), httpPut: vi.fn() }));

describe("settings service", () => {
  beforeEach(() => vi.clearAllMocks());

  it("never sends a legacy JSON-RPC Token in ordinary settings requests", () => {
    getAppConfig();
    saveAppConfig({
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
      jsonRpcToken: "legacy-secret",
    } as never);

    expect(httpGet).toHaveBeenCalledWith("/api/settings");
    expect(httpPut).toHaveBeenCalledWith("/api/settings", {
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
    });
    expect(JSON.stringify(vi.mocked(httpPut).mock.calls)).not.toContain("legacy-secret");
  });
});
