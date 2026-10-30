import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import TaskMetaItems from "./TaskMetaItems.vue";
import { createTask } from "../../../test/tasks";

describe("TaskMetaItems", () => {
  it("renders inline task metrics", () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
      },
    });

    expect(wrapper.classes()).toContain("task-card-meta--inline");
    expect(wrapper.text()).toContain("1000 B / 2.0 KB");
    expect(wrapper.get('[data-test="task-size-metric"]').text()).toContain("1000 B / 2.0 KB");
    const dynamicMetrics = wrapper.get('[data-test="task-dynamic-metrics"]');
    expect(dynamicMetrics.findAll(":scope > .task-card-metric")).toHaveLength(3);
    expect(wrapper.get('[data-test="task-eta-metric"]').attributes("aria-label")).toBe("剩余时间 约 1秒");
    expect(wrapper.get('[data-test="task-speed-metric"]').attributes("aria-label")).toBe("速度 1.0 KB/s");
    expect(wrapper.get('[data-test="task-category-metric"]').text()).toContain("默认");
  });

  it("renders grid task metrics", () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
        variant: "grid",
      },
    });

    expect(wrapper.classes()).toContain("task-card-meta--grid");
    expect(wrapper.findAll("dt").map((item) => item.text())).toEqual(["已下载 / 总大小", "速度", "剩余时间", "分类"]);
    expect(wrapper.findAll("dd").map((item) => item.text())).toEqual(["1000 B / 2.0 KB", "1.0 KB/s", "约 1秒", "默认"]);
  });

  it("keeps the same metric slots while speed and ETA change", async () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
        variant: "grid",
      },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 1900, downloadSpeed: 100 }) });

    expect(wrapper.findAll("dt").map((item) => item.text())).toEqual(["已下载 / 总大小", "速度", "剩余时间", "分类"]);
    expect(wrapper.findAll("dd").map((item) => item.text())).toEqual(["1.9 KB / 2.0 KB", "100 B/s", "约 1秒", "默认"]);
  });
});
