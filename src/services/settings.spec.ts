import { beforeEach, describe, expect, it, vi } from "vitest";
import { httpGet, httpPut } from "./http";
import { getAppConfig, saveAppConfig } from "./settings";

vi.mock("./http", () => ({ httpGet: vi.fn(), httpPut: vi.fn() }));

describe("settings service", () => {
  beforeEach(() => vi.clearAllMocks());

  it("sends all public download settings without a legacy JSON-RPC Token", () => {
    getAppConfig();
    saveAppConfig({
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      maxConnectionPerServer: 6,
      split: 7,
      minSplitSize: "5M",
      connectTimeout: 90,
      maxTries: 8,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
      jsonRpcToken: "legacy-secret",
    } as never);

    expect(httpGet).toHaveBeenCalledWith("/api/settings");
    expect(httpPut).toHaveBeenCalledWith("/api/settings", {
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      maxConnectionPerServer: 6,
      split: 7,
      minSplitSize: "5M",
      connectTimeout: 90,
      maxTries: 8,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
    });
    expect(JSON.stringify(vi.mocked(httpPut).mock.calls)).not.toContain("legacy-secret");
  });

  it("fills stable tuning defaults for legacy callers", () => {
    saveAppConfig({
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
    });

    expect(httpPut).toHaveBeenCalledWith("/api/settings", {
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      maxConnectionPerServer: 1,
      split: 5,
      minSplitSize: "20M",
      connectTimeout: 60,
      maxTries: 5,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN",
    });
  });

  it("preserves the optional runtime apply status from the save response", async () => {
    const response = {
      defaultDownloadDir: "/downloads",
      maxConcurrentDownloads: 5,
      downloadLimit: 0,
      uploadLimit: 0,
      language: "zh-CN" as const,
      runtimeApply: "deferred" as const,
    };
    vi.mocked(httpPut).mockResolvedValueOnce(response);

    await expect(
      saveAppConfig({
        defaultDownloadDir: "/downloads",
        maxConcurrentDownloads: 5,
        downloadLimit: 0,
        uploadLimit: 0,
        language: "zh-CN",
      }),
    ).resolves.toEqual(response);
  });
});
