import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createEventSourceMock } from "../test/mount";

const mockTaskStore = {
  isRuntimeExiting: false,
  applyTaskSnapshot: vi.fn(),
  markRuntimeExiting: vi.fn(),
};

vi.mock("../features/tasks/stores/taskStore", () => ({
  useTaskStore: () => mockTaskStore,
}));

import { disposeRuntimeEvents, initializeRuntimeEvents } from "./runtimeEvents";

describe("runtimeEvents", () => {
  beforeEach(() => {
    mockTaskStore.isRuntimeExiting = false;
    mockTaskStore.applyTaskSnapshot.mockReset();
    mockTaskStore.markRuntimeExiting.mockReset();
  });

  afterEach(() => {
    disposeRuntimeEvents();
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
          tasks: [{ id: 1 }],
        }),
      }),
    );

    expect(mockTaskStore.applyTaskSnapshot).toHaveBeenCalledWith({
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

    const secondMock = createEventSourceMock();
    vi.stubGlobal("EventSource", secondMock.EventSourceMock);

    initializeRuntimeEvents();

    expect(secondMock.EventSourceMock.calls).toHaveLength(1);
    expect(secondMock.instances[0]).not.toBe(firstInstance);
  });
});
