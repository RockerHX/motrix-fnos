import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskProgressBar from "./TaskProgressBar.vue";
import TaskProgressCell from "./TaskProgressCell.vue";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskProgressCell", () => {
  it("passes complete tone for completed tasks", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ status: "complete", completedLength: 50, totalLength: 100 }),
      },
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("complete");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(100);
  });

  it("keeps default tone for unfinished tasks", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ status: "active", completedLength: 50, totalLength: 100 }),
      },
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("default");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(50);
  });
});

function createTask(overrides: Partial<DownloadTask> = {}): DownloadTask {
  return {
    id: 1,
    url: "https://example.com/file.zip",
    fileName: "file.zip",
    saveDir: "/downloads",
    category: "downloading",
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
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}
