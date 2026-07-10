import { describe, expect, it, vi } from "vitest";

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
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 1,
    updatedAt: 2,
  };
}
