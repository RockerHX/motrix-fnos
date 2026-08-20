import { nextTick, reactive } from "vue";
import { describe, expect, it, vi } from "vitest";
import { useTaskToasts } from "./useTaskToasts";

describe("useTaskToasts", () => {
  it("does not emit a message for an ordinary refresh", async () => {
    const taskStore = reactive({
      pendingTaskErrorMessages: [] as string[],
      refreshTasks: vi.fn().mockResolvedValue({ refreshError: "" }),
      refreshRemovedTasks: vi.fn(),
      consumeTaskErrorMessages: vi.fn().mockReturnValue([]),
    });
    const message = { error: vi.fn() };
    const toasts = useTaskToasts({ taskStore: taskStore as never, message });

    await toasts.refreshTasks();

    expect(message.error).not.toHaveBeenCalled();
  });

  it("reports refresh and newly consumed task errors", async () => {
    const taskStore = reactive({
      pendingTaskErrorMessages: [] as string[],
      refreshTasks: vi.fn().mockResolvedValue({ refreshError: "刷新失败" }),
      refreshRemovedTasks: vi.fn(),
      consumeTaskErrorMessages: vi.fn().mockReturnValue(["任务 A 失败"]),
    });
    const message = { error: vi.fn() };
    const toasts = useTaskToasts({ taskStore: taskStore as never, message });

    await toasts.refreshTasks(true);

    expect(taskStore.refreshTasks).toHaveBeenCalledWith({ showError: true });
    expect(message.error).toHaveBeenNthCalledWith(1, "刷新失败");
    expect(message.error).toHaveBeenNthCalledWith(2, "任务 A 失败");
  });

  it("reports removed-task refresh errors without consuming active task errors", async () => {
    const taskStore = reactive({
      pendingTaskErrorMessages: [] as string[],
      refreshTasks: vi.fn(),
      refreshRemovedTasks: vi.fn().mockResolvedValue({ refreshError: "回收站刷新失败" }),
      consumeTaskErrorMessages: vi.fn(),
    });
    const message = { error: vi.fn() };
    const toasts = useTaskToasts({ taskStore: taskStore as never, message });

    await toasts.refreshRemovedTasks();

    expect(taskStore.refreshRemovedTasks).toHaveBeenCalledWith({ showError: false });
    expect(message.error).toHaveBeenCalledWith("回收站刷新失败");
    expect(taskStore.consumeTaskErrorMessages).not.toHaveBeenCalled();
  });

  it("flushes pending task errors when the queue becomes non-empty", async () => {
    const taskStore = reactive({
      pendingTaskErrorMessages: [] as string[],
      refreshTasks: vi.fn(),
      refreshRemovedTasks: vi.fn(),
      consumeTaskErrorMessages: vi.fn().mockReturnValue(["后台任务失败"]),
    });
    const message = { error: vi.fn() };
    useTaskToasts({ taskStore: taskStore as never, message });

    taskStore.pendingTaskErrorMessages.push("后台任务失败");
    await nextTick();

    expect(taskStore.consumeTaskErrorMessages).toHaveBeenCalledOnce();
    expect(message.error).toHaveBeenCalledWith("后台任务失败");
  });

  it("does not replay the same pending error on later ticks", async () => {
    const taskStore = reactive({
      pendingTaskErrorMessages: [] as string[],
      refreshTasks: vi.fn(),
      refreshRemovedTasks: vi.fn(),
      consumeTaskErrorMessages: vi.fn().mockReturnValue(["后台任务失败"]),
    });
    const message = { error: vi.fn() };
    useTaskToasts({ taskStore: taskStore as never, message });

    taskStore.pendingTaskErrorMessages.push("后台任务失败");
    await nextTick();
    await nextTick();

    expect(taskStore.consumeTaskErrorMessages).toHaveBeenCalledOnce();
    expect(message.error).toHaveBeenCalledOnce();
  });
});
