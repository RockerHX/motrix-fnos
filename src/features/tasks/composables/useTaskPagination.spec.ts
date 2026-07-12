import { nextTick, ref } from "vue";
import { describe, expect, it } from "vitest";
import { useTaskPagination } from "./useTaskPagination";
import type { MainNavCategory } from "../../../types/navigation";
import type { DownloadTask } from "../../../types/tasks";

describe("useTaskPagination", () => {
  it.each([0, 20])("keeps pagination hidden for %i tasks", (count) => {
    const pagination = createPagination(count);
    expect(pagination.showPagination.value).toBe(false);
    expect(pagination.pagedTasks.value).toHaveLength(count);
  });

  it("paginates 21 tasks with a default page size of 20", () => {
    const pagination = createPagination(21);
    expect(pagination.showPagination.value).toBe(true);
    expect(pagination.pagedTasks.value.map((task) => task.id)).toEqual(Array.from({ length: 20 }, (_, i) => i + 1));

    pagination.page.value = 2;
    expect(pagination.pagedTasks.value.map((task) => task.id)).toEqual([21]);
  });

  it.each([
    [50, 50],
    [100, 1],
  ])("supports a page size of %i for 101 tasks", async (pageSize, expectedSecondPageCount) => {
    const pagination = createPagination(101);
    pagination.pageSize.value = pageSize;
    await nextTick();
    pagination.page.value = 2;
    expect(pagination.pagedTasks.value).toHaveLength(expectedSecondPageCount);
  });

  it("resets on category and page-size changes and clamps after deletions", async () => {
    const tasks = ref(createTasks(101));
    const activeCategory = ref<MainNavCategory>("all");
    const pagination = useTaskPagination({ tasks, activeCategory });

    pagination.page.value = 4;
    activeCategory.value = "completed";
    await nextTick();
    expect(pagination.page.value).toBe(1);

    pagination.page.value = 3;
    pagination.pageSize.value = 50;
    await nextTick();
    expect(pagination.page.value).toBe(1);

    pagination.page.value = 3;
    tasks.value = createTasks(21);
    await nextTick();
    expect(pagination.page.value).toBe(1);
  });
});

function createPagination(count: number) {
  return useTaskPagination({
    tasks: ref(createTasks(count)),
    activeCategory: ref<MainNavCategory>("all"),
  });
}

function createTasks(count: number): DownloadTask[] {
  return Array.from({ length: count }, (_, index) => ({
    id: index + 1,
    url: `https://example.com/${index + 1}.iso`,
    fileName: `${index + 1}.iso`,
    saveDir: "/downloads",
    category: "默认",
    gid: `gid-${index + 1}`,
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
    updatedAt: 1,
  }));
}
