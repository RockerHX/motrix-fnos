import { describe, expect, it, vi } from "vitest";
import { checkAppUpdate } from "../services/aboutService";
import { useUpdateCheck } from "./useUpdateCheck";

vi.mock("../services/aboutService", () => ({ checkAppUpdate: vi.fn() }));

const mockedCheckAppUpdate = vi.mocked(checkAppUpdate);

describe("useUpdateCheck", () => {
  it("stores successful update results and restores loading state", async () => {
    const result = {
      currentVersion: "1.7.0",
      latestVersion: "1.8.0",
      hasUpdate: true,
      status: "available" as const,
      releaseUrl: "https://example.com/releases/1.8.0",
      assets: [],
      checkedAt: 1,
      message: "发现新版本",
    };
    const deferred = createDeferred<typeof result>();
    mockedCheckAppUpdate.mockReturnValueOnce(deferred.promise);
    const message = { error: vi.fn() };
    const update = useUpdateCheck({ message, fallbackMessage: "检查失败" });

    const promise = update.runUpdateCheck();
    expect(update.isCheckingUpdate.value).toBe(true);

    deferred.resolve(result);
    await promise;
    expect(update.updateCheck.value).toEqual(result);
    expect(update.isCheckingUpdate.value).toBe(false);
    expect(message.error).not.toHaveBeenCalled();
  });

  it("reports request errors and uses the fallback for empty errors", async () => {
    const message = { error: vi.fn() };
    const update = useUpdateCheck({ message, fallbackMessage: "检查失败" });
    mockedCheckAppUpdate.mockRejectedValueOnce(new Error("网络不可用")).mockRejectedValueOnce("");

    await update.runUpdateCheck();
    await update.runUpdateCheck();

    expect(message.error).toHaveBeenNthCalledWith(1, "网络不可用");
    expect(message.error).toHaveBeenNthCalledWith(2, "检查失败");
    expect(update.isCheckingUpdate.value).toBe(false);
  });
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
