import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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
    vi.stubGlobal("fetch", vi.fn());
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

  it("opens one bearer fetch stream and parses runtime events", async () => {
    const stream = controllableStream();
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(stream.response);

    initializeRuntimeEvents({
      checkAuth: vi.fn(),
      onUnauthorized: vi.fn(),
      getAccessToken: () => "jwt-value",
    });
    await flush();
    expect(fetchMock).toHaveBeenCalledWith("/api/events", expect.objectContaining({
      method: "GET",
      credentials: "omit",
      headers: { Authorization: "Bearer jwt-value" },
    }));

    stream.push("event: tasks.snapshot\ndata: {\"revision\":1,\ndata: \"tasks\":[{\"id\":1}]}\n\n");
    await flush();
    expect(mockTaskStore.applyTaskSnapshot).toHaveBeenCalledWith({ revision: 1, tasks: [{ id: 1 }] });

    stream.push("event: runtime.exiting\ndata: {\"reason\":\"shutdown\",\"timestamp\":3}\n\n");
    await flush();
    expect(mockTaskStore.markRuntimeExiting).toHaveBeenCalledWith({ reason: "shutdown", timestamp: 3 });
  });

  it("does not send a token for anonymous streams", async () => {
    const stream = controllableStream();
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(stream.response);
    initializeRuntimeEvents({ checkAuth: vi.fn(), onUnauthorized: vi.fn(), getAccessToken: () => null });
    await flush();
    expect(fetchMock.mock.calls[0]?.[1]).toEqual(expect.objectContaining({ headers: {} }));
  });

  it("rechecks auth and aborts an unauthorized stream", async () => {
    vi.useFakeTimers();
    const stream = controllableStream();
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(stream.response);
    const status = { setupRequired: false, enabled: true, authenticated: false };
    const checkAuth = vi.fn().mockResolvedValue(status);
    const onUnauthorized = vi.fn();
    initializeRuntimeEvents({ checkAuth, onUnauthorized, getAccessToken: () => "jwt" });
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(15_000);
    expect(checkAuth).toHaveBeenCalledOnce();
    expect(onUnauthorized).toHaveBeenCalledWith(status);
    expect(stream.cancelled()).toBe(true);
  });

  it("reconnects with bounded backoff after a closed stream", async () => {
    vi.useFakeTimers();
    const first = controllableStream();
    const second = controllableStream();
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockResolvedValueOnce(first.response).mockResolvedValueOnce(second.response);
    const checkAuth = vi.fn().mockResolvedValue({ setupRequired: false, enabled: true, authenticated: true });
    initializeRuntimeEvents({ checkAuth, onUnauthorized: vi.fn(), getAccessToken: () => "jwt" });
    await vi.runAllTicks();
    first.close();
    await vi.runAllTicks();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1_000);
    await vi.runAllTicks();
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("cancels pending streams and retries on dispose", async () => {
    const stream = controllableStream();
    vi.mocked(fetch).mockResolvedValueOnce(stream.response);
    initializeRuntimeEvents();
    await flush();
    disposeRuntimeEvents();
    expect(stream.cancelled()).toBe(true);
    expect(mockTaskStore.cancelRefreshRequests).toHaveBeenCalledOnce();
  });
});

function controllableStream() {
  let streamController: ReadableStreamDefaultController<Uint8Array> | undefined;
  let cancelled = false;
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      streamController = controller;
    },
    cancel() {
      cancelled = true;
    },
  });
  return {
    response: new Response(stream, { status: 200, headers: { "content-type": "text/event-stream" } }),
    push(value: string) {
      streamController?.enqueue(new TextEncoder().encode(value));
    },
    close() {
      streamController?.close();
    },
    cancelled: () => cancelled,
  };
}

function flush() {
  return new Promise<void>((resolve) => queueMicrotask(resolve));
}
