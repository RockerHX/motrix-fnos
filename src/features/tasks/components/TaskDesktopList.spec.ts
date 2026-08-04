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

vi.mock("./TaskDesktopCard.vue", () => ({
  default: {
    name: "TaskDesktopCardStub",
    props: ["task"],
    template: '<article data-test="task-desktop-card">{{ task.fileName }}</article>',
  },
}));

import TaskDesktopList from "./TaskDesktopList.vue";
import { mountWithPinia } from "../../../test/mount";
import type { DownloadTask } from "../../../types/tasks";

describe("TaskDesktopList", () => {
  it("renders an empty desktop list container", () => {
    const { wrapper } = mountWithPinia(TaskDesktopList, {
      props: {
        tasks: [],
      },
    });

    expect(wrapper.find('[data-test="task-desktop-list"]').exists()).toBe(true);
    expect(wrapper.findAll('[data-test="task-desktop-card"]')).toHaveLength(0);
  });

  it("renders all desktop task cards", () => {
    const { wrapper } = mountWithPinia(TaskDesktopList, {
      props: {
        tasks: [createTask(1, "one.iso"), createTask(2, "two.iso")],
      },
    });

    const cards = wrapper.findAll('[data-test="task-desktop-card"]');
    expect(cards).toHaveLength(2);
    expect(cards[0].text()).toBe("one.iso");
    expect(cards[1].text()).toBe("two.iso");
    expect(wrapper.findAll(".n-list-item")).toHaveLength(2);
  });

  it("forwards a card double click to the task status action", async () => {
    const task = createTask(1, "one.iso");
    const { wrapper } = mountWithPinia(TaskDesktopList, {
      props: { tasks: [task] },
    });

    await wrapper.get('[data-test="task-desktop-card"]').trigger("dblclick");

    expect(handleTaskDoubleClick).toHaveBeenCalledOnce();
    expect(handleTaskDoubleClick.mock.calls[0][0]).toEqual(task);
    expect(handleTaskDoubleClick.mock.calls[0][1]).toBeInstanceOf(MouseEvent);
  });
});

function createTask(id: number, fileName: string): DownloadTask {
  return {
    id,
    url: `https://example.com/${fileName}`,
    fileName,
    saveDir: "/downloads",
    category: "默认",
    gid: `gid-${id}`,
    status: "active",
    totalLength: 2000,
    completedLength: 1000,
    downloadSpeed: 1024,
    errorCode: null,
    errorMessage: null,
    filePath: `/downloads/${fileName}`,
    useProxy: false,
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
  };
}
