import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { language } from "../../../i18n";
import { getAccessiblePaths, refreshAccessiblePaths } from "../../../services/storage";
import { getAppConfig, saveAppConfig } from "../../../services/settings";
import type { AppConfig } from "../../../types/settings";
import { useSettingsStore } from "./settingsStore";

vi.mock("../../../services/storage", () => ({ getAccessiblePaths: vi.fn(), refreshAccessiblePaths: vi.fn() }));
vi.mock("../../../services/settings", () => ({ getAppConfig: vi.fn(), saveAppConfig: vi.fn() }));

const mockedGetAccessiblePaths = vi.mocked(getAccessiblePaths);
const mockedRefreshAccessiblePaths = vi.mocked(refreshAccessiblePaths);
const mockedGetAppConfig = vi.mocked(getAppConfig);
const mockedSaveAppConfig = vi.mocked(saveAppConfig);

describe("settingsStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads config, normalizes language and restores loading state", async () => {
    const store = useSettingsStore();
    const deferred = createDeferred<AppConfig>();
    mockedGetAppConfig.mockReturnValueOnce(deferred.promise);

    const promise = store.loadConfig();
    expect(store.isLoading).toBe(true);

    deferred.resolve(config({ language: "unsupported" as AppConfig["language"] }));
    await expect(promise).resolves.toMatchObject({ language: "zh-CN" });
    expect(store.config?.language).toBe("zh-CN");
    expect(language.value).toBe("zh-CN");
    expect(store.isLoading).toBe(false);
  });

  it("normalizes language before and after saving", async () => {
    const store = useSettingsStore();
    mockedSaveAppConfig.mockResolvedValueOnce(config({ language: "invalid" as AppConfig["language"] }));

    await expect(store.saveConfig(config({ language: "invalid" as AppConfig["language"] }))).resolves.toMatchObject({
      language: "zh-CN",
    });

    expect(mockedSaveAppConfig).toHaveBeenCalledWith(config({ language: "zh-CN" }));
    expect(store.isSaving).toBe(false);
  });

  it("loads accessible paths and clears earlier errors", async () => {
    const store = useSettingsStore();
    store.accessiblePathsError = "旧错误";
    mockedGetAccessiblePaths.mockResolvedValueOnce({ paths: ["/vol1/downloads"] });

    await expect(store.loadAccessiblePaths()).resolves.toEqual(["/vol1/downloads"]);

    expect(store.accessiblePaths).toEqual(["/vol1/downloads"]);
    expect(store.accessiblePathsError).toBe("");
    expect(store.isLoadingAccessiblePaths).toBe(false);
  });

  it("clears paths, records the error and restores loading state on failure", async () => {
    const store = useSettingsStore();
    store.accessiblePaths = ["/old"];
    mockedGetAccessiblePaths.mockRejectedValueOnce(new Error("授权目录读取失败"));

    await expect(store.loadAccessiblePaths()).rejects.toThrow("授权目录读取失败");

    expect(store.accessiblePaths).toEqual([]);
    expect(store.accessiblePathsError).toBe("授权目录读取失败");
    expect(store.isLoadingAccessiblePaths).toBe(false);
  });

  it("refreshes from the official API and clears a previously empty snapshot", async () => {
    const store = useSettingsStore();
    store.accessiblePaths = ["/old"];
    mockedRefreshAccessiblePaths.mockResolvedValueOnce({ paths: [] });

    await expect(store.refreshAccessiblePaths()).resolves.toEqual([]);

    expect(store.accessiblePaths).toEqual([]);
    expect(store.accessiblePathsStale).toBe(false);
    expect(store.accessiblePathsError).toBe("");
  });

  it("keeps the last snapshot and reads the server snapshot when refresh fails", async () => {
    const store = useSettingsStore();
    store.accessiblePaths = ["/old"];
    mockedRefreshAccessiblePaths.mockRejectedValueOnce(new Error("上游不可用"));
    mockedGetAccessiblePaths.mockResolvedValueOnce({ paths: ["/confirmed"] });

    await expect(store.refreshAccessiblePaths()).rejects.toThrow("上游不可用");

    expect(store.accessiblePaths).toEqual(["/confirmed"]);
    expect(store.accessiblePathsStale).toBe(true);
    expect(store.accessiblePathsError).toBe("上游不可用");
    expect(store.isLoadingAccessiblePaths).toBe(false);
  });

  it("keeps the in-memory snapshot when both refresh and fallback read fail", async () => {
    const store = useSettingsStore();
    store.accessiblePaths = ["/old"];
    mockedRefreshAccessiblePaths.mockRejectedValueOnce(new Error("刷新失败"));
    mockedGetAccessiblePaths.mockRejectedValueOnce(new Error("读取失败"));

    await expect(store.refreshAccessiblePaths()).rejects.toThrow("刷新失败");

    expect(store.accessiblePaths).toEqual(["/old"]);
    expect(store.accessiblePathsStale).toBe(true);
  });
});

function config(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    defaultDownloadDir: "/downloads",
    maxConcurrentDownloads: 5,
    downloadLimit: 0,
    uploadLimit: 0,
    language: "zh-CN",
    ...overrides,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
