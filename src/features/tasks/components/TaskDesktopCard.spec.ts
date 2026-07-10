import { describe, expect, it, vi } from "vitest";

vi.mock("./TaskActionsContainer.vue", () => ({
  default: {
    name: "TaskActionsContainerStub",
    props: ["task", "compact"],
    template: '<div data-test="task-actions">actions-{{ task.id }}-{{ compact }}</div>',
  },
}));

vi.mock("./TaskProgressCell.vue", () => ({
  default: {
    name: "TaskProgressCellStub",
    props: ["task", "showLabel", "variant"],
    template: '<div data-test="task-progress">{{ task.id }}-{{ showLabel }}-{{ variant }}</div>',
  },
}));

vi.mock("./TaskStatusBadge.vue", () => ({
  default: {
    name: "TaskStatusBadgeStub",
    props: ["task"],
    template: '<span data-test="task-status">{{ task.status }}</span>',
  },
}));

import TaskDesktopCard from "./TaskDesktopCard.vue";
import { mountWithPinia } from "../../../test/mount";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskDesktopCard", () => {
  it("renders task title, metrics, progress, status and actions", () => {
    const { wrapper } = mountWithPinia(TaskDesktopCard, {
      props: {
        task: createTask(),
      },
    });

    expect(wrapper.find('[data-test="task-desktop-card"]').exists()).toBe(true);
    expect(wrapper.text()).toContain("ubuntu.iso");
    expect(wrapper.text()).toContain("1000 B / 2.0 KB");
    expect(wrapper.text()).toContain("1.0 KB/s");
    expect(wrapper.text()).toContain("1s");
    expect(wrapper.find('[data-test="task-progress"]').text()).toContain("false-card");
    expect(wrapper.find('[data-test="task-status"]').text()).toBe("active");
    expect(wrapper.find('[data-test="task-actions"]').text()).toContain("actions-1");
  });

  it("renders error message for failed task", () => {
    const { wrapper } = mountWithPinia(TaskDesktopCard, {
      props: {
        task: createTask({
          status: "error",
          errorCode: "16",
          errorMessage: "network unreachable",
        }),
      },
    });

    expect(wrapper.text()).toContain("错误码 16：network unreachable");
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
