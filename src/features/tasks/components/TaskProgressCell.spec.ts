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

  it("uses empty tone when task size is unknown", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ status: "pending", completedLength: 0, totalLength: 0 }),
      },
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("empty");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(0);
  });

  it("keeps default tone for unfinished tasks", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ status: "active", completedLength: 50, totalLength: 100 }),
      },
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("default");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(50);
    expect(wrapper.findComponent(TaskProgressBar).props()).not.toHaveProperty("transitionMs");
  });

  it("advances progress for the same task", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 20 }) },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 40 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(40);
  });

  it("does not move backwards for an older event from the same task", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 20 }) },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 40 }) });
    await wrapper.setProps({ task: createTask({ completedLength: 30 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(40);
  });

  it("resets progress when the task id changes", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 80 }) },
    });

    await wrapper.setProps({ task: createTask({ id: 2, completedLength: 10 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(10);
  });

  it("resets progress when the task gid changes", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 80 }) },
    });

    await wrapper.setProps({ task: createTask({ gid: "gid-2", completedLength: 10 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(10);
  });

  it("recalculates progress when total length changes", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 80, totalLength: 100 }) },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 20, totalLength: 200 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(10);
  });

  it("recovers from unknown total length when a size becomes available", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ completedLength: 0, totalLength: 0, status: "pending" }),
      },
    });

    await wrapper.setProps({
      task: createTask({ completedLength: 20, totalLength: 100, status: "active" }),
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("default");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(20);
  });

  it("forces complete tasks to 100 percent", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 20, totalLength: 100 }) },
    });

    await wrapper.setProps({
      task: createTask({ completedLength: 20, totalLength: 100, status: "complete" }),
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("complete");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(100);
  });

  it("does not render a percentage label when showLabel is false", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ completedLength: 20 }),
        showLabel: false,
      },
    });

    expect(wrapper.find("small").exists()).toBe(false);
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
    useProxy: false,
    metadataTorrentPath: null,
    confirmationRequired: false,
    files: [],
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}
