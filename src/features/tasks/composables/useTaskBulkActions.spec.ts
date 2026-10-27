import { describe, expect, it, vi } from "vitest";
import { computed, ref } from "vue";
import { useTaskBulkActions } from "./useTaskBulkActions";
import type { DownloadTask } from "../../../types/tasks";

describe("useTaskBulkActions", () => {
  it("runs visible pause tasks and reports partial failures", async () => {
    const taskStore = { pauseTask: vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error("failed")) };
    const message = messageApi();
    const actions = useTaskBulkActions({
      taskStore: taskStore as never,
      toolbar: toolbar([task(1), task(2)]) as never,
      message,
      t: (key, params) => `${key}:${params?.count ?? ""}`,
    });

    await actions.pauseVisibleTasks();

    expect(taskStore.pauseTask).toHaveBeenCalledTimes(2);
    expect(message.success).toHaveBeenCalledWith("task.bulk.pauseSuccess:1");
    expect(message.error).toHaveBeenCalledWith("task.bulk.partialFailed:1");
    expect(actions.isBulkOperating.value).toBe(false);
  });

  it("opens and closes the delete confirmation around deletion", async () => {
    const taskStore = { deleteTask: vi.fn().mockResolvedValue(undefined) };
    const actions = useTaskBulkActions({
      taskStore: taskStore as never,
      toolbar: toolbar([task(1)]) as never,
      message: messageApi(),
      t: (key) => key,
    });

    actions.requestDeleteVisibleTasks();
    expect(actions.showBulkDeleteConfirm.value).toBe(true);
    await actions.confirmDeleteVisibleTasks();
    expect(taskStore.deleteTask).toHaveBeenCalledWith(1);
    expect(actions.showBulkDeleteConfirm.value).toBe(false);
  });

  it("passes the file choice to each Trash task and reports partial failures", async () => {
    const taskStore = { permanentlyDeleteTask: vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error("failed")) };
    const message = messageApi();
    const actions = useTaskBulkActions({ taskStore: taskStore as never, toolbar: toolbar([task(1), task(2)]) as never,
      message, t: (key, params) => `${key}:${params?.count ?? ""}` });
    actions.requestClearTrash();
    await actions.confirmDeleteVisibleTasks(true);
    expect(taskStore.permanentlyDeleteTask.mock.calls).toEqual([[1, true], [2, true]]);
    expect(message.success).toHaveBeenCalledWith("task.bulk.clearTrashSuccess:1");
    expect(message.error).toHaveBeenCalledWith("task.bulk.partialFailed:1");
    expect(actions.showBulkDeleteConfirm.value).toBe(false);
  });

  it("warns instead of opening an empty delete confirmation", () => {
    const message = messageApi();
    const actions = useTaskBulkActions({
      taskStore: {} as never,
      toolbar: toolbar([]) as never,
      message,
      t: (key) => key,
    });

    actions.requestDeleteVisibleTasks();
    expect(message.warning).toHaveBeenCalledWith("task.bulk.noDeletable");
    expect(actions.showBulkDeleteConfirm.value).toBe(false);
  });

  it("offers file deletion only when Trash contains eligible content and applies it per task", async () => {
    const parsing = { ...task(1), url: "magnet:?xt=urn:btih:test" };
    const awaiting = { ...parsing, id: 2, confirmationRequired: true,
      files: [{ index: 1, path: "/downloads/preview.bin", name: "preview.bin", length: 100, completedLength: 0, selected: true }] };
    const candidates = toolbar([parsing, awaiting]);
    const taskStore = { permanentlyDeleteTask: vi.fn().mockResolvedValue(undefined) };
    const actions = useTaskBulkActions({ taskStore: taskStore as never, toolbar: candidates as never,
      message: messageApi(), t: (key) => key });
    actions.requestClearTrash();
    expect(actions.canBulkDeleteFiles.value).toBe(false);
    const confirmed = { ...parsing, id: 3, ownedTaskDir: "/downloads/owned" };
    candidates.clearTrashCandidates.value = [parsing, awaiting, confirmed, { ...task(4), filesDeleted: true }];
    expect(actions.canBulkDeleteFiles.value).toBe(true);

    await actions.confirmDeleteVisibleTasks(true);

    expect(taskStore.permanentlyDeleteTask.mock.calls).toEqual([[1, false], [2, false], [3, true], [4, false]]);
  });
});

function toolbar(tasks: DownloadTask[]) {
  return {
    pauseCandidates: ref(tasks),
    resumeCandidates: ref(tasks),
    deleteCandidates: ref(tasks),
    clearTrashCandidates: ref(tasks),
    canDeleteVisible: computed(() => tasks.length > 0),
    canClearTrash: computed(() => tasks.length > 0),
  };
}

function messageApi() {
  return { success: vi.fn(), warning: vi.fn(), error: vi.fn() };
}

function task(id: number): DownloadTask {
  return {
    id, url: "https://example.com/file", fileName: "file", saveDir: "/downloads", category: "默认",
    gid: `gid-${id}`, status: "active", totalLength: 1, completedLength: 0, downloadSpeed: 0,
    errorCode: null, errorMessage: null, filePath: null, useProxy: false,
    confirmationRequired: false, files: [], createdAt: 1, updatedAt: 1,
  };
}
