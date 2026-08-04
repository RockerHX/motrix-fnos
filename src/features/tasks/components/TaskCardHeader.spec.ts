import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";

vi.mock("./TaskStatusBadge.vue", () => ({
  default: {
    name: "TaskStatusBadgeStub",
    props: ["task"],
    template: '<span data-test="task-status">{{ task.status }}</span>',
  },
}));

import TaskCardHeader from "./TaskCardHeader.vue";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskCardHeader", () => {
  it("renders file title and status badge", () => {
    const wrapper = mount(TaskCardHeader, {
      props: {
        task: createTask(),
      },
    });

    expect(wrapper.classes()).toContain("task-card-header--desktop");
    expect(wrapper.get(".task-card-title").text()).toBe("ubuntu.iso");
    expect(wrapper.get(".task-card-title").attributes("title")).toBe("ubuntu.iso");
    expect(wrapper.get('[data-test="task-status"]').text()).toBe("active");
    expect(wrapper.get(".task-source-icon").attributes("aria-label")).toBe("链接下载");
  });

  it.each([
    ["url", "链接下载", "link"],
    ["torrent", "种子文件", "torrent"],
    ["magnet", "磁力链接", "magnet"],
  ] as const)("identifies %s tasks with a source icon", (sourceType, label, iconName) => {
    const wrapper = mount(TaskCardHeader, {
      props: {
        task: createTask({ sourceType }),
      },
    });

    expect(wrapper.get(".task-source-icon").attributes("aria-label")).toBe(label);
    expect(wrapper.get(".task-source-icon [data-icon-name]").attributes("data-icon-name")).toBe(iconName);
  });

  it("renders actions slot when provided", () => {
    const wrapper = mount(TaskCardHeader, {
      props: {
        task: createTask(),
        variant: "mobile",
      },
      slots: {
        actions: '<button data-test="action">操作</button>',
      },
    });

    expect(wrapper.classes()).toContain("task-card-header--mobile");
    expect(wrapper.find(".task-card-actions").exists()).toBe(true);
    expect(wrapper.get('[data-test="action"]').text()).toBe("操作");
  });

  it("shows a proxy status icon without exposing the proxy address", () => {
    const wrapper = mount(TaskCardHeader, {
      props: {
        task: createTask({ useProxy: true }),
        variant: "mobile",
      },
    });

    const indicator = wrapper.get(".task-proxy-indicator");
    expect(indicator.attributes("title")).toBe("此任务使用下载代理");
    expect(indicator.attributes("aria-label")).toBe("此任务使用下载代理");
    expect(indicator.get('[data-icon-name="proxy"]').attributes("data-icon-name")).toBe("proxy");
    expect(wrapper.text()).not.toContain("http://");
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
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}
