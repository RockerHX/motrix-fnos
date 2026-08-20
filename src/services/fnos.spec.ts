import { describe, expect, it, vi } from "vitest";
import { FnosHostAdapter } from "./fnos";

function adapter(options: {
  isWeb?: boolean;
  isStandaloneWeb?: boolean;
  ready?: () => Promise<void>;
  pickSharedFile?: () => Promise<unknown>;
  getPlatformConfig?: () => Promise<unknown>;
  setTitle?: (title: string) => Promise<unknown>;
  openFile?: (path: string) => Promise<unknown>;
  openFileManager?: (path: string) => Promise<unknown>;
  showFileDetails?: (paths: string[]) => Promise<unknown>;
  on?: (event: string, listener: (...args: unknown[]) => void) => Promise<void>;
  off?: (event: string, listener: (...args: unknown[]) => void) => Promise<void>;
} = {}) {
  const app = {
    isWeb: options.isWeb ?? true,
    isStandaloneWeb: options.isStandaloneWeb ?? false,
    ready: options.ready ?? vi.fn().mockResolvedValue(undefined),
    pickSharedFile: options.pickSharedFile ?? vi.fn().mockResolvedValue({ code: 0, msg: "", data: ["/vol1/private"] }),
    getPlatformConfig:
      options.getPlatformConfig ??
      vi.fn().mockResolvedValue({
        theme: "dark",
        language: "zh-CN",
        systemVersion: "1.2.0401",
        format: {},
      }),
    setTitle: options.setTitle ?? vi.fn().mockResolvedValue(undefined),
    openFile: options.openFile ?? vi.fn().mockResolvedValue(undefined),
    openFileManager: options.openFileManager ?? vi.fn().mockResolvedValue(undefined),
    showFileDetails: options.showFileDetails ?? vi.fn().mockResolvedValue(undefined),
    $on: options.on ?? vi.fn().mockResolvedValue(undefined),
    $off: options.off ?? vi.fn().mockResolvedValue(undefined),
  };
  const load = vi.fn().mockResolvedValue({ TrimApp: class { constructor() { return app; } } });
  return { adapter: new FnosHostAdapter(load), app, load };
}

