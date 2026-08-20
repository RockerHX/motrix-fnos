import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createEventSourceMock } from "../test/mount";

const mockTaskStore = {
  isRuntimeExiting: false,
  applyTaskSnapshot: vi.fn(),
  markRuntimeExiting: vi.fn(),
  refreshTasks: vi.fn(),
  cancelRefreshRequests: vi.fn(),
};

vi.mock("../features/tasks/stores/taskStore", () => ({
  useTaskStore: () => mockTaskStore,
}));

import { disposeRuntimeEvents, initializeRuntimeEvents } from "./runtimeEvents";

describe("runtimeEvents", () => {
  beforeEach(() => {
    vi.useRealTimers();
    mockTaskStore.isRuntimeExiting = false;
    mockTaskStore.applyTaskSnapshot.mockReset();
    mockTaskStore.markRuntimeExiting.mockReset();
    mockTaskStore.refreshTasks.mockReset();
    mockTaskStore.cancelRefreshRequests.mockReset();
  });

  afterEach(() => {
    disposeRuntimeEvents();
    vi.unstubAllGlobals();
  });

  it("initializes a single EventSource instance", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);

    const first = initializeRuntimeEvents();
    const second = initializeRuntimeEvents();

    expect(first).toBe(second);
    expect(EventSourceMock.calls).toHaveLength(1);
    expect(instances).toHaveLength(1);
    expect(instances[0]?.url).toBe("/api/events");
  });

  it("applies task snapshots when runtime is active", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);

    initializeRuntimeEvents();
    instances[0]?.emit(
      "tasks.snapshot",
      new MessageEvent("tasks.snapshot", {
        data: JSON.stringify({
          revision: 1,
          tasks: [{ id: 1 }],
        }),
      }),
    );

    expect(mockTaskStore.applyTaskSnapshot).toHaveBeenCalledWith({
      revision: 1,
      tasks: [{ id: 1 }],
    });
  });

  it("ignores task snapshots after runtime is exiting", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    mockTaskStore.isRuntimeExiting = true;

    initializeRuntimeEvents();
    instances[0]?.emit(
      "tasks.snapshot",
      new MessageEvent("tasks.snapshot", {
        data: JSON.stringify({
          revision: 1,
          tasks: [{ id: 2 }],
        }),
      }),
    );

    expect(mockTaskStore.applyTaskSnapshot).not.toHaveBeenCalled();
  });

  it("marks runtime exiting when receiving runtime.exiting event", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);

    initializeRuntimeEvents();
    instances[0]?.emit(
      "runtime.exiting",
      new MessageEvent("runtime.exiting", {
        data: JSON.stringify({
          reason: "shutdown",
          timestamp: 123,
        }),
      }),
    );

    expect(mockTaskStore.markRuntimeExiting).toHaveBeenCalledWith({
      reason: "shutdown",
      timestamp: 123,
    });
  });

  it("ignores invalid payloads and non MessageEvent instances", () => {
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);

    initializeRuntimeEvents();
    instances[0]?.emit(
      "tasks.snapshot",
      new MessageEvent("tasks.snapshot", {
        data: "{invalid-json",
      }),
    );
    instances[0]?.emit("runtime.exiting", new Event("runtime.exiting"));

    expect(mockTaskStore.applyTaskSnapshot).not.toHaveBeenCalled();
    expect(mockTaskStore.markRuntimeExiting).not.toHaveBeenCalled();
  });

  it("dispose closes current source and allows later reinitialization", () => {
    const firstMock = createEventSourceMock();
    vi.stubGlobal("EventSource", firstMock.EventSourceMock);

    initializeRuntimeEvents();
    const firstInstance = firstMock.instances[0];

    disposeRuntimeEvents();
    expect(firstInstance?.close).toHaveBeenCalledTimes(1);
    expect(mockTaskStore.cancelRefreshRequests).toHaveBeenCalledOnce();

    const secondMock = createEventSourceMock();
    vi.stubGlobal("EventSource", secondMock.EventSourceMock);

    initializeRuntimeEvents();

    expect(secondMock.EventSourceMock.calls).toHaveLength(1);
    expect(secondMock.instances[0]).not.toBe(firstInstance);
  });

  it("closes the source and stops reconnecting when auth is invalid", async () => {
    vi.useFakeTimers();
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    const status = { setupRequired: false, enabled: true, authenticated: false, csrfToken: null };
    const checkAuth = vi.fn().mockResolvedValue(status);
    const onUnauthorized = vi.fn();

    initializeRuntimeEvents({ checkAuth, onUnauthorized });
    instances[0]?.emit("error", new Event("error"));
    await vi.runAllTicks();

    expect(instances[0]?.close).toHaveBeenCalledOnce();
    expect(checkAuth).toHaveBeenCalledOnce();
    expect(onUnauthorized).toHaveBeenCalledWith(status);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(instances).toHaveLength(1);
  });

  it("reconnects with bounded backoff while auth remains valid", async () => {
    vi.useFakeTimers();
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    const checkAuth = vi.fn().mockResolvedValue({
      setupRequired: false,
      enabled: true,
      authenticated: true,
      csrfToken: "csrf",
    });
    initializeRuntimeEvents({ checkAuth, onUnauthorized: vi.fn() });

    for (const delay of [1, 2, 4, 8, 16, 30, 30]) {
      instances[instances.length - 1]?.emit("error", new Event("error"));
      await vi.runAllTicks();
      await vi.advanceTimersByTimeAsync(delay * 1_000 - 1);
      expect(instances).toHaveLength(checkAuth.mock.calls.length);
      await vi.advanceTimersByTimeAsync(1);
      expect(instances).toHaveLength(checkAuth.mock.calls.length + 1);
    }
  });

  it("resets backoff after a successful open event", async () => {
    vi.useFakeTimers();
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    const checkAuth = vi.fn().mockResolvedValue({
      setupRequired: false,
      enabled: false,
      authenticated: false,
      csrfToken: "anonymous-csrf",
    });
    initializeRuntimeEvents({ checkAuth, onUnauthorized: vi.fn() });

    instances[0]?.emit("error", new Event("error"));
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(1_000);
    instances[1]?.emit("error", new Event("error"));
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(2_000);
    instances[2]?.emit("open", new Event("open"));
    instances[2]?.emit("error", new Event("error"));
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(999);
    expect(instances).toHaveLength(3);
    await vi.advanceTimersByTimeAsync(1);
    expect(instances).toHaveLength(4);
  });

  it("refreshes tasks after a reconnected source opens", async () => {
    vi.useFakeTimers();
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    initializeRuntimeEvents({
      checkAuth: vi.fn().mockResolvedValue({
        setupRequired: false,
        enabled: true,
        authenticated: true,
        csrfToken: "csrf",
      }),
      onUnauthorized: vi.fn(),
    });

    instances[0]?.emit("open", new Event("open"));
    expect(mockTaskStore.refreshTasks).not.toHaveBeenCalled();

    instances[0]?.emit("error", new Event("error"));
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(1_000);
    instances[1]?.emit("open", new Event("open"));

    expect(mockTaskStore.refreshTasks).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(mockTaskStore.refreshTasks).toHaveBeenCalledTimes(1);
  });

  it("cancels retries and ignores a late auth probe after disposal", async () => {
    vi.useFakeTimers();
    const { EventSourceMock, instances } = createEventSourceMock();
    vi.stubGlobal("EventSource", EventSourceMock);
    let resolveStatus: ((status: { setupRequired: boolean; enabled: boolean; authenticated: boolean; csrfToken: null }) => void) | undefined;
    const checkAuth = vi.fn(
      () =>
        new Promise<{ setupRequired: boolean; enabled: boolean; authenticated: boolean; csrfToken: null }>((resolve) => {
          resolveStatus = resolve;
        }),
    );
    const onUnauthorized = vi.fn();
    initializeRuntimeEvents({ checkAuth, onUnauthorized });

    instances[0]?.emit("error", new Event("error"));
    disposeRuntimeEvents();
    resolveStatus?.({ setupRequired: false, enabled: true, authenticated: false, csrfToken: null });
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(60_000);

    expect(onUnauthorized).not.toHaveBeenCalled();
    expect(instances).toHaveLength(1);
  });
});
