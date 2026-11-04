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
    expect(dynamicMetrics.findAll(":scope > .task-card-metric")).toHaveLength(2);
    expect(wrapper.get('[data-test="task-eta-metric"]').attributes("aria-label")).toBe("剩余时间 约 1秒");
    expect(wrapper.get('[data-test="task-speed-metric"]').attributes("aria-label")).toBe("速度 1.0 KB/s");
    expect(wrapper.find('[data-test="task-category-metric"]').exists()).toBe(false);
  });

  it("renders grid task metrics", () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
        variant: "grid",
      },
    });

    expect(wrapper.classes()).toContain("task-card-meta--grid");
    expect(wrapper.findAll("dt").map((item) => item.text())).toEqual(["已下载 / 总大小", "速度", "剩余时间"]);
    expect(wrapper.findAll("dd").map((item) => item.text())).toEqual(["1000 B / 2.0 KB", "1.0 KB/s", "约 1秒"]);
  });

  it("keeps the same metric slots while speed and ETA change", async () => {
    const wrapper = mount(TaskMetaItems, {
      props: {
        task: createTask(),
        variant: "grid",
      },
    });

    await wrapper.setProps({ task: createTask({ completedLength: 1900, downloadSpeed: 100 }) });

    expect(wrapper.findAll("dt").map((item) => item.text())).toEqual(["已下载 / 总大小", "速度", "剩余时间"]);
    expect(wrapper.findAll("dd").map((item) => item.text())).toEqual(["1.9 KB / 2.0 KB", "100 B/s", "约 1秒"]);
  });

  it.each(["inline", "grid"] as const)("keeps %s metric slots while tasks pause and finish", async (variant) => {
    const wrapper = mount(TaskMetaItems, { props: { task: createTask(), variant } });
    for (const status of ["pending", "paused", "complete", "error", "removed"] as const) {
      await wrapper.setProps({ task: createTask({ status, downloadSpeed: 1024 }) });
      expect(wrapper.get('[data-test="task-speed-metric"]').text()).toContain("--");
      expect(wrapper.get('[data-test="task-eta-metric"]').text()).toContain("--");
      expect(wrapper.get('[data-test="task-size-metric"]').text()).toContain("1000 B / 2.0 KB");
    }
    await wrapper.setProps({ task: createTask({ status: "active" }) });
    expect(wrapper.get('[data-test="task-speed-metric"]').text()).toContain("1.0 KB/s");
    expect(wrapper.get('[data-test="task-eta-metric"]').text()).toContain("约 1秒");
  });

  it("keeps downloaded bytes and speed visible when total size is unknown", () => {
    const wrapper = mount(TaskMetaItems, {
      props: { task: createTask({ totalLength: 0, completedLength: 2048 }) },
    });
    expect(wrapper.get('[data-test="task-size-metric"]').text()).toContain("2.0 KB / 未知");
    expect(wrapper.get('[data-test="task-speed-metric"]').text()).toContain("1.0 KB/s");
    expect(wrapper.get('[data-test="task-eta-metric"]').text()).toContain("--");
  });
});
