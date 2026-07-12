import { describe, expect, it } from "vitest";
import { ref } from "vue";
import { runTaskToolbarBatch, useTaskToolbar } from "./useTaskToolbar";
import type { MainNavCategory } from "../../../types/navigation";
import type { DownloadTask } from "../../../types/tasks";

describe("useTaskToolbar", () => {
  it("enables create only for task categories while runtime is active", () => {
    const activeCategory = ref<MainNavCategory>("downloading");
    const isRuntimeExiting = ref(false);
    const toolbar = useTaskToolbar({ activeCategory, isRuntimeExiting });

    expect(toolbar.canCreate.value).toBe(true);

    activeCategory.value = "all";
    expect(toolbar.canCreate.value).toBe(true);

    activeCategory.value = "trash";
    expect(toolbar.canCreate.value).toBe(false);

    activeCategory.value = "completed";
    expect(toolbar.canCreate.value).toBe(true);

    isRuntimeExiting.value = true;
    expect(toolbar.canCreate.value).toBe(false);
  });

  it("enables refresh outside extensions while runtime is active", () => {
    const activeCategory = ref<MainNavCategory>("trash");
    const isRuntimeExiting = ref(false);
    const toolbar = useTaskToolbar({ activeCategory, isRuntimeExiting });

    expect(toolbar.canRefresh.value).toBe(true);

    activeCategory.value = "extensions";
    expect(toolbar.canRefresh.value).toBe(false);

    activeCategory.value = "downloading";
    isRuntimeExiting.value = true;
    expect(toolbar.canRefresh.value).toBe(false);
  });

  it("filters visible pause and resume candidates", () => {
    const activeCategory = ref<MainNavCategory>("downloading");
    const isRuntimeExiting = ref(false);
    const isBulkOperating = ref(false);
    const visibleTasks = ref<DownloadTask[]>([
      createTask({ id: 1, status: "active" }),
      createTask({ id: 2, status: "pending" }),
      createTask({ id: 3, status: "paused" }),
      createTask({ id: 4, status: "error" }),
      createTask({ id: 5, status: "paused", confirmationRequired: true }),
      createTask({ id: 6, status: "complete" }),
    ]);
    const toolbar = useTaskToolbar({
      activeCategory,
      isRuntimeExiting,
      visibleTasks,
      isBulkOperating,
      isTaskOperating: (taskId) => taskId === 2,
    });

    expect(toolbar.pauseCandidates.value.map((task) => task.id)).toEqual([1]);
    expect(toolbar.resumeCandidates.value.map((task) => task.id)).toEqual([3, 4]);
    expect(toolbar.canPauseVisible.value).toBe(true);
    expect(toolbar.canResumeVisible.value).toBe(true);
    expect(toolbar.deleteCandidates.value.map((task) => task.id)).toEqual([1, 3, 4, 5, 6]);
    expect(toolbar.canDeleteVisible.value).toBe(true);

    isBulkOperating.value = true;
    expect(toolbar.canPauseVisible.value).toBe(false);
    expect(toolbar.canResumeVisible.value).toBe(false);
    expect(toolbar.canDeleteVisible.value).toBe(false);
  });

  it("runs task batches serially and continues after failures", async () => {
    const visited: number[] = [];
    const result = await runTaskToolbarBatch(
      [createTask({ id: 1 }), createTask({ id: 2 }), createTask({ id: 3 })],
      async (task) => {
        visited.push(task.id);
        if (task.id === 2) {
          throw new Error("failed");
        }
      },
    );

    expect(visited).toEqual([1, 2, 3]);
    expect(result).toEqual({ successCount: 2, failureCount: 1 });
  });

  it("exposes removable records as clear-trash candidates only in Trash", () => {
    const activeCategory = ref<MainNavCategory>("trash");
    const isRuntimeExiting = ref(false);
    const visibleTasks = ref<DownloadTask[]>([createTask({ id: 1, status: "removed" })]);
    const clearTrashTasks = ref<DownloadTask[]>([
      createTask({ id: 1, status: "removed" }),
      createTask({ id: 2, status: "removed" }),
    ]);
    const toolbar = useTaskToolbar({
      activeCategory,
      isRuntimeExiting,
      visibleTasks,
      clearTrashTasks,
    });

    expect(toolbar.clearTrashCandidates.value.map((task) => task.id)).toEqual([1, 2]);
    expect(toolbar.canClearTrash.value).toBe(true);

    activeCategory.value = "downloading";
    expect(toolbar.clearTrashCandidates.value).toEqual([]);
    expect(toolbar.canClearTrash.value).toBe(false);
  });
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.iso",
    fileName: "file.iso",
    saveDir: "/downloads",
    category: "默认",
    gid: "gid-1",
    status: "active",
    totalLength: 100,
    completedLength: 0,
    downloadSpeed: 0,
    errorCode: null,
    errorMessage: null,
    filePath: null,
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}
