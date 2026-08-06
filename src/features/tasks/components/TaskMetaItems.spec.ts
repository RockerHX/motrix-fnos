import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskMetaItems from "./TaskMetaItems.vue";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskMetaItems", () => {
  it("renders inline task metrics", () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
      },
    });

    expect(wrapper.classes()).toContain("task-card-meta--inline");
    expect(wrapper.text()).toContain("1000 B / 2.0 KB");
    expect(wrapper.get('[data-test="task-size-metric"]').text()).toBe("1000 B / 2.0 KB");
    const dynamicMetrics = wrapper.get('[data-test="task-dynamic-metrics"]');
    expect(dynamicMetrics.findAll(":scope > .task-card-metric")).toHaveLength(2);
    expect(wrapper.get('[data-test="task-eta-metric"]').attributes("aria-label")).toBe("剩余时间 1s");
    expect(wrapper.get('[data-test="task-speed-metric"]').attributes("aria-label")).toBe("速度 1.0 KB/s");
  });

  it("renders grid task metrics", () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
        variant: "grid",
      },
    });

    expect(wrapper.classes()).toContain("task-card-meta--grid");
    expect(wrapper.findAll("dt").map((item) => item.text())).toEqual(["已下载 / 总大小", "速度", "剩余时间"]);
    expect(wrapper.findAll("dd").map((item) => item.text())).toEqual(["1000 B / 2.0 KB", "1.0 KB/s", "1s"]);
  });
});

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
