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
    expect(wrapper.text()).toContain("速度 1.0 KB/s");
    expect(wrapper.text()).toContain("剩余时间 1s");
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
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}