describe("FnosHostAdapter", () => {
  it("classifies desktop and mobile hosts after SDK readiness", async () => {
    const desktop = adapter();
    const mobile = adapter({ isWeb: false });

    await expect(desktop.adapter.getHostKind()).resolves.toBe("hosted");
    await expect(mobile.adapter.getHostKind()).resolves.toBe("mobile");
    expect(desktop.load).toHaveBeenCalledOnce();
  });

  it("keeps dynamic import or readiness failures from escaping into the app", async () => {
    const importFailure = new FnosHostAdapter(vi.fn().mockRejectedValue(new Error("module failed")));
    const readinessFailure = adapter({ ready: vi.fn().mockRejectedValue(new Error("runtime failed")) });

    await expect(importFailure.getHostKind()).resolves.toBe("unavailable");
    await expect(importFailure.requestSharedFolderAuthorization()).resolves.toEqual({ status: "unsupported" });
    await expect(readinessFailure.adapter.getHostKind()).resolves.toBe("unavailable");
  });

  it("never calls App runtime methods from an independent browser", async () => {
    const standalone = adapter({ isStandaloneWeb: true });

    await expect(standalone.adapter.getHostKind()).resolves.toBe("standalone");
    await expect(standalone.adapter.requestSharedFolderAuthorization()).resolves.toEqual({ status: "unsupported" });
    await expect(standalone.adapter.getPlatformConfig()).resolves.toBeNull();
    await expect(standalone.adapter.setTitle("Motrix")).resolves.toEqual({ status: "unsupported" });
    await expect(standalone.adapter.openFile("/vol1/file")).resolves.toEqual({ status: "unsupported" });
    await expect(standalone.adapter.openFileManager("/vol1/file")).resolves.toEqual({ status: "unsupported" });
    await expect(standalone.adapter.showFileDetails(["/vol1/file"])).resolves.toEqual({ status: "unsupported" });
    await expect(standalone.adapter.subscribeTheme(vi.fn())).resolves.toMatchObject({ status: "unsupported" });
    expect(standalone.app.pickSharedFile).not.toHaveBeenCalled();
    expect(standalone.app.getPlatformConfig).not.toHaveBeenCalled();
    expect(standalone.app.setTitle).not.toHaveBeenCalled();
    expect(standalone.app.openFile).not.toHaveBeenCalled();
    expect(standalone.app.openFileManager).not.toHaveBeenCalled();
    expect(standalone.app.showFileDetails).not.toHaveBeenCalled();
    expect(standalone.app.$on).not.toHaveBeenCalled();
  });

  it("returns authorization success without exposing SDK paths", async () => {
    const hosted = adapter();

    const result = await hosted.adapter.requestSharedFolderAuthorization();

    expect(result).toEqual({ status: "authorized" });
    expect(JSON.stringify(result)).not.toContain("/vol1/private");
  });

  it.each([
    [undefined, "cancelled"],
    [{ code: 1, msg: "仅管理员可进行此操作", data: [] }, "admin_required"],
    [{ code: 1_000_030, msg: "unsupported", data: [] }, "unsupported"],
    [{ code: 1_000_000, msg: "failed", data: [] }, "failed"],
  ])("classifies picker response %#", async (response, status) => {
    const hosted = adapter({ pickSharedFile: vi.fn().mockResolvedValue(response) });
    await expect(hosted.adapter.requestSharedFolderAuthorization()).resolves.toEqual({ status });
  });

  it.each([
    [new Error("Operation failed"), "cancelled"],
    [new Error("仅管理员可进行此操作"), "admin_required"],
    [new Error("App runtime is not supported"), "unsupported"],
    [new Error("unexpected bridge error"), "failed"],
  ])("classifies picker exception %#", async (error, status) => {
    const hosted = adapter({ pickSharedFile: vi.fn().mockRejectedValue(error) });
    await expect(hosted.adapter.requestSharedFolderAuthorization()).resolves.toEqual({ status });
  });

  it("reads platform config and wraps title and file host actions", async () => {
    const hosted = adapter();

    await expect(hosted.adapter.getPlatformConfig()).resolves.toMatchObject({
      theme: "dark",
      language: "zh-CN",
    });
    await expect(hosted.adapter.setTitle("Motrix")).resolves.toEqual({ status: "opened" });
    await expect(hosted.adapter.openFile("/vol1/file.pdf")).resolves.toEqual({ status: "opened" });
    await expect(hosted.adapter.openFileManager("/vol1/file.pdf")).resolves.toEqual({ status: "opened" });
    await expect(hosted.adapter.showFileDetails(["/vol1/file.pdf"])).resolves.toEqual({ status: "opened" });

    expect(hosted.app.setTitle).toHaveBeenCalledWith("Motrix");
    expect(hosted.app.openFile).toHaveBeenCalledWith("/vol1/file.pdf");
    expect(hosted.app.openFileManager).toHaveBeenCalledWith("/vol1/file.pdf");
    expect(hosted.app.showFileDetails).toHaveBeenCalledWith(["/vol1/file.pdf"]);
    expect(hosted.app.showFileDetails).toHaveBeenCalledTimes(1);
  });

  it("does not pass admin options to showFileDetails", async () => {
    const showFileDetails = vi.fn().mockResolvedValue(undefined);
    const hosted = adapter({ showFileDetails });

    await hosted.adapter.showFileDetails(["/vol1/file.pdf"]);

    expect(showFileDetails).toHaveBeenCalledWith(["/vol1/file.pdf"]);
    expect(showFileDetails.mock.calls[0]).toHaveLength(1);
  });

  it("subscribes to desktop theme and language events and unsubscribes once", async () => {
    const listeners = new Map<string, (...args: unknown[]) => void>();
    const on = vi.fn(async (event: string, listener: (...args: unknown[]) => void) => {
      listeners.set(event, listener);
    });
    const off = vi.fn().mockResolvedValue(undefined);
    const hosted = adapter({ on, off });
    const themes: string[] = [];
    const languages: string[] = [];

    const themeSubscription = await hosted.adapter.subscribeTheme((theme) => themes.push(theme));
    const languageSubscription = await hosted.adapter.subscribeLanguage((language) => languages.push(language));
    listeners.get("os/theme")?.("light");
    listeners.get("os/theme")?.("unknown");
    listeners.get("os/language")?.("en-US");
    themeSubscription.unsubscribe();
    themeSubscription.unsubscribe();
    languageSubscription.unsubscribe();

    expect(themes).toEqual(["light"]);
    expect(languages).toEqual(["en-US"]);
    expect(off).toHaveBeenCalledTimes(2);
  });

  it("does not subscribe to Web-only events in mobile hosts", async () => {
    const mobile = adapter({ isWeb: false });

    await expect(mobile.adapter.subscribeTheme(vi.fn())).resolves.toMatchObject({ status: "unsupported" });
    await expect(mobile.adapter.subscribeLanguage(vi.fn())).resolves.toMatchObject({ status: "unsupported" });
    expect(mobile.app.$on).not.toHaveBeenCalled();
  });

  it("normalizes host action and subscription failures", async () => {
    const hosted = adapter({
      setTitle: vi.fn().mockRejectedValue(new Error("failed")),
      openFile: vi.fn().mockRejectedValue(new Error("failed")),
      on: vi.fn().mockRejectedValue(new Error("failed")),
    });

    await expect(hosted.adapter.setTitle("Motrix")).resolves.toEqual({ status: "failed" });
    await expect(hosted.adapter.openFile("/vol1/file")).resolves.toEqual({ status: "failed" });
    await expect(hosted.adapter.subscribeTheme(vi.fn())).resolves.toMatchObject({ status: "failed" });
  });
});
