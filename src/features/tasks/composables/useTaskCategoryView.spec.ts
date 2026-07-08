import { ref } from "vue";
import { describe, expect, it } from "vitest";
import type { DownloadTask } from "../../../types/tasks";
import { useTaskCategoryView } from "./useTaskCategoryView";

describe("useTaskCategoryView", () => {
  it("filters tasks by category", () => {
    const tasks = ref<DownloadTask[]>([
      createTask({ id: 1, status: "pending" }),
      createTask({ id: 2, status: "active" }),
      createTask({ id: 3, status: "complete" }),
      createTask({ id: 4, status: "paused" }),
      createTask({ id: 5, status: "error" }),
    ]);
    const removedTasks = ref<DownloadTask[]>([createTask({ id: 6, status: "removed" })]);
    const view = useTaskCategoryView({
      tasks,
      removedTasks,
      isRuntimeExiting: ref(false),
      isMobileLayout: ref(false),
    });

    expect(view.visibleTasks.value.map((task) => task.id)).toEqual([1, 2]);

    view.activeCategory.value = "completed";
    expect(view.visibleTasks.value.map((task) => task.id)).toEqual([3]);

    view.activeCategory.value = "stopped";
    expect(view.visibleTasks.value.map((task) => task.id)).toEqual([4, 5]);

    view.activeCategory.value = "trash";
    expect(view.visibleTasks.value.map((task) => task.id)).toEqual([6]);

    view.activeCategory.value = "extensions";
    expect(view.visibleTasks.value).toEqual([]);
  });

  it("switches empty state and content view key with category and list presence", () => {
    const tasks = ref<DownloadTask[]>([]);
    const removedTasks = ref<DownloadTask[]>([]);
    const view = useTaskCategoryView({
      tasks,
      removedTasks,
      isRuntimeExiting: ref(false),
      isMobileLayout: ref(false),
    });

    expect(view.emptyState.value.titleKey).toBe("empty.downloading.title");
    expect(view.contentViewKey.value).toBe("downloading-empty");

    tasks.value = [createTask({ id: 10, status: "active" })];
    expect(view.contentViewKey.value).toBe("downloading-list");

    view.activeCategory.value = "extensions";
    expect(view.emptyState.value.titleKey).toBe("empty.extensions.title");
    expect(view.contentViewKey.value).toBe("extensions-extensions");
  });

  it("controls floating add visibility for mobile empty state, runtime exiting and non task pages", () => {
    const tasks = ref<DownloadTask[]>([]);
    const removedTasks = ref<DownloadTask[]>([]);
    const isRuntimeExiting = ref(false);
    const isMobileLayout = ref(true);
    const view = useTaskCategoryView({
      tasks,
      removedTasks,
      isRuntimeExiting,
      isMobileLayout,
    });

    expect(view.showFloatingAdd.value).toBe(false);

    tasks.value = [createTask({ id: 20, status: "active" })];
    expect(view.showFloatingAdd.value).toBe(true);

    view.activeCategory.value = "completed";
    tasks.value = [];
    expect(view.showFloatingAdd.value).toBe(true);

    view.activeCategory.value = "trash";
    expect(view.showFloatingAdd.value).toBe(false);

    isRuntimeExiting.value = true;
    view.activeCategory.value = "downloading";
    tasks.value = [createTask({ id: 21, status: "active" })];
    expect(view.showFloatingAdd.value).toBe(false);
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
    status: "pending",
    totalLength: 1024,
    completedLength: 0,
    downloadSpeed: 128,
    errorCode: null,
    errorMessage: null,
    filePath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}
