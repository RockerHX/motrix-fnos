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
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(50);
    expect(wrapper.get("small").text()).toBe("50.0%");
  });

  it("uses empty tone when task size is unknown", () => {
    const wrapper = mount(TaskProgressCell, {
      props: {
        task: createTask({ status: "pending", completedLength: 0, totalLength: 0 }),
      },
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("empty");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBeNull();
    expect(wrapper.get("small").text()).toBe("--");
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBeUndefined();
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuetext")).toBe("进度未知");
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

  it("reflects corrected progress from the current task snapshot", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 20 }) },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 40 }) });
    await wrapper.setProps({ task: createTask({ completedLength: 30 }) });

    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(30);
    expect(wrapper.get("small").text()).toBe("30.0%");
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

  it("keeps completed task progress consistent with its reported bytes", async () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ completedLength: 20, totalLength: 100 }) },
    });

    await wrapper.setProps({
      task: createTask({ completedLength: 20, totalLength: 100, status: "complete" }),
    });

    expect(wrapper.findComponent(TaskProgressBar).props("tone")).toBe("complete");
    expect(wrapper.findComponent(TaskProgressBar).props("percentage")).toBe(20);
    expect(wrapper.get("small").text()).toBe("20.0%");
  });

  it("does not round an unfinished task up to 100 percent", () => {
    const wrapper = mount(TaskProgressCell, {
      props: { task: createTask({ totalLength: 100000, completedLength: 99999 }) },
    });

    expect(wrapper.get("small").text()).toBe("99.9%");
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("99.9");
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
    confirmationRequired: false,
    files: [],
    createdAt: 0,
    updatedAt: 0,
    ...overrides,
  };
}
