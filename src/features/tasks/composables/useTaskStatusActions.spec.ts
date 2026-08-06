import { describe, expect, it, vi } from "vitest";
import { useTaskStatusActions } from "./useTaskStatusActions";
import type { DownloadTask } from "../../../types/tasks";

describe("useTaskStatusActions", () => {
  it.each(["active", "pending"] as const)("pauses a %s task on double click", async (status) => {
    const { actions, taskStore, message } = setup();
    const task = createTask({ status });

    await actions.handleTaskDoubleClick(task, mouseEvent(document.createElement("div")));

    expect(taskStore.pauseTask).toHaveBeenCalledWith(task.id);
    expect(taskStore.resumeTask).not.toHaveBeenCalled();
    expect(message.success).toHaveBeenCalledWith("task.actions.paused");
  });

  it.each(["paused", "error"] as const)("resumes a %s task on double click", async (status) => {
    const { actions, taskStore, message } = setup();
    const task = createTask({ status });

    await actions.handleTaskDoubleClick(task, mouseEvent(document.createElement("div")));

    expect(taskStore.resumeTask).toHaveBeenCalledWith(task.id);
    expect(taskStore.pauseTask).not.toHaveBeenCalled();
    expect(message.success).toHaveBeenCalledWith("task.actions.resumed");
  });

  it("ignores unsupported, confirming and already operating tasks", async () => {
    const { actions, taskStore } = setup();

    await actions.handleTaskDoubleClick(createTask({ status: "complete" }), mouseEvent(document.body));
    await actions.handleTaskDoubleClick(
      createTask({ status: "paused", confirmationRequired: true }),
      mouseEvent(document.body),
    );
    taskStore.isTaskOperating.mockReturnValueOnce(true);
    await actions.handleTaskDoubleClick(createTask(), mouseEvent(document.body));

    expect(taskStore.pauseTask).not.toHaveBeenCalled();
    expect(taskStore.resumeTask).not.toHaveBeenCalled();
  });

  it("ignores double clicks originating from an interactive control", async () => {
    const { actions, taskStore } = setup();
    const button = document.createElement("button");
    const icon = document.createElement("span");
    button.append(icon);

    await actions.handleTaskDoubleClick(createTask(), mouseEvent(icon));

    expect(taskStore.pauseTask).not.toHaveBeenCalled();
  });

  it("reports runtime and operation failures consistently", async () => {
    const { actions, taskStore, message } = setup();
    taskStore.isRuntimeExiting = true;

    await actions.handleTaskDoubleClick(createTask(), mouseEvent(document.body));
    expect(message.warning).toHaveBeenCalledWith("task.runtimeExiting");

    taskStore.isRuntimeExiting = false;
    taskStore.pauseTask.mockRejectedValueOnce(new Error("request failed"));
    await actions.handleTaskDoubleClick(createTask(), mouseEvent(document.body));
    expect(message.error).toHaveBeenCalledWith("request failed");
  });
});

function setup() {
  const taskStore = {
    isRuntimeExiting: false,
    isTaskOperating: vi.fn().mockReturnValue(false),
    pauseTask: vi.fn().mockResolvedValue(undefined),
    resumeTask: vi.fn().mockResolvedValue(undefined),
  };
  const message = { success: vi.fn(), warning: vi.fn(), error: vi.fn() };
  const actions = useTaskStatusActions({
    taskStore: taskStore as never,
    message,
    t: (key) => key,
  });
  return { actions, taskStore, message };
}

function mouseEvent(target: Element): MouseEvent {
  return { target } as unknown as MouseEvent;
}

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/ubuntu.iso",
    fileName: "ubuntu.iso",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "active",
    totalLength: 2000,
    completedLength: 1000,
    downloadSpeed: 1024,
    errorCode: null,
    errorMessage: null,
    filePath: "/downloads/ubuntu.iso",
    useProxy: false,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}
