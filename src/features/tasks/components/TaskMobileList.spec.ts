import { describe, expect, it, vi } from "vitest";

const { handleTaskDoubleClick } = vi.hoisted(() => ({
  handleTaskDoubleClick: vi.fn(),
}));

vi.mock("naive-ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("naive-ui")>();
  return { ...actual, useMessage: () => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }) };
});

vi.mock("../composables/useTaskStatusActions", () => ({
  useTaskStatusActions: () => ({ handleTaskDoubleClick }),
}));

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
    props: {
      task: {
        type: Object,
        required: true,
      },
      showLabel: {
        type: Boolean,
        default: true,
      },
      variant: {
        type: String,
        default: "compact",
      },
    },
    template: '<div data-test="task-progress">progress-{{ task.id }}-{{ showLabel }}-{{ variant }}</div>',
  },
}));

vi.mock("./TaskStatusBadge.vue", () => ({
  default: {
    name: "TaskStatusBadgeStub",
    props: ["task"],
    template: '<span data-test="task-status">{{ task.status }}</span>',
  },
}));

import TaskMobileList from "./TaskMobileList.vue";
import { mountWithPinia } from "../../../test/mount";
import { createTask } from "../../../test/tasks";

describe("TaskMobileList", () => {
  it("renders task title, status, url, metrics, progress and actions", () => {
    const { wrapper } = mountWithPinia(TaskMobileList, {
      props: {
        tasks: [createTask()],
      },
    });

    expect(wrapper.findAll(".task-card")).toHaveLength(1);
    expect(wrapper.get(".task-card-title").text()).toBe("ubuntu.iso");
    expect(wrapper.get('[data-test="task-status"]').text()).toBe("active");
    expect(wrapper.get(".task-card-url").text()).toBe("https://example.com/ubuntu.iso");
    expect(wrapper.get('[data-test="task-progress"]').text()).toBe("progress-1-true-compact");
    expect(wrapper.get(".task-card-meta").text()).toContain("1000 B / 2.0 KB");
    expect(wrapper.get(".task-card-meta").text()).toContain("1.0 KB/s");
    expect(wrapper.get(".task-card-meta").text()).not.toContain("默认");
    expect(wrapper.get('[data-test="task-actions"]').text()).toContain("actions-1-mobile");
  });

  it("renders shared error message for failed task", () => {
    const { wrapper } = mountWithPinia(TaskMobileList, {
      props: {
        tasks: [createTask({ status: "error", errorCode: "16", errorMessage: "network unreachable" })],
      },
    });

    const error = wrapper.get(".task-card-error");
    expect(error.classes()).toContain("task-card-error--multi-line");
    expect(error.text()).toBe("错误码 16：network unreachable");
    expect(error.attributes("title")).toBe("错误码 16：network unreachable");
  });

  it("forwards a card double click to the task status action", async () => {
    const task = createTask();
    const { wrapper } = mountWithPinia(TaskMobileList, {
      props: { tasks: [task] },
    });

    await wrapper.get(".task-card").trigger("dblclick");

    expect(handleTaskDoubleClick).toHaveBeenCalledOnce();
    expect(handleTaskDoubleClick.mock.calls[0][0]).toEqual(task);
    expect(handleTaskDoubleClick.mock.calls[0][1]).toBeInstanceOf(MouseEvent);
  });

  it("keeps long URLs inside a single task card and leaves categories in details", () => {
    const url = "https://example.com/releases/2026/09/08/" + "candidate-assets-".repeat(8) + ".tar.zst";
    const category = "共享文件夹 / projects / releases / 2026 / candidate";
    const { wrapper } = mountWithPinia(TaskMobileList, {
      props: {
        tasks: [createTask({ url, category })],
      },
    });

    expect(wrapper.get(".task-card-url").attributes("title")).toBe(url);
    expect(wrapper.get(".task-card-url").text()).toBe(url);
    expect(wrapper.get(".task-card-meta").text()).not.toContain(category);
    expect(wrapper.find(".task-card-actions").exists()).toBe(true);
  });

  it("keeps multiple task cards present when dynamic metrics differ", () => {
    const { wrapper } = mountWithPinia(TaskMobileList, {
      props: {
        tasks: [
          createTask({ id: 1, downloadSpeed: 1024, completedLength: 1000 }),
          createTask({ id: 2, downloadSpeed: 0, completedLength: 0, status: "pending" }),
        ],
      },
    });

    expect(wrapper.findAll(".task-card")).toHaveLength(2);
    expect(wrapper.findAll('[data-test="task-dynamic-metrics"]')).toHaveLength(0);
    expect(wrapper.findAll(".task-card-meta")).toHaveLength(2);
  });
});
