import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskErrorMessage from "./TaskErrorMessage.vue";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskErrorMessage", () => {
  it("renders formatted error with title for failed task", () => {
    const wrapper = mount(TaskErrorMessage, {
      props: {
        task: createTask({ status: "error", errorCode: "16", errorMessage: "network unreachable" }),
      },
    });

    const error = wrapper.get(".task-card-error");
    expect(wrapper.find('[data-test="task-card-error-slot"]').exists()).toBe(true);
    expect(error.classes()).toContain("task-card-error--single-line");
    expect(error.text()).toBe("错误码 16：network unreachable");
    expect(error.attributes("title")).toBe("错误码 16：network unreachable");
  });

  it("keeps slot container but hides text for non-error task", () => {
    const wrapper = mount(TaskErrorMessage, {
      props: {
        task: createTask(),
        variant: "multi-line",
      },
    });

    expect(wrapper.find('[data-test="task-card-error-slot"]').exists()).toBe(true);
    expect(wrapper.find(".task-card-error").exists()).toBe(false);
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
