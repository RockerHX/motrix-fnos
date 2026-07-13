import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { clearDebugLogs, listDebugLogs } from "../services/debugLogService";
import type { DebugLogEntry } from "../types";
import { useDebugLogStore } from "./debugLogStore";

vi.mock("../services/debugLogService", () => ({ clearDebugLogs: vi.fn(), listDebugLogs: vi.fn() }));

const mockedClearDebugLogs = vi.mocked(clearDebugLogs);
const mockedListDebugLogs = vi.mocked(listDebugLogs);

describe("debugLogStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads logs and restores loading state", async () => {
    const store = useDebugLogStore();
    const deferred = createDeferred<DebugLogEntry[]>();
    const entries = [entry(1)];
    mockedListDebugLogs.mockReturnValueOnce(deferred.promise);

    const promise = store.refreshLogs();
    expect(store.isLoading).toBe(true);

    deferred.resolve(entries);
    await promise;
    expect(store.logs).toEqual(entries);
    expect(store.errorMessage).toBe("");
    expect(store.isLoading).toBe(false);
  });

  it("records refresh errors and keeps previous logs", async () => {
    const store = useDebugLogStore();
    store.logs = [entry(1)];
    mockedListDebugLogs.mockRejectedValueOnce(new Error("日志读取失败"));

    await expect(store.refreshLogs()).rejects.toThrow("日志读取失败");

    expect(store.logs).toHaveLength(1);
    expect(store.errorMessage).toBe("日志读取失败");
    expect(store.isLoading).toBe(false);
  });

  it("clears logs after the backend succeeds", async () => {
    const store = useDebugLogStore();
    store.logs = [entry(1), entry(2)];
    mockedClearDebugLogs.mockResolvedValueOnce(undefined);

    await store.clearLogs();

    expect(store.logs).toEqual([]);
    expect(store.errorMessage).toBe("");
    expect(store.isClearing).toBe(false);
  });

  it("keeps logs and records the error when clearing fails", async () => {
    const store = useDebugLogStore();
    store.logs = [entry(1)];
    mockedClearDebugLogs.mockRejectedValueOnce(new Error("日志清理失败"));

    await expect(store.clearLogs()).rejects.toThrow("日志清理失败");

    expect(store.logs).toHaveLength(1);
    expect(store.errorMessage).toBe("日志清理失败");
    expect(store.isClearing).toBe(false);
  });
});

function entry(id: number): DebugLogEntry {
  return {
    id,
    timestampMs: id,
    lastTimestampMs: id,
    level: "info",
    category: "app",
    module: "test",
    message: `message-${id}`,
    repeatCount: 1,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
