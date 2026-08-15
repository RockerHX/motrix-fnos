import { describe, expect, it, vi } from "vitest";
import { FnosHostAdapter } from "./fnos";

function adapter(options: {
  isWeb?: boolean;
  isStandaloneWeb?: boolean;
  ready?: () => Promise<void>;
  pickSharedFile?: () => Promise<unknown>;
  openAppSetting?: () => Promise<unknown>;
} = {}) {
  const app = {
    isWeb: options.isWeb ?? true,
    isStandaloneWeb: options.isStandaloneWeb ?? false,
    ready: options.ready ?? vi.fn().mockResolvedValue(undefined),
    pickSharedFile: options.pickSharedFile ?? vi.fn().mockResolvedValue({ code: 0, msg: "", data: ["/vol1/private"] }),
    openAppSetting: options.openAppSetting ?? vi.fn().mockResolvedValue(undefined),
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
    await expect(standalone.adapter.openAppSettings()).resolves.toEqual({ status: "unsupported" });
    expect(standalone.app.pickSharedFile).not.toHaveBeenCalled();
    expect(standalone.app.openAppSetting).not.toHaveBeenCalled();
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

  it("opens app settings only inside supported hosts", async () => {
    const hosted = adapter();
    await expect(hosted.adapter.openAppSettings()).resolves.toEqual({ status: "opened" });
    expect(hosted.app.openAppSetting).toHaveBeenCalledOnce();
  });
});
