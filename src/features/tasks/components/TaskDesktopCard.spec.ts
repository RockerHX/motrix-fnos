import { describe, expect, it, vi } from "vitest";

vi.mock("./TaskActionsContainer.vue", () => ({
  default: {
    name: "TaskActionsContainerStub",
    props: ["task", "mode"],
    template: '<div data-test="task-actions">actions-{{ task.id }}-{{ mode }}</div>',
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
import { createTask } from "../../../test/tasks";

describe("TaskDesktopCard", () => {
  it("renders task title, metrics, progress, status and actions", () => {
    const { wrapper } = mountWithPinia(TaskDesktopCard, {
      props: {
        task: createTask(),
      },
    });

    expect(wrapper.find('[data-test="task-desktop-card"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="task-card-status-rail"]').exists()).toBe(false);
    expect(wrapper.find(".task-card-grip").exists()).toBe(false);
    expect(wrapper.text()).toContain("ubuntu.iso");
    expect(wrapper.get(".task-card-title").attributes("title")).toBe("ubuntu.iso");
    expect(wrapper.text()).toContain("1000 B / 2.0 KB");
    expect(wrapper.text()).toContain("1.0 KB/s");
    expect(wrapper.text()).toContain("1s");
    expect(wrapper.find(".task-card-body").exists()).toBe(true);
    expect(wrapper.find(".task-card-meta").text()).toContain("速度");
    expect(wrapper.find('[data-test="task-progress"]').text()).toBe("1-true-card");
    expect(wrapper.find('[data-test="task-status"]').text()).toBe("active");
    expect(wrapper.find('[data-test="task-actions"]').text()).toContain("actions-1-desktop");
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

    const error = wrapper.get(".task-card-error");
    expect(error.text()).toBe("错误码 16：network unreachable");
    expect(error.attributes("title")).toBe("错误码 16：network unreachable");
  });

  it("keeps stable error slot for non-error task without rendering error text", () => {
    const { wrapper } = mountWithPinia(TaskDesktopCard, {
      props: {
        task: createTask({
          fileName: "very-long-download-file-name-that-should-stay-readable-in-title.iso",
        }),
      },
    });

    expect(wrapper.get(".task-card-title").attributes("title")).toBe(
      "very-long-download-file-name-that-should-stay-readable-in-title.iso",
    );
    expect(wrapper.find('[data-test="task-card-error-slot"]').exists()).toBe(true);
    expect(wrapper.find(".task-card-error").exists()).toBe(false);
  });

  it("keeps the action column available beside long task text", () => {
    const fileName = "release_candidate_with_localization_assets_and_debug_symbols.tar.zst";
    const category = "共享文件夹 / projects / releases / 2026 / candidate";
    const { wrapper } = mountWithPinia(TaskDesktopCard, {
      props: {
        task: createTask({ fileName, category }),
      },
    });

    expect(wrapper.get(".task-card-title").attributes("title")).toBe(fileName);
    expect(wrapper.get(".task-card-meta").text()).toContain(category);
    expect(wrapper.find(".task-card-actions").exists()).toBe(true);
  });
});
