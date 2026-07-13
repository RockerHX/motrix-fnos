import { describe, expect, it, vi } from "vitest";
import { getAria2ProcessStatus, pingAria2Rpc } from "../../../services/aria2";
import { useAria2Status } from "./useAria2Status";

vi.mock("../../../services/aria2", () => ({
  getAria2ProcessStatus: vi.fn(),
  pingAria2Rpc: vi.fn(),
}));

const mockedGetAria2ProcessStatus = vi.mocked(getAria2ProcessStatus);
const mockedPingAria2Rpc = vi.mocked(pingAria2Rpc);

describe("useAria2Status", () => {
  it("refreshes process and RPC state in parallel", async () => {
    const processDeferred = createDeferred<{ running: boolean; message: string }>();
    const rpcDeferred = createDeferred<{ connected: boolean; message: string }>();
    mockedGetAria2ProcessStatus.mockReturnValueOnce(processDeferred.promise);
    mockedPingAria2Rpc.mockReturnValueOnce(rpcDeferred.promise);
    const status = useAria2Status();

    const promise = status.refreshAria2Status();
    expect(mockedGetAria2ProcessStatus).toHaveBeenCalledOnce();
    expect(mockedPingAria2Rpc).toHaveBeenCalledOnce();

    processDeferred.resolve({ running: true, message: "running" });
    rpcDeferred.resolve({ connected: true, message: "ready" });
    await promise;
    expect(status.aria2Process.value).toEqual({ running: true, message: "running" });
    expect(status.aria2Rpc.value).toEqual({ connected: true, message: "ready" });
  });

  it("updates both states from a runtime snapshot", () => {
    const status = useAria2Status();

    status.updateAria2Status({
      process: { running: false, message: "stopped" },
      rpc: { connected: false, message: "offline" },
    });

    expect(status.aria2Process.value?.running).toBe(false);
    expect(status.aria2Rpc.value?.connected).toBe(false);
  });

  it("does not partially replace state when one refresh request fails", async () => {
    const status = useAria2Status();
    status.updateAria2Status({
      process: { running: false, message: "old process" },
      rpc: { connected: false, message: "old rpc" },
    });
    mockedGetAria2ProcessStatus.mockResolvedValueOnce({ running: true, message: "new process" });
    mockedPingAria2Rpc.mockRejectedValueOnce(new Error("rpc failed"));

    await expect(status.refreshAria2Status()).rejects.toThrow("rpc failed");

    expect(status.aria2Process.value?.message).toBe("old process");
    expect(status.aria2Rpc.value?.message).toBe("old rpc");
  });
});

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
